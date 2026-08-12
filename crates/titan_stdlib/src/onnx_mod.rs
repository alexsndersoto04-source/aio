//! ONNX inference (`std::onnx::*`) powered by `tract-onnx` 0.21.
//!
//! `tract` is Sonos' production inference engine — the same runtime
//! that powers wake-word detection on their smart speakers. It's
//! 100% pure Rust: no CUDA, no cuDNN, no BLAS, no ONNX Runtime C++.
//! Runs on any target Rust compiles for, including armv7-linux-androideabi
//! (Termux). Perfect for on-device inference on modest phones.
//!
//! ## What it does
//!
//! Given a `.onnx` model file, this module lets `.titan` code:
//!
//! * Load the model once (`load(path) → handle`).
//! * Inspect its input / output shapes.
//! * Run inference: give it a flat `[f64]` array plus its shape, get
//!   a flat `[f64]` array back plus the output shape.
//!
//! Inputs and outputs cross the .titan boundary as flat `Array` of
//! `Float` values, plus a small `[Int]` shape descriptor. That keeps the
//! surface simple and the memory model predictable: no opaque tensor
//! handles, no lifetime dance.
//!
//! ## Example (Rust API)
//!
//! ```rust,ignore
//! use titan_stdlib::onnx_mod;
//! let h = onnx_mod::load("mnist-8.onnx")?;
//! // MNIST expects a 1x1x28x28 float tensor.
//! let pixels: Vec<f32> = read_28x28_grayscale();
//! let (out, shape) = onnx_mod::run_f32(h, &[1, 1, 28, 28], &pixels)?;
//! // out.len() == 10, one score per digit.
//! onnx_mod::close(h);
//! ```

use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::sync::{Arc, Mutex, OnceLock};

use thiserror::Error;
use tract_onnx::prelude::*;

/// The heavy runnable model type. Once optimized it can accept a
/// stream of inputs and produce outputs without further compilation.
type RunnableModel = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

const MAX_MODEL_HANDLES: usize = 4;
const MAX_MODEL_FILE_BYTES: usize = 256 * 1024 * 1024;
const MAX_RUNTIME_MODEL_FILE_BYTES: usize = 512 * 1024 * 1024;
const MAX_MODEL_PATH_BYTES: usize = 16 * 1024;
const MAX_MODEL_INPUTS: usize = 8;
const MAX_MODEL_OUTPUTS: usize = 32;
const MAX_SHAPE_RANK: usize = 8;
const MAX_TENSOR_DIMENSION: usize = 4_194_304;
const MAX_TENSOR_ELEMENTS: usize = 4_194_304;
const MAX_INFERENCE_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTPUT_ELEMENTS: usize = 4_194_304;
const MAX_INFERENCE_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_CONCURRENT_ONNX_OPERATIONS: usize = 1;

#[derive(Debug, Error)]
pub enum OnnxError {
    #[error("onnx error: {0}")]
    Tract(String),
    #[error("unknown model handle {0}")]
    UnknownHandle(i64),
    #[error("shape mismatch: model expects a tensor of {expected} elements, got {found}")]
    ShapeMismatch { expected: usize, found: usize },
    #[error("shape must not contain zero or negative dimensions")]
    BadShape,
    #[error("onnx I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{resource} exceeds limit {limit}")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    #[error("invalid ONNX model source: {0}")]
    InvalidSource(&'static str),
    #[error("onnx handle space exhausted")]
    HandleSpaceExhausted,
}

fn map_err<E: std::fmt::Display>(e: E) -> OnnxError {
    OnnxError::Tract(e.to_string())
}

// ---- Registry --------------------------------------------------------

struct ModelEntry {
    model: Arc<Mutex<RunnableModel>>,
    source_bytes: usize,
}

struct Registry {
    models: HashMap<(u64, i64), ModelEntry>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        Mutex::new(Registry {
            models: HashMap::new(),
            next_id: 1,
        })
    })
}

fn handle_key(handle: i64) -> (u64, i64) {
    crate::native::runtime_handle_key(handle)
}

#[derive(Default)]
struct OperationUsage {
    active: usize,
}

struct OperationPermit {
    runtime_id: u64,
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        let mut usage = crate::native::lock_recover(operation_usage());
        if let Some(runtime) = usage.get_mut(&self.runtime_id) {
            runtime.active = runtime.active.saturating_sub(1);
            if runtime.active == 0 {
                usage.remove(&self.runtime_id);
            }
        }
    }
}

fn operation_usage() -> &'static Mutex<HashMap<u64, OperationUsage>> {
    static USAGE: OnceLock<Mutex<HashMap<u64, OperationUsage>>> = OnceLock::new();
    USAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reserve_operation() -> Result<OperationPermit, OnnxError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut usage = crate::native::lock_recover(operation_usage());
    let active = usage.get(&runtime_id).map_or(0, |runtime| runtime.active);
    if active >= MAX_CONCURRENT_ONNX_OPERATIONS {
        return Err(OnnxError::ResourceLimit {
            resource: "concurrent ONNX operations",
            limit: MAX_CONCURRENT_ONNX_OPERATIONS,
        });
    }
    usage.entry(runtime_id).or_default().active += 1;
    Ok(OperationPermit { runtime_id })
}

fn validate_capacity(active: usize, runtime_bytes: usize, source_bytes: usize) -> Result<(), OnnxError> {
    if active >= MAX_MODEL_HANDLES {
        return Err(OnnxError::ResourceLimit {
            resource: "ONNX model handles",
            limit: MAX_MODEL_HANDLES,
        });
    }
    if source_bytes > MAX_MODEL_FILE_BYTES {
        return Err(OnnxError::ResourceLimit {
            resource: "ONNX model file bytes",
            limit: MAX_MODEL_FILE_BYTES,
        });
    }
    if runtime_bytes.saturating_add(source_bytes) > MAX_RUNTIME_MODEL_FILE_BYTES {
        return Err(OnnxError::ResourceLimit {
            resource: "runtime ONNX model file bytes",
            limit: MAX_RUNTIME_MODEL_FILE_BYTES,
        });
    }
    Ok(())
}

fn current_capacity() -> (usize, usize) {
    let runtime_id = crate::native::current_runtime_id();
    let registry = crate::native::lock_recover(registry());
    registry
        .models
        .iter()
        .filter(|((owner, _), _)| *owner == runtime_id)
        .fold((0usize, 0usize), |(handles, bytes), (_, entry)| {
            (
                handles.saturating_add(1),
                bytes.saturating_add(entry.source_bytes),
            )
        })
}

fn inspect_source(path: &str) -> Result<(std::fs::File, usize), OnnxError> {
    if path.len() > MAX_MODEL_PATH_BYTES {
        return Err(OnnxError::ResourceLimit {
            resource: "ONNX model path bytes",
            limit: MAX_MODEL_PATH_BYTES,
        });
    }
    let (active, runtime_bytes) = current_capacity();
    validate_capacity(active, runtime_bytes, 0)?;
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Err(OnnxError::InvalidSource("model path is not a regular file"));
    }
    let source_bytes = usize::try_from(metadata.len()).map_err(|_| OnnxError::ResourceLimit {
        resource: "ONNX model file bytes",
        limit: MAX_MODEL_FILE_BYTES,
    })?;
    validate_capacity(active, runtime_bytes, source_bytes)?;
    Ok((file, source_bytes))
}

fn parse_model(file: std::fs::File, source_bytes: usize) -> Result<InferenceModel, OnnxError> {
    let mut reader = BufReader::new(file.take(source_bytes as u64));
    tract_onnx::onnx().model_for_read(&mut reader).map_err(map_err)
}

fn validate_fact_shape(fact: &TypedFact, allow_zero: bool) -> Result<(usize, usize), OnnxError> {
    if fact.shape.rank() > MAX_SHAPE_RANK {
        return Err(OnnxError::ResourceLimit {
            resource: "model tensor shape rank",
            limit: MAX_SHAPE_RANK,
        });
    }
    if matches!(&fact.datum_type, DatumType::String | DatumType::Blob) {
        return Err(OnnxError::InvalidSource(
            "variable-width input and output tensors are unsupported",
        ));
    }
    let mut known_elements = 1usize;
    for dimension in fact.shape.iter().filter_map(|dimension| dimension.to_i64()) {
        let dimension = usize::try_from(dimension).map_err(|_| OnnxError::BadShape)?;
        if dimension == 0 && !allow_zero {
            return Err(OnnxError::BadShape);
        }
        if dimension > MAX_TENSOR_DIMENSION {
            return Err(OnnxError::ResourceLimit {
                resource: "model tensor dimension",
                limit: MAX_TENSOR_DIMENSION,
            });
        }
        known_elements = known_elements
            .checked_mul(dimension)
            .ok_or(OnnxError::ResourceLimit {
                resource: "model tensor elements",
                limit: MAX_TENSOR_ELEMENTS,
            })?;
        if known_elements > MAX_TENSOR_ELEMENTS {
            return Err(OnnxError::ResourceLimit {
                resource: "model tensor elements",
                limit: MAX_TENSOR_ELEMENTS,
            });
        }
    }
    let known_bytes = known_elements
        .checked_mul(fact.datum_type.size_of())
        .ok_or(OnnxError::ResourceLimit {
            resource: "model tensor bytes",
            limit: MAX_INFERENCE_OUTPUT_BYTES,
        })?;
    Ok((known_elements, known_bytes))
}

fn validate_model_ports(input_count: usize, output_count: usize) -> Result<(), OnnxError> {
    if input_count > MAX_MODEL_INPUTS {
        return Err(OnnxError::ResourceLimit {
            resource: "ONNX model inputs",
            limit: MAX_MODEL_INPUTS,
        });
    }
    if output_count > MAX_MODEL_OUTPUTS {
        return Err(OnnxError::ResourceLimit {
            resource: "ONNX model outputs",
            limit: MAX_MODEL_OUTPUTS,
        });
    }
    Ok(())
}

fn validate_model(model: &RunnableModel) -> Result<(), OnnxError> {
    validate_model_ports(model.model().inputs.len(), model.model().outputs.len())?;
    let mut input_bytes = 0usize;
    for index in 0..model.model().inputs.len() {
        let (_, known_bytes) =
            validate_fact_shape(model.model().input_fact(index).map_err(map_err)?, false)?;
        input_bytes = input_bytes
            .checked_add(known_bytes)
            .ok_or(OnnxError::ResourceLimit {
                resource: "model input bytes",
                limit: MAX_INFERENCE_INPUT_BYTES,
            })?;
    }
    if input_bytes > MAX_INFERENCE_INPUT_BYTES {
        return Err(OnnxError::ResourceLimit {
            resource: "model input bytes",
            limit: MAX_INFERENCE_INPUT_BYTES,
        });
    }

    let mut output_elements = 0usize;
    let mut output_bytes = 0usize;
    for index in 0..model.model().outputs.len() {
        let (known_elements, known_bytes) =
            validate_fact_shape(model.model().output_fact(index).map_err(map_err)?, true)?;
        output_elements = output_elements
            .checked_add(known_elements)
            .ok_or(OnnxError::ResourceLimit {
                resource: "model output elements",
                limit: MAX_OUTPUT_ELEMENTS,
            })?;
        output_bytes = output_bytes
            .checked_add(known_bytes)
            .ok_or(OnnxError::ResourceLimit {
                resource: "model output bytes",
                limit: MAX_INFERENCE_OUTPUT_BYTES,
            })?;
    }
    if output_elements > MAX_OUTPUT_ELEMENTS {
        return Err(OnnxError::ResourceLimit {
            resource: "model output elements",
            limit: MAX_OUTPUT_ELEMENTS,
        });
    }
    if output_bytes > MAX_INFERENCE_OUTPUT_BYTES {
        return Err(OnnxError::ResourceLimit {
            resource: "model output bytes",
            limit: MAX_INFERENCE_OUTPUT_BYTES,
        });
    }
    Ok(())
}

fn insert(model: RunnableModel, source_bytes: usize) -> Result<i64, OnnxError> {
    validate_model(&model)?;
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(registry());
    let (active, runtime_bytes) = registry
        .models
        .iter()
        .filter(|((owner, _), _)| *owner == runtime_id)
        .fold((0usize, 0usize), |(handles, bytes), (_, entry)| {
            (
                handles.saturating_add(1),
                bytes.saturating_add(entry.source_bytes),
            )
        });
    validate_capacity(active, runtime_bytes, source_bytes)?;
    let id = registry.next_id;
    registry.next_id = id.checked_add(1).ok_or(OnnxError::HandleSpaceExhausted)?;
    registry.models.insert(
        (runtime_id, id),
        ModelEntry {
            model: Arc::new(Mutex::new(model)),
            source_bytes,
        },
    );
    Ok(id)
}

fn get(handle: i64) -> Result<Arc<Mutex<RunnableModel>>, OnnxError> {
    crate::native::lock_recover(registry())
        .models
        .get(&handle_key(handle))
        .map(|entry| Arc::clone(&entry.model))
        .ok_or(OnnxError::UnknownHandle(handle))
}

fn with<F, R>(handle: i64, action: F) -> Result<R, OnnxError>
where
    F: FnOnce(&RunnableModel) -> Result<R, OnnxError>,
{
    let model = get(handle)?;
    let model = crate::native::lock_recover(&model);
    action(&model)
}

/// Reject oversized VM shape arrays before converting their elements.
#[doc(hidden)]
pub fn preflight_shape_rank(rank: usize) -> Result<(), OnnxError> {
    if rank > MAX_SHAPE_RANK {
        return Err(OnnxError::ResourceLimit {
            resource: "tensor shape rank",
            limit: MAX_SHAPE_RANK,
        });
    }
    Ok(())
}

fn checked_shape(shape: &[i64]) -> Result<(Vec<usize>, usize), OnnxError> {
    preflight_shape_rank(shape.len())?;
    let mut converted = Vec::with_capacity(shape.len());
    let mut elements = 1usize;
    for &dimension in shape {
        let dimension = usize::try_from(dimension).map_err(|_| OnnxError::BadShape)?;
        if dimension == 0 {
            return Err(OnnxError::BadShape);
        }
        if dimension > MAX_TENSOR_DIMENSION {
            return Err(OnnxError::ResourceLimit {
                resource: "tensor dimension",
                limit: MAX_TENSOR_DIMENSION,
            });
        }
        elements = elements.checked_mul(dimension).ok_or(OnnxError::ResourceLimit {
            resource: "tensor elements",
            limit: MAX_TENSOR_ELEMENTS,
        })?;
        if elements > MAX_TENSOR_ELEMENTS {
            return Err(OnnxError::ResourceLimit {
                resource: "tensor elements",
                limit: MAX_TENSOR_ELEMENTS,
            });
        }
        converted.push(dimension);
    }
    Ok((converted, elements))
}

fn validate_input_bytes(elements: usize, element_bytes: usize, tensors: usize) -> Result<(), OnnxError> {
    let bytes = elements
        .checked_mul(element_bytes)
        .and_then(|bytes| bytes.checked_mul(tensors))
        .ok_or(OnnxError::ResourceLimit {
            resource: "inference input bytes",
            limit: MAX_INFERENCE_INPUT_BYTES,
        })?;
    if bytes > MAX_INFERENCE_INPUT_BYTES {
        return Err(OnnxError::ResourceLimit {
            resource: "inference input bytes",
            limit: MAX_INFERENCE_INPUT_BYTES,
        });
    }
    Ok(())
}

/// Validate tensor lengths before the VM duplicates values into tract buffers.
#[doc(hidden)]
pub fn preflight_input_lengths(
    shape: &[i64],
    lengths: &[usize],
    element_bytes: usize,
) -> Result<(), OnnxError> {
    if lengths.len() > MAX_MODEL_INPUTS {
        return Err(OnnxError::ResourceLimit {
            resource: "ONNX inference inputs",
            limit: MAX_MODEL_INPUTS,
        });
    }
    let (_, expected) = checked_shape(shape)?;
    validate_input_bytes(expected, element_bytes, lengths.len())?;
    if let Some(&found) = lengths.iter().find(|&&length| length != expected) {
        return Err(OnnxError::ShapeMismatch { expected, found });
    }
    Ok(())
}

fn validate_outputs(outputs: &[TValue]) -> Result<(), OnnxError> {
    if outputs.len() > MAX_MODEL_OUTPUTS {
        return Err(OnnxError::ResourceLimit {
            resource: "ONNX inference outputs",
            limit: MAX_MODEL_OUTPUTS,
        });
    }
    let mut elements = 0usize;
    let mut bytes = 0usize;
    for output in outputs {
        if output.rank() > MAX_SHAPE_RANK {
            return Err(OnnxError::ResourceLimit {
                resource: "output tensor shape rank",
                limit: MAX_SHAPE_RANK,
            });
        }
        if output
            .shape()
            .iter()
            .any(|&dimension| dimension > MAX_TENSOR_DIMENSION)
        {
            return Err(OnnxError::ResourceLimit {
                resource: "output tensor dimension",
                limit: MAX_TENSOR_DIMENSION,
            });
        }
        elements = elements
            .checked_add(output.len())
            .ok_or(OnnxError::ResourceLimit {
                resource: "inference output elements",
                limit: MAX_OUTPUT_ELEMENTS,
            })?;
        bytes = output
            .len()
            .checked_mul(output.datum_type().size_of())
            .and_then(|output_bytes| bytes.checked_add(output_bytes))
            .ok_or(OnnxError::ResourceLimit {
                resource: "inference output bytes",
                limit: MAX_INFERENCE_OUTPUT_BYTES,
            })?;
    }
    if elements > MAX_OUTPUT_ELEMENTS {
        return Err(OnnxError::ResourceLimit {
            resource: "inference output elements",
            limit: MAX_OUTPUT_ELEMENTS,
        });
    }
    if bytes > MAX_INFERENCE_OUTPUT_BYTES {
        return Err(OnnxError::ResourceLimit {
            resource: "inference output bytes",
            limit: MAX_INFERENCE_OUTPUT_BYTES,
        });
    }
    Ok(())
}

// ---- Public API -----------------------------------------------------

/// Load a `.onnx` model from disk. Reads → parses → optimizes → makes
/// runnable in one shot; the returned handle is ready for `run_*`.
///
/// If the model has symbolic ("None") input shapes, callers should
/// prefer `load_with_input_shape` so tract can constant-fold shapes
/// before optimization.
pub fn load(path: &str) -> Result<i64, OnnxError> {
    let _permit = reserve_operation()?;
    let (file, source_bytes) = inspect_source(path)?;
    let model = parse_model(file, source_bytes)?;
    validate_model_ports(model.inputs.len(), model.outputs.len())?;
    let model = model
        .into_optimized()
        .map_err(map_err)?
        .into_runnable()
        .map_err(map_err)?;
    insert(model, source_bytes)
}

/// Same as `load` but pin the model's *first* input to `input_shape`
/// (of `f32`) before optimization. Necessary for models whose ONNX
/// graph exposes dynamic dimensions (batch, sequence length, ...).
pub fn load_with_input_shape(path: &str, input_shape: &[i64]) -> Result<i64, OnnxError> {
    let (shape, elements) = checked_shape(input_shape)?;
    validate_input_bytes(elements, std::mem::size_of::<f32>(), 1)?;
    let _permit = reserve_operation()?;
    let (file, source_bytes) = inspect_source(path)?;
    let model = parse_model(file, source_bytes)?;
    validate_model_ports(model.inputs.len(), model.outputs.len())?;
    let model = model
        .with_input_fact(0, f32::fact(&shape).into())
        .map_err(map_err)?
        .into_optimized()
        .map_err(map_err)?
        .into_runnable()
        .map_err(map_err)?;
    insert(model, source_bytes)
}

/// Load a BERT-family model with two `i64` inputs (`input_ids`,
/// `attention_mask`) both pinned to the same `[batch, seq_len]` shape.
/// This is the shape most HuggingFace transformer ONNX exports use.
pub fn load_bert_shape(path: &str, batch: i64, seq_len: i64) -> Result<i64, OnnxError> {
    let (shape, elements) = checked_shape(&[batch, seq_len])?;
    validate_input_bytes(elements, std::mem::size_of::<i64>(), 2)?;
    let _permit = reserve_operation()?;
    let (file, source_bytes) = inspect_source(path)?;
    let model = parse_model(file, source_bytes)?;
    validate_model_ports(model.inputs.len(), model.outputs.len())?;
    let model = model
        .with_input_fact(0, i64::fact(&shape).into())
        .map_err(map_err)?
        .with_input_fact(1, i64::fact(&shape).into())
        .map_err(map_err)?
        .into_optimized()
        .map_err(map_err)?
        .into_runnable()
        .map_err(map_err)?;
    insert(model, source_bytes)
}

/// Load a HuggingFace transformer that expects three `i64` inputs
/// (`input_ids`, `attention_mask`, `token_type_ids`) — some BERT
/// variants (e.g. classic uncased BERT) need this third tensor.
pub fn load_bert3_shape(path: &str, batch: i64, seq_len: i64) -> Result<i64, OnnxError> {
    let (shape, elements) = checked_shape(&[batch, seq_len])?;
    validate_input_bytes(elements, std::mem::size_of::<i64>(), 3)?;
    let _permit = reserve_operation()?;
    let (file, source_bytes) = inspect_source(path)?;
    let model = parse_model(file, source_bytes)?;
    validate_model_ports(model.inputs.len(), model.outputs.len())?;
    let model = model
        .with_input_fact(0, i64::fact(&shape).into())
        .map_err(map_err)?
        .with_input_fact(1, i64::fact(&shape).into())
        .map_err(map_err)?
        .with_input_fact(2, i64::fact(&shape).into())
        .map_err(map_err)?
        .into_optimized()
        .map_err(map_err)?
        .into_runnable()
        .map_err(map_err)?;
    insert(model, source_bytes)
}

/// Drop a model. Idempotent.
pub fn close(handle: i64) {
    crate::native::lock_recover(registry())
        .models
        .remove(&handle_key(handle));
}

/// How many inputs does the model take?
pub fn input_count(handle: i64) -> Result<usize, OnnxError> {
    with(handle, |m| Ok(m.model().inputs.len()))
}

/// How many outputs does the model produce?
pub fn output_count(handle: i64) -> Result<usize, OnnxError> {
    with(handle, |m| Ok(m.model().outputs.len()))
}

/// Return the shape declared for input `i` (may contain -1 for
/// symbolic / dynamic axes tract could not resolve statically).
pub fn input_shape(handle: i64, i: usize) -> Result<Vec<i64>, OnnxError> {
    with(handle, |m| {
        let fact = m.model().input_fact(i).map_err(map_err)?;
        let shape: Vec<i64> = fact
            .shape
            .iter()
            .map(|d| d.to_i64().unwrap_or(-1))
            .collect();
        Ok(shape)
    })
}

/// Return the shape declared for output `i`.
pub fn output_shape(handle: i64, i: usize) -> Result<Vec<i64>, OnnxError> {
    with(handle, |m| {
        let fact = m.model().output_fact(i).map_err(map_err)?;
        let shape: Vec<i64> = fact
            .shape
            .iter()
            .map(|d| d.to_i64().unwrap_or(-1))
            .collect();
        Ok(shape)
    })
}

/// Run the model with a **single** `f32` tensor input. Returns the
/// **first** output tensor as `(values, shape)` — flat row-major.
///
/// * `input_shape` — expected shape, e.g. `[1, 1, 28, 28]` for MNIST.
/// * `input_data`  — flat row-major buffer whose length must equal the
///                   product of `input_shape`.
pub fn run_f32(
    handle: i64,
    input_shape: &[i64],
    input_data: &[f32],
) -> Result<(Vec<f32>, Vec<usize>), OnnxError> {
    let (shape, expected) = checked_shape(input_shape)?;
    validate_input_bytes(expected, std::mem::size_of::<f32>(), 1)?;
    if input_data.len() != expected {
        return Err(OnnxError::ShapeMismatch {
            expected,
            found: input_data.len(),
        });
    }

    let _permit = reserve_operation()?;
    with(handle, |m| {
        // Build the input tensor from the flat data + shape.
        let tensor = tract_ndarray::ArrayD::from_shape_vec(shape, input_data.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let outputs = m.run(tvec!(tensor.into())).map_err(map_err)?;
        validate_outputs(&outputs)?;
        let first = outputs
            .into_iter()
            .next()
            .ok_or_else(|| OnnxError::Tract("model produced no outputs".into()))?;
        let out_shape: Vec<usize> = first.shape().to_vec();
        let view = first.to_array_view::<f32>().map_err(map_err)?;
        let values: Vec<f32> = view.iter().copied().collect();
        Ok((values, out_shape))
    })
}

/// Run the model with a **single** `i64` tensor input (common for
/// token-id inputs to LLMs / BERT). Returns first output as `f32`.
pub fn run_i64_in_f32_out(
    handle: i64,
    input_shape: &[i64],
    input_data: &[i64],
) -> Result<(Vec<f32>, Vec<usize>), OnnxError> {
    let (shape, expected) = checked_shape(input_shape)?;
    validate_input_bytes(expected, std::mem::size_of::<i64>(), 1)?;
    if input_data.len() != expected {
        return Err(OnnxError::ShapeMismatch {
            expected,
            found: input_data.len(),
        });
    }

    let _permit = reserve_operation()?;
    with(handle, |m| {
        let tensor = tract_ndarray::ArrayD::from_shape_vec(shape, input_data.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let outputs = m.run(tvec!(tensor.into())).map_err(map_err)?;
        validate_outputs(&outputs)?;
        let first = outputs
            .into_iter()
            .next()
            .ok_or_else(|| OnnxError::Tract("model produced no outputs".into()))?;
        let out_shape: Vec<usize> = first.shape().to_vec();
        let view = first.to_array_view::<f32>().map_err(map_err)?;
        let values: Vec<f32> = view.iter().copied().collect();
        Ok((values, out_shape))
    })
}

/// Run a transformer with **two** `i64` inputs of the same shape:
/// `input_ids` and `attention_mask`. Returns the first output as `f32`.
/// Perfect for DistilBERT / BERT-style classifiers.
pub fn run_two_i64(
    handle: i64,
    input_shape: &[i64],
    input_ids: &[i64],
    attention_mask: &[i64],
) -> Result<(Vec<f32>, Vec<usize>), OnnxError> {
    let (shape, expected) = checked_shape(input_shape)?;
    validate_input_bytes(expected, std::mem::size_of::<i64>(), 2)?;
    if input_ids.len() != expected {
        return Err(OnnxError::ShapeMismatch {
            expected,
            found: input_ids.len(),
        });
    }
    if attention_mask.len() != expected {
        return Err(OnnxError::ShapeMismatch {
            expected,
            found: attention_mask.len(),
        });
    }

    let _permit = reserve_operation()?;
    with(handle, |m| {
        let ids_tensor = tract_ndarray::ArrayD::from_shape_vec(shape.clone(), input_ids.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let mask_tensor = tract_ndarray::ArrayD::from_shape_vec(shape, attention_mask.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let outputs = m
            .run(tvec!(ids_tensor.into(), mask_tensor.into()))
            .map_err(map_err)?;
        validate_outputs(&outputs)?;
        let first = outputs
            .into_iter()
            .next()
            .ok_or_else(|| OnnxError::Tract("model produced no outputs".into()))?;
        let out_shape: Vec<usize> = first.shape().to_vec();
        let view = first.to_array_view::<f32>().map_err(map_err)?;
        let values: Vec<f32> = view.iter().copied().collect();
        Ok((values, out_shape))
    })
}

/// Run a transformer with **three** `i64` inputs of the same shape:
/// `input_ids`, `attention_mask`, `token_type_ids`. Used by classic
/// BERT-uncased when the token_type_ids input is not baked into the
/// graph.
pub fn run_three_i64(
    handle: i64,
    input_shape: &[i64],
    input_ids: &[i64],
    attention_mask: &[i64],
    token_type_ids: &[i64],
) -> Result<(Vec<f32>, Vec<usize>), OnnxError> {
    let (shape, expected) = checked_shape(input_shape)?;
    validate_input_bytes(expected, std::mem::size_of::<i64>(), 3)?;
    for data in [input_ids, attention_mask, token_type_ids] {
        if data.len() != expected {
            return Err(OnnxError::ShapeMismatch {
                expected,
                found: data.len(),
            });
        }
    }

    let _permit = reserve_operation()?;
    with(handle, |m| {
        let ids = tract_ndarray::ArrayD::from_shape_vec(shape.clone(), input_ids.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let mask = tract_ndarray::ArrayD::from_shape_vec(shape.clone(), attention_mask.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let types = tract_ndarray::ArrayD::from_shape_vec(shape, token_type_ids.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let outputs = m
            .run(tvec!(ids.into(), mask.into(), types.into()))
            .map_err(map_err)?;
        validate_outputs(&outputs)?;
        let first = outputs
            .into_iter()
            .next()
            .ok_or_else(|| OnnxError::Tract("model produced no outputs".into()))?;
        let out_shape: Vec<usize> = first.shape().to_vec();
        let view = first.to_array_view::<f32>().map_err(map_err)?;
        let values: Vec<f32> = view.iter().copied().collect();
        Ok((values, out_shape))
    })
}

/// Run a sentence-transformer (BERT-family encoder) and pool the token
/// embeddings into a single sentence embedding via **mean pooling
/// weighted by `attention_mask`** (the standard technique used by
/// sentence-transformers). Returns a flat `[hidden_size]` vector.
///
/// This is the pooling that MiniLM / all-MiniLM-L6-v2 / etc. expect
/// after running their raw encoder. Doing it in Rust is much faster
/// than looping in `.titan` (384 * seq_len fp adds per sentence).
///
/// The model must expose `last_hidden_state` as its FIRST output with
/// shape `[batch, seq_len, hidden]`. Every HuggingFace ONNX export of
/// a sentence encoder ships this layout.
pub fn run_bert_pooled(
    handle: i64,
    batch: i64,
    seq_len: i64,
    input_ids: &[i64],
    attention_mask: &[i64],
) -> Result<(Vec<f32>, Vec<usize>), OnnxError> {
    let (shape, expected) = checked_shape(&[batch, seq_len])?;
    validate_input_bytes(expected, std::mem::size_of::<i64>(), 2)?;
    if input_ids.len() != expected {
        return Err(OnnxError::ShapeMismatch {
            expected,
            found: input_ids.len(),
        });
    }
    if attention_mask.len() != expected {
        return Err(OnnxError::ShapeMismatch {
            expected,
            found: attention_mask.len(),
        });
    }

    let _permit = reserve_operation()?;
    with(handle, |m| {
        let ids = tract_ndarray::ArrayD::from_shape_vec(shape.to_vec(), input_ids.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let mask = tract_ndarray::ArrayD::from_shape_vec(shape.to_vec(), attention_mask.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let outputs = m.run(tvec!(ids.into(), mask.into())).map_err(map_err)?;
        validate_outputs(&outputs)?;
        let first = outputs
            .into_iter()
            .next()
            .ok_or_else(|| OnnxError::Tract("model produced no outputs".into()))?;
        let out_shape: Vec<usize> = first.shape().to_vec();
        if out_shape.len() != 3 {
            return Err(OnnxError::Tract(format!(
                "expected [batch, seq_len, hidden] output, got {out_shape:?}"
            )));
        }
        let (b, s, h) = (out_shape[0], out_shape[1], out_shape[2]);
        let pooled_elements = b.checked_mul(h).ok_or(OnnxError::ResourceLimit {
            resource: "pooled output elements",
            limit: MAX_OUTPUT_ELEMENTS,
        })?;
        if pooled_elements > MAX_OUTPUT_ELEMENTS {
            return Err(OnnxError::ResourceLimit {
                resource: "pooled output elements",
                limit: MAX_OUTPUT_ELEMENTS,
            });
        }
        if b != shape[0] || s != shape[1] {
            return Err(OnnxError::Tract(format!(
                "output batch/seq mismatch: expected {shape:?}, got [{b},{s},{h}]"
            )));
        }
        let view = first.to_array_view::<f32>().map_err(map_err)?;

        // Mean pooling weighted by attention_mask.
        // For each batch item, sum embeddings across tokens where
        // mask==1 and divide by count. This ignores [PAD] positions
        // and yields the standard sentence-transformer output.
        let mut pooled = vec![0.0f32; pooled_elements];
        for bi in 0..b {
            let mut count = 0u32;
            for si in 0..s {
                let m_val = attention_mask[bi * s + si];
                if m_val == 0 {
                    continue;
                }
                count += 1;
                for hi in 0..h {
                    pooled[bi * h + hi] += view[[bi, si, hi]];
                }
            }
            if count > 0 {
                let denom = count as f32;
                for hi in 0..h {
                    pooled[bi * h + hi] /= denom;
                }
            }
        }
        Ok((pooled, vec![b, h]))
    })
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut reg = crate::native::lock_recover(registry());
    crate::native::remove_runtime_entries(&mut reg.models, runtime_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_model() -> RunnableModel {
        let mut model = TypedModel::default();
        let input = model.add_source("input", f32::fact(&[1])).unwrap();
        model.set_output_outlets(&[input]).unwrap();
        model.into_runnable().unwrap()
    }

    #[test]
    fn models_shapes_inputs_and_outputs_are_bounded() {
        assert!(matches!(
            validate_capacity(MAX_MODEL_HANDLES, 0, 0),
            Err(OnnxError::ResourceLimit {
                resource: "ONNX model handles",
                ..
            })
        ));
        assert!(matches!(
            validate_capacity(0, MAX_RUNTIME_MODEL_FILE_BYTES, 1),
            Err(OnnxError::ResourceLimit {
                resource: "runtime ONNX model file bytes",
                ..
            })
        ));
        assert!(matches!(
            checked_shape(&[1; MAX_SHAPE_RANK + 1]),
            Err(OnnxError::ResourceLimit {
                resource: "tensor shape rank",
                ..
            })
        ));
        assert!(matches!(checked_shape(&[-1]), Err(OnnxError::BadShape)));
        assert!(matches!(
            checked_shape(&[MAX_TENSOR_DIMENSION as i64, 2]),
            Err(OnnxError::ResourceLimit {
                resource: "tensor elements",
                ..
            })
        ));
        assert!(matches!(
            validate_input_bytes(MAX_TENSOR_ELEMENTS, std::mem::size_of::<i64>(), 3),
            Err(OnnxError::ResourceLimit {
                resource: "inference input bytes",
                ..
            })
        ));
        assert!(matches!(
            preflight_input_lengths(&[2, 2], &[3], std::mem::size_of::<f32>()),
            Err(OnnxError::ShapeMismatch {
                expected: 4,
                found: 3,
            })
        ));
        let outputs = (0..=MAX_MODEL_OUTPUTS)
            .map(|_| TValue::from(tensor0(0.0f32)))
            .collect::<Vec<_>>();
        assert!(matches!(
            validate_outputs(&outputs),
            Err(OnnxError::ResourceLimit {
                resource: "ONNX inference outputs",
                ..
            })
        ));
        assert!(matches!(
            run_f32(999_999, &[MAX_TENSOR_DIMENSION as i64, 2], &[]),
            Err(OnnxError::ResourceLimit { .. })
        ));
    }

    #[test]
    fn handle_capacity_close_replacement_and_inference_are_real() {
        let runtime_id = 8_300_009;
        let foreign_handle = crate::native::with_runtime_context(runtime_id, || {
            let mut handles = (0..MAX_MODEL_HANDLES)
                .map(|_| insert(identity_model(), 1).unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                insert(identity_model(), 1),
                Err(OnnxError::ResourceLimit {
                    resource: "ONNX model handles",
                    ..
                })
            ));
            let (values, shape) = run_f32(handles[0], &[1], &[2.5]).unwrap();
            assert_eq!(values, vec![2.5]);
            assert_eq!(shape, vec![1]);
            close(handles.pop().unwrap());
            handles.push(insert(identity_model(), 1).unwrap());
            handles[0]
        });
        crate::native::with_runtime_context(runtime_id + 1, || {
            assert!(matches!(
                input_count(foreign_handle),
                Err(OnnxError::UnknownHandle(_))
            ));
        });
        assert_eq!(cleanup_runtime(runtime_id), MAX_MODEL_HANDLES);
    }

    #[test]
    fn operations_are_released_and_scoped_per_runtime() {
        let runtime_id = 8_300_010;
        crate::native::with_runtime_context(runtime_id, || {
            let permit = reserve_operation().unwrap();
            assert!(matches!(
                reserve_operation(),
                Err(OnnxError::ResourceLimit {
                    resource: "concurrent ONNX operations",
                    ..
                })
            ));
            drop(permit);
            reserve_operation().unwrap();
        });
        assert!(!crate::native::lock_recover(operation_usage()).contains_key(&runtime_id));
    }

    #[test]
    fn files_are_preflighted_before_parsing() {
        assert!(matches!(
            inspect_source(std::env::temp_dir().to_string_lossy().as_ref()),
            Err(OnnxError::InvalidSource(_))
        ));

        let path = std::env::temp_dir().join(format!(
            "titan-onnx-oversized-{}-{}.onnx",
            std::process::id(),
            crate::native::current_runtime_id()
        ));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len((MAX_MODEL_FILE_BYTES as u64) + 1).unwrap();
        drop(file);
        let result = inspect_source(path.to_string_lossy().as_ref());
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(
            result,
            Err(OnnxError::ResourceLimit {
                resource: "ONNX model file bytes",
                ..
            })
        ));

        std::fs::write(&path, b"not an ONNX protobuf").unwrap();
        let runtime_id = 8_300_011;
        let result = crate::native::with_runtime_context(runtime_id, || {
            load(path.to_string_lossy().as_ref())
        });
        std::fs::remove_file(path).unwrap();
        assert!(matches!(result, Err(OnnxError::Tract(_))));
        assert!(!crate::native::lock_recover(operation_usage()).contains_key(&runtime_id));
        assert_eq!(cleanup_runtime(runtime_id), 0);
    }

    #[test]
    fn unknown_handle_reports_typed_error() {
        assert!(matches!(
            input_count(999_999),
            Err(OnnxError::UnknownHandle(_))
        ));
        assert!(matches!(
            run_f32(999_999, &[1, 1, 1, 1], &[0.0]),
            Err(OnnxError::UnknownHandle(_))
        ));
    }

    #[test]
    fn load_errors_on_missing_file() {
        assert!(load("/nonexistent/model.onnx").is_err());
    }

    #[test]
    fn bad_shapes_are_rejected() {
        assert!(matches!(
            load_with_input_shape("/nope.onnx", &[0, 1]),
            Err(OnnxError::BadShape)
        ));
    }

    /// Live test opt-in: set TITAN_ONNX_MODEL=/path/to/model.onnx and
    /// TITAN_ONNX_SHAPE=1,1,28,28 (comma-separated).
    #[test]
    fn round_trip_when_configured() {
        let Ok(path) = std::env::var("TITAN_ONNX_MODEL") else {
            return;
        };
        let shape_str = std::env::var("TITAN_ONNX_SHAPE").unwrap_or_else(|_| "1,1,28,28".into());
        let shape: Vec<i64> = shape_str
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect();
        let (_, elems) = checked_shape(&shape).expect("shape");

        let h = load_with_input_shape(&path, &shape).expect("load");
        let data = vec![0.0f32; elems];
        let (out, out_shape) = run_f32(h, &shape, &data).expect("run");
        assert!(!out.is_empty());
        assert!(!out_shape.is_empty());
        close(h);
    }
}
