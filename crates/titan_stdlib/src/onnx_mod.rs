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
use std::sync::{Mutex, OnceLock};

use thiserror::Error;
use tract_onnx::prelude::*;

/// The heavy runnable model type. Once optimized it can accept a
/// stream of inputs and produce outputs without further compilation.
type RunnableModel = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

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
}

fn map_err<E: std::fmt::Display>(e: E) -> OnnxError { OnnxError::Tract(e.to_string()) }

// ---- Registry --------------------------------------------------------

struct Registry {
    models:  HashMap<(u64, i64), RunnableModel>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry { models: HashMap::new(), next_id: 1 }))
}

fn handle_key(handle: i64) -> (u64, i64) { crate::native::runtime_handle_key(handle) }

fn insert(m: RunnableModel) -> i64 {
    let mut reg = registry().lock().expect("onnx registry poisoned");
    let id = reg.next_id;
    reg.next_id += 1;
    reg.models.insert(handle_key(id), m);
    id
}

fn with<F, R>(handle: i64, action: F) -> Result<R, OnnxError>
where F: FnOnce(&RunnableModel) -> Result<R, OnnxError> {
    let reg = registry().lock().expect("onnx registry poisoned");
    let m = reg.models.get(&handle_key(handle)).ok_or(OnnxError::UnknownHandle(handle))?;
    action(m)
}

// ---- Public API -----------------------------------------------------

/// Load a `.onnx` model from disk. Reads → parses → optimizes → makes
/// runnable in one shot; the returned handle is ready for `run_*`.
///
/// If the model has symbolic ("None") input shapes, callers should
/// prefer `load_with_input_shape` so tract can constant-fold shapes
/// before optimization.
pub fn load(path: &str) -> Result<i64, OnnxError> {
    let model = tract_onnx::onnx()
        .model_for_path(path).map_err(map_err)?
        .into_optimized().map_err(map_err)?
        .into_runnable().map_err(map_err)?;
    Ok(insert(model))
}

/// Same as `load` but pin the model's *first* input to `input_shape`
/// (of `f32`) before optimization. Necessary for models whose ONNX
/// graph exposes dynamic dimensions (batch, sequence length, ...).
pub fn load_with_input_shape(path: &str, input_shape: &[i64]) -> Result<i64, OnnxError> {
    if input_shape.iter().any(|&d| d <= 0) { return Err(OnnxError::BadShape); }
    let shape: Vec<usize> = input_shape.iter().map(|&d| d as usize).collect();
    let model = tract_onnx::onnx()
        .model_for_path(path).map_err(map_err)?
        .with_input_fact(0, f32::fact(&shape).into()).map_err(map_err)?
        .into_optimized().map_err(map_err)?
        .into_runnable().map_err(map_err)?;
    Ok(insert(model))
}

/// Load a BERT-family model with two `i64` inputs (`input_ids`,
/// `attention_mask`) both pinned to the same `[batch, seq_len]` shape.
/// This is the shape most HuggingFace transformer ONNX exports use.
pub fn load_bert_shape(path: &str, batch: i64, seq_len: i64) -> Result<i64, OnnxError> {
    if batch <= 0 || seq_len <= 0 { return Err(OnnxError::BadShape); }
    let shape = [batch as usize, seq_len as usize];
    let model = tract_onnx::onnx()
        .model_for_path(path).map_err(map_err)?
        .with_input_fact(0, i64::fact(&shape).into()).map_err(map_err)?
        .with_input_fact(1, i64::fact(&shape).into()).map_err(map_err)?
        .into_optimized().map_err(map_err)?
        .into_runnable().map_err(map_err)?;
    Ok(insert(model))
}

/// Load a HuggingFace transformer that expects three `i64` inputs
/// (`input_ids`, `attention_mask`, `token_type_ids`) — some BERT
/// variants (e.g. classic uncased BERT) need this third tensor.
pub fn load_bert3_shape(path: &str, batch: i64, seq_len: i64) -> Result<i64, OnnxError> {
    if batch <= 0 || seq_len <= 0 { return Err(OnnxError::BadShape); }
    let shape = [batch as usize, seq_len as usize];
    let model = tract_onnx::onnx()
        .model_for_path(path).map_err(map_err)?
        .with_input_fact(0, i64::fact(&shape).into()).map_err(map_err)?
        .with_input_fact(1, i64::fact(&shape).into()).map_err(map_err)?
        .with_input_fact(2, i64::fact(&shape).into()).map_err(map_err)?
        .into_optimized().map_err(map_err)?
        .into_runnable().map_err(map_err)?;
    Ok(insert(model))
}

/// Drop a model. Idempotent.
pub fn close(handle: i64) {
    if let Ok(mut reg) = registry().lock() { reg.models.remove(&handle_key(handle)); }
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
        let shape: Vec<i64> = fact.shape.iter().map(|d| d.to_i64().unwrap_or(-1)).collect();
        Ok(shape)
    })
}

/// Return the shape declared for output `i`.
pub fn output_shape(handle: i64, i: usize) -> Result<Vec<i64>, OnnxError> {
    with(handle, |m| {
        let fact = m.model().output_fact(i).map_err(map_err)?;
        let shape: Vec<i64> = fact.shape.iter().map(|d| d.to_i64().unwrap_or(-1)).collect();
        Ok(shape)
    })
}

/// Run the model with a **single** `f32` tensor input. Returns the
/// **first** output tensor as `(values, shape)` — flat row-major.
///
/// * `input_shape` — expected shape, e.g. `[1, 1, 28, 28]` for MNIST.
/// * `input_data`  — flat row-major buffer whose length must equal the
///                   product of `input_shape`.
pub fn run_f32(handle: i64, input_shape: &[i64], input_data: &[f32]) -> Result<(Vec<f32>, Vec<usize>), OnnxError> {
    if input_shape.iter().any(|&d| d <= 0) { return Err(OnnxError::BadShape); }
    let shape: Vec<usize> = input_shape.iter().map(|&d| d as usize).collect();
    let expected: usize = shape.iter().product();
    if input_data.len() != expected {
        return Err(OnnxError::ShapeMismatch { expected, found: input_data.len() });
    }

    with(handle, |m| {
        // Build the input tensor from the flat data + shape.
        let tensor = tract_ndarray::ArrayD::from_shape_vec(shape, input_data.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let outputs = m.run(tvec!(tensor.into())).map_err(map_err)?;
        let first = outputs.into_iter().next().ok_or_else(|| OnnxError::Tract("model produced no outputs".into()))?;
        let out_shape: Vec<usize> = first.shape().to_vec();
        let view = first.to_array_view::<f32>().map_err(map_err)?;
        let values: Vec<f32> = view.iter().copied().collect();
        Ok((values, out_shape))
    })
}

/// Run the model with a **single** `i64` tensor input (common for
/// token-id inputs to LLMs / BERT). Returns first output as `f32`.
pub fn run_i64_in_f32_out(handle: i64, input_shape: &[i64], input_data: &[i64]) -> Result<(Vec<f32>, Vec<usize>), OnnxError> {
    if input_shape.iter().any(|&d| d <= 0) { return Err(OnnxError::BadShape); }
    let shape: Vec<usize> = input_shape.iter().map(|&d| d as usize).collect();
    let expected: usize = shape.iter().product();
    if input_data.len() != expected {
        return Err(OnnxError::ShapeMismatch { expected, found: input_data.len() });
    }

    with(handle, |m| {
        let tensor = tract_ndarray::ArrayD::from_shape_vec(shape, input_data.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let outputs = m.run(tvec!(tensor.into())).map_err(map_err)?;
        let first = outputs.into_iter().next().ok_or_else(|| OnnxError::Tract("model produced no outputs".into()))?;
        let out_shape: Vec<usize> = first.shape().to_vec();
        let view = first.to_array_view::<f32>().map_err(map_err)?;
        let values: Vec<f32> = view.iter().copied().collect();
        Ok((values, out_shape))
    })
}

/// Run a transformer with **two** `i64` inputs of the same shape:
/// `input_ids` and `attention_mask`. Returns the first output as `f32`.
/// Perfect for DistilBERT / BERT-style classifiers.
pub fn run_two_i64(handle: i64, input_shape: &[i64], input_ids: &[i64], attention_mask: &[i64])
    -> Result<(Vec<f32>, Vec<usize>), OnnxError>
{
    if input_shape.iter().any(|&d| d <= 0) { return Err(OnnxError::BadShape); }
    let shape: Vec<usize> = input_shape.iter().map(|&d| d as usize).collect();
    let expected: usize = shape.iter().product();
    if input_ids.len() != expected {
        return Err(OnnxError::ShapeMismatch { expected, found: input_ids.len() });
    }
    if attention_mask.len() != expected {
        return Err(OnnxError::ShapeMismatch { expected, found: attention_mask.len() });
    }

    with(handle, |m| {
        let ids_tensor = tract_ndarray::ArrayD::from_shape_vec(shape.clone(), input_ids.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let mask_tensor = tract_ndarray::ArrayD::from_shape_vec(shape, attention_mask.to_vec())
            .map_err(map_err)?
            .into_tensor();
        let outputs = m.run(tvec!(ids_tensor.into(), mask_tensor.into())).map_err(map_err)?;
        let first = outputs.into_iter().next().ok_or_else(|| OnnxError::Tract("model produced no outputs".into()))?;
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
pub fn run_three_i64(handle: i64, input_shape: &[i64], input_ids: &[i64], attention_mask: &[i64], token_type_ids: &[i64])
    -> Result<(Vec<f32>, Vec<usize>), OnnxError>
{
    if input_shape.iter().any(|&d| d <= 0) { return Err(OnnxError::BadShape); }
    let shape: Vec<usize> = input_shape.iter().map(|&d| d as usize).collect();
    let expected: usize = shape.iter().product();
    for data in [input_ids, attention_mask, token_type_ids] {
        if data.len() != expected {
            return Err(OnnxError::ShapeMismatch { expected, found: data.len() });
        }
    }

    with(handle, |m| {
        let ids = tract_ndarray::ArrayD::from_shape_vec(shape.clone(), input_ids.to_vec()).map_err(map_err)?.into_tensor();
        let mask = tract_ndarray::ArrayD::from_shape_vec(shape.clone(), attention_mask.to_vec()).map_err(map_err)?.into_tensor();
        let types = tract_ndarray::ArrayD::from_shape_vec(shape, token_type_ids.to_vec()).map_err(map_err)?.into_tensor();
        let outputs = m.run(tvec!(ids.into(), mask.into(), types.into())).map_err(map_err)?;
        let first = outputs.into_iter().next().ok_or_else(|| OnnxError::Tract("model produced no outputs".into()))?;
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
    handle: i64, batch: i64, seq_len: i64,
    input_ids: &[i64], attention_mask: &[i64],
) -> Result<(Vec<f32>, Vec<usize>), OnnxError> {
    if batch <= 0 || seq_len <= 0 { return Err(OnnxError::BadShape); }
    let shape = [batch as usize, seq_len as usize];
    let expected: usize = shape.iter().product();
    if input_ids.len() != expected {
        return Err(OnnxError::ShapeMismatch { expected, found: input_ids.len() });
    }
    if attention_mask.len() != expected {
        return Err(OnnxError::ShapeMismatch { expected, found: attention_mask.len() });
    }

    with(handle, |m| {
        let ids  = tract_ndarray::ArrayD::from_shape_vec(shape.to_vec(), input_ids.to_vec()).map_err(map_err)?.into_tensor();
        let mask = tract_ndarray::ArrayD::from_shape_vec(shape.to_vec(), attention_mask.to_vec()).map_err(map_err)?.into_tensor();
        let outputs = m.run(tvec!(ids.into(), mask.into())).map_err(map_err)?;
        let first = outputs.into_iter().next().ok_or_else(|| OnnxError::Tract("model produced no outputs".into()))?;
        let out_shape: Vec<usize> = first.shape().to_vec();
        if out_shape.len() != 3 {
            return Err(OnnxError::Tract(format!("expected [batch, seq_len, hidden] output, got {out_shape:?}")));
        }
        let (b, s, h) = (out_shape[0], out_shape[1], out_shape[2]);
        if b != shape[0] || s != shape[1] {
            return Err(OnnxError::Tract(format!("output batch/seq mismatch: expected {shape:?}, got [{b},{s},{h}]")));
        }
        let view = first.to_array_view::<f32>().map_err(map_err)?;

        // Mean pooling weighted by attention_mask.
        // For each batch item, sum embeddings across tokens where
        // mask==1 and divide by count. This ignores [PAD] positions
        // and yields the standard sentence-transformer output.
        let mut pooled = vec![0.0f32; b * h];
        for bi in 0..b {
            let mut count = 0u32;
            for si in 0..s {
                let m_val = attention_mask[bi * s + si];
                if m_val == 0 { continue; }
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

    #[test]
    fn unknown_handle_reports_typed_error() {
        assert!(matches!(input_count(999_999), Err(OnnxError::UnknownHandle(_))));
        assert!(matches!(run_f32(999_999, &[1, 1, 1, 1], &[0.0]), Err(OnnxError::UnknownHandle(_))));
    }

    #[test]
    fn load_errors_on_missing_file() {
        assert!(load("/nonexistent/model.onnx").is_err());
    }

    #[test]
    fn bad_shapes_are_rejected() {
        assert!(matches!(load_with_input_shape("/nope.onnx", &[0, 1]), Err(OnnxError::BadShape)));
    }

    /// Live test opt-in: set TITAN_ONNX_MODEL=/path/to/model.onnx and
    /// TITAN_ONNX_SHAPE=1,1,28,28 (comma-separated).
    #[test]
    fn round_trip_when_configured() {
        let Ok(path) = std::env::var("TITAN_ONNX_MODEL") else { return; };
        let shape_str = std::env::var("TITAN_ONNX_SHAPE").unwrap_or_else(|_| "1,1,28,28".into());
        let shape: Vec<i64> = shape_str.split(',').map(|s| s.trim().parse().unwrap()).collect();
        let elems: usize = shape.iter().map(|&d| d as usize).product();

        let h = load_with_input_shape(&path, &shape).expect("load");
        let data = vec![0.0f32; elems];
        let (out, out_shape) = run_f32(h, &shape, &data).expect("run");
        assert!(!out.is_empty());
        assert!(!out_shape.is_empty());
        close(h);
    }
}
