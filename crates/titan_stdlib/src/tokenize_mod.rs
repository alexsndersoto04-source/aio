//! HuggingFace tokenizers (`std::tokenize::*`) — pure-Rust build.
//!
//! Wraps the official `tokenizers` crate (the same one PyTorch/HuggingFace
//! ships in Python), configured **without** the default `esaxx_fast`,
//! `onig` or `progressbar` features so no C++/C libraries sneak into the
//! Termux build. Regex fallback uses `fancy-regex` (pure Rust), which
//! covers everything BPE/WordPiece/Unigram tokenizers need.
//!
//! ## What it does
//!
//! Given a `tokenizer.json` file (the artefact HuggingFace publishes
//! alongside every model — BERT, GPT-2, MiniLM, Llama tokenizer, ...),
//! this module lets `.titan` code:
//!
//! * Load the tokenizer once (`load(path) → handle`).
//! * Encode a string into ids / tokens / attention masks.
//! * Encode a batch of strings.
//! * Decode ids back into text.
//! * Ask for the vocabulary size.
//!
//! ## Getting a `tokenizer.json`
//!
//! ```bash
//! # Any modern HuggingFace repo ships one, e.g. sentence-transformers/all-MiniLM-L6-v2
//! curl -L -o tokenizer.json \
//!   https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json
//! ```
//!
//! ## Example (Rust API)
//!
//! ```rust,ignore
//! use titan_stdlib::tokenize_mod;
//! let h = tokenize_mod::load("tokenizer.json")?;
//! let enc = tokenize_mod::encode(h, "hola mundo desde Titan!", true)?;
//! assert!(!enc.ids.is_empty());
//! tokenize_mod::close(h);
//! ```

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex, OnceLock};

use thiserror::Error;
use tokenizers::Tokenizer;

const MAX_TOKENIZER_HANDLES: usize = 16;
const MAX_TOKENIZER_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_RUNTIME_TOKENIZER_SOURCE_BYTES: usize = 64 * 1024 * 1024;
const MAX_TOKENIZER_PATH_BYTES: usize = 16 * 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_BATCH_TEXTS: usize = 64;
const MAX_BATCH_TEXT_BYTES: usize = 256 * 1024;
const MAX_TOKENS_PER_ENCODING: usize = 131_072;
const MAX_BATCH_TOKENS: usize = 262_144;
const MAX_ENCODING_TOKEN_BYTES: usize = 8 * 1024 * 1024;
const MAX_BATCH_TOKEN_BYTES: usize = 16 * 1024 * 1024;
const MAX_PADDED_LENGTH: usize = 65_536;
const MAX_DECODE_IDS: usize = 131_072;
const MAX_DECODED_TEXT_BYTES: usize = 1024 * 1024;
const MAX_TOKEN_BYTES: usize = 256;
const MAX_VOCAB_SIZE: usize = 262_144;
const MAX_CONCURRENT_TOKENIZER_OPERATIONS: usize = 2;

#[derive(Debug, Error)]
pub enum TokenizeError {
    #[error("tokenizer error: {0}")]
    Backend(String),
    #[error("unknown tokenizer handle {0}")]
    UnknownHandle(i64),
    #[error("tokenizer I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{resource} exceeds limit {limit}")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    #[error("invalid tokenizer argument: {0}")]
    InvalidArgument(&'static str),
    #[error("tokenizer handle space exhausted")]
    HandleSpaceExhausted,
}

/// A single-sentence encoding result surfaced to `.titan`.
pub struct Encoding {
    pub ids: Vec<u32>,
    pub tokens: Vec<String>,
    pub type_ids: Vec<u32>,
    pub attention_mask: Vec<u32>,
    pub special_tokens_mask: Vec<u32>,
}

// ---- Registry --------------------------------------------------------

struct TokenizerEntry {
    tokenizer: Arc<Tokenizer>,
    source_bytes: usize,
}

struct Registry {
    tokenizers: HashMap<(u64, i64), TokenizerEntry>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        Mutex::new(Registry {
            tokenizers: HashMap::new(),
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

fn reserve_operation() -> Result<OperationPermit, TokenizeError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut usage = crate::native::lock_recover(operation_usage());
    let active = usage.get(&runtime_id).map_or(0, |runtime| runtime.active);
    if active >= MAX_CONCURRENT_TOKENIZER_OPERATIONS {
        return Err(TokenizeError::ResourceLimit {
            resource: "concurrent tokenizer operations",
            limit: MAX_CONCURRENT_TOKENIZER_OPERATIONS,
        });
    }
    usage.entry(runtime_id).or_default().active += 1;
    Ok(OperationPermit { runtime_id })
}

fn validate_capacity(active: usize, runtime_bytes: usize, source_bytes: usize) -> Result<(), TokenizeError> {
    if active >= MAX_TOKENIZER_HANDLES {
        return Err(TokenizeError::ResourceLimit {
            resource: "tokenizer handles",
            limit: MAX_TOKENIZER_HANDLES,
        });
    }
    if source_bytes > MAX_TOKENIZER_SOURCE_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "tokenizer source bytes",
            limit: MAX_TOKENIZER_SOURCE_BYTES,
        });
    }
    if runtime_bytes.saturating_add(source_bytes) > MAX_RUNTIME_TOKENIZER_SOURCE_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "runtime tokenizer source bytes",
            limit: MAX_RUNTIME_TOKENIZER_SOURCE_BYTES,
        });
    }
    Ok(())
}

fn current_capacity() -> (usize, usize) {
    let runtime_id = crate::native::current_runtime_id();
    let registry = crate::native::lock_recover(registry());
    registry
        .tokenizers
        .iter()
        .filter(|((owner, _), _)| *owner == runtime_id)
        .fold((0usize, 0usize), |(handles, bytes), (_, entry)| {
            (handles.saturating_add(1), bytes.saturating_add(entry.source_bytes))
        })
}

fn preflight_load(source_bytes: usize) -> Result<(), TokenizeError> {
    let (active, runtime_bytes) = current_capacity();
    validate_capacity(active, runtime_bytes, source_bytes)
}

fn insert(tokenizer: Tokenizer, source_bytes: usize) -> Result<i64, TokenizeError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(registry());
    let (active, runtime_bytes) = registry
        .tokenizers
        .iter()
        .filter(|((owner, _), _)| *owner == runtime_id)
        .fold((0usize, 0usize), |(handles, bytes), (_, entry)| {
            (handles.saturating_add(1), bytes.saturating_add(entry.source_bytes))
        });
    validate_capacity(active, runtime_bytes, source_bytes)?;
    let id = registry.next_id;
    registry.next_id = id
        .checked_add(1)
        .ok_or(TokenizeError::HandleSpaceExhausted)?;
    registry.tokenizers.insert(
        (runtime_id, id),
        TokenizerEntry {
            tokenizer: Arc::new(tokenizer),
            source_bytes,
        },
    );
    Ok(id)
}

fn get(handle: i64) -> Result<Arc<Tokenizer>, TokenizeError> {
    crate::native::lock_recover(registry())
        .tokenizers
        .get(&handle_key(handle))
        .map(|entry| Arc::clone(&entry.tokenizer))
        .ok_or(TokenizeError::UnknownHandle(handle))
}

fn validate_text(text: &str) -> Result<(), TokenizeError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "tokenizer input text bytes",
            limit: MAX_TEXT_BYTES,
        });
    }
    Ok(())
}

fn encoding_size(encoding: &tokenizers::Encoding) -> Result<(usize, usize), TokenizeError> {
    let tokens = encoding.len();
    let token_bytes = encoding.get_tokens().iter().try_fold(0usize, |total, token| {
        total.checked_add(token.len()).ok_or(TokenizeError::ResourceLimit {
            resource: "encoded token bytes",
            limit: MAX_ENCODING_TOKEN_BYTES,
        })
    })?;
    Ok((tokens, token_bytes))
}

fn convert_encoding(encoding: tokenizers::Encoding) -> Result<Encoding, TokenizeError> {
    let (tokens, token_bytes) = encoding_size(&encoding)?;
    if tokens > MAX_TOKENS_PER_ENCODING {
        return Err(TokenizeError::ResourceLimit {
            resource: "tokens per encoding",
            limit: MAX_TOKENS_PER_ENCODING,
        });
    }
    if token_bytes > MAX_ENCODING_TOKEN_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "encoded token bytes",
            limit: MAX_ENCODING_TOKEN_BYTES,
        });
    }
    Ok(Encoding {
        ids: encoding.get_ids().to_vec(),
        tokens: encoding.get_tokens().to_vec(),
        type_ids: encoding.get_type_ids().to_vec(),
        attention_mask: encoding.get_attention_mask().to_vec(),
        special_tokens_mask: encoding.get_special_tokens_mask().to_vec(),
    })
}

fn validate_tokenizer(tokenizer: &Tokenizer) -> Result<(), TokenizeError> {
    if tokenizer.get_vocab_size(true) > MAX_VOCAB_SIZE {
        return Err(TokenizeError::ResourceLimit {
            resource: "tokenizer vocabulary entries",
            limit: MAX_VOCAB_SIZE,
        });
    }
    if tokenizer
        .get_vocab(true)
        .keys()
        .any(|token| token.len() > MAX_TOKEN_BYTES)
    {
        return Err(TokenizeError::ResourceLimit {
            resource: "vocabulary token bytes",
            limit: MAX_TOKEN_BYTES,
        });
    }
    Ok(())
}

fn read_source(path: &str) -> Result<Vec<u8>, TokenizeError> {
    let file = std::fs::File::open(path)?;
    let mut source = Vec::new();
    file.take((MAX_TOKENIZER_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut source)?;
    if source.len() > MAX_TOKENIZER_SOURCE_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "tokenizer source bytes",
            limit: MAX_TOKENIZER_SOURCE_BYTES,
        });
    }
    Ok(source)
}

// ---- Public API -----------------------------------------------------

/// Load a HuggingFace `tokenizer.json` from `path`. Returns an opaque
/// handle stored in a process-wide registry.
pub fn load(path: &str) -> Result<i64, TokenizeError> {
    if path.len() > MAX_TOKENIZER_PATH_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "tokenizer path bytes",
            limit: MAX_TOKENIZER_PATH_BYTES,
        });
    }
    let source = read_source(path)?;
    preflight_load(source.len())?;
    let json = std::str::from_utf8(&source)
        .map_err(|_| TokenizeError::InvalidArgument("tokenizer JSON is not UTF-8"))?;
    let _permit = reserve_operation()?;
    let tokenizer: Tokenizer = json
        .parse()
        .map_err(|error: tokenizers::Error| TokenizeError::Backend(error.to_string()))?;
    validate_tokenizer(&tokenizer)?;
    insert(tokenizer, source.len())
}

/// Load a tokenizer from a raw JSON string (handy when the tokenizer
/// definition is embedded in another file or fetched over HTTP).
pub fn from_json(json: &str) -> Result<i64, TokenizeError> {
    preflight_load(json.len())?;
    let _permit = reserve_operation()?;
    let tokenizer: Tokenizer = json
        .parse()
        .map_err(|error: tokenizers::Error| TokenizeError::Backend(error.to_string()))?;
    validate_tokenizer(&tokenizer)?;
    insert(tokenizer, json.len())
}

/// Drop a tokenizer. Idempotent.
pub fn close(handle: i64) {
    crate::native::lock_recover(registry())
        .tokenizers
        .remove(&handle_key(handle));
}

/// Encode a single `text`. `add_special_tokens` controls whether
/// CLS/SEP-like markers are appended (usually `true` for BERT-style
/// models, `false` for GPT-style tokenizers when concatenating).
pub fn encode(
    handle: i64,
    text: &str,
    add_special_tokens: bool,
) -> Result<Encoding, TokenizeError> {
    validate_text(text)?;
    let _permit = reserve_operation()?;
    let tokenizer = get(handle)?;
    let encoding = tokenizer
        .encode(text, add_special_tokens)
        .map_err(|error| TokenizeError::Backend(error.to_string()))?;
    convert_encoding(encoding)
}

/// Encode `text` and pad / truncate to exactly `max_length` tokens.
/// Padding uses `pad_id` for `ids` / `type_ids` / `special_tokens_mask`
/// and `0` for the `attention_mask` so downstream models correctly
/// ignore padded positions. Perfect for BERT-family transformers where
/// the ONNX graph is compiled for a fixed sequence length.
pub fn encode_padded(
    handle: i64,
    text: &str,
    max_length: usize,
    pad_id: u32,
    add_special_tokens: bool,
) -> Result<Encoding, TokenizeError> {
    validate_text(text)?;
    if max_length == 0 {
        return Err(TokenizeError::InvalidArgument(
            "padded token length must be positive",
        ));
    }
    if max_length > MAX_PADDED_LENGTH {
        return Err(TokenizeError::ResourceLimit {
            resource: "padded token length",
            limit: MAX_PADDED_LENGTH,
        });
    }
    let _permit = reserve_operation()?;
    let tokenizer = get(handle)?;
    let encoding = tokenizer
        .encode(text, add_special_tokens)
        .map_err(|error| TokenizeError::Backend(error.to_string()))?;
    if encoding.len() > MAX_TOKENS_PER_ENCODING {
        return Err(TokenizeError::ResourceLimit {
            resource: "raw tokens before padding",
            limit: MAX_TOKENS_PER_ENCODING,
        });
    }
    let mut ids = encoding.get_ids().to_vec();
    let mut tokens = encoding.get_tokens().to_vec();
    let mut types = encoding.get_type_ids().to_vec();
    let mut mask = encoding.get_attention_mask().to_vec();
    let mut special = encoding.get_special_tokens_mask().to_vec();

    if ids.len() > max_length {
        let last = ids.last().copied();
        let last_token = tokens.last().cloned();
        let last_type = types.last().copied();
        let last_mask = mask.last().copied();
        let last_special = special.last().copied();
        ids.truncate(max_length);
        tokens.truncate(max_length);
        types.truncate(max_length);
        mask.truncate(max_length);
        special.truncate(max_length);
        if add_special_tokens {
            if let Some(value) = last {
                ids[max_length - 1] = value;
            }
            if let Some(value) = last_token {
                tokens[max_length - 1] = value;
            }
            if let Some(value) = last_type {
                types[max_length - 1] = value;
            }
            if let Some(value) = last_mask {
                mask[max_length - 1] = value;
            }
            if let Some(value) = last_special {
                special[max_length - 1] = value;
            }
        }
    } else {
        let padding = max_length - ids.len();
        ids.extend(std::iter::repeat(pad_id).take(padding));
        tokens.extend(std::iter::repeat("[PAD]".to_string()).take(padding));
        types.extend(std::iter::repeat(0).take(padding));
        mask.extend(std::iter::repeat(0).take(padding));
        special.extend(std::iter::repeat(1).take(padding));
    }

    let token_bytes = tokens
        .iter()
        .fold(0usize, |total, token| total.saturating_add(token.len()));
    if token_bytes > MAX_ENCODING_TOKEN_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "encoded token bytes",
            limit: MAX_ENCODING_TOKEN_BYTES,
        });
    }
    Ok(Encoding {
        ids,
        tokens,
        type_ids: types,
        attention_mask: mask,
        special_tokens_mask: special,
    })
}

/// Encode a batch of texts in one call. Uses the tokenizer's internal
/// parallelism (rayon) when the batch is large.
pub fn encode_batch(
    handle: i64,
    texts: &[String],
    add_special_tokens: bool,
) -> Result<Vec<Encoding>, TokenizeError> {
    if texts.len() > MAX_BATCH_TEXTS {
        return Err(TokenizeError::ResourceLimit {
            resource: "tokenizer batch items",
            limit: MAX_BATCH_TEXTS,
        });
    }
    let input_bytes = texts.iter().try_fold(0usize, |total, text| {
        validate_text(text)?;
        total
            .checked_add(text.len())
            .ok_or(TokenizeError::ResourceLimit {
                resource: "tokenizer batch input bytes",
                limit: MAX_BATCH_TEXT_BYTES,
            })
    })?;
    if input_bytes > MAX_BATCH_TEXT_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "tokenizer batch input bytes",
            limit: MAX_BATCH_TEXT_BYTES,
        });
    }
    let _permit = reserve_operation()?;
    let tokenizer = get(handle)?;
    let inputs = texts.iter().map(String::as_str).collect::<Vec<_>>();
    let batch = tokenizer
        .encode_batch(inputs, add_special_tokens)
        .map_err(|error| TokenizeError::Backend(error.to_string()))?;
    let (tokens, token_bytes) = batch.iter().try_fold(
        (0usize, 0usize),
        |(total_tokens, total_bytes), encoding| {
            let (encoding_tokens, encoding_bytes) = encoding_size(encoding)?;
            Ok::<_, TokenizeError>((
                total_tokens.saturating_add(encoding_tokens),
                total_bytes.saturating_add(encoding_bytes),
            ))
        },
    )?;
    if tokens > MAX_BATCH_TOKENS {
        return Err(TokenizeError::ResourceLimit {
            resource: "tokens per batch",
            limit: MAX_BATCH_TOKENS,
        });
    }
    if token_bytes > MAX_BATCH_TOKEN_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "encoded batch token bytes",
            limit: MAX_BATCH_TOKEN_BYTES,
        });
    }
    batch.into_iter().map(convert_encoding).collect()
}

/// Decode a sequence of `ids` back into a string. `skip_special_tokens`
/// drops CLS/SEP/PAD/etc. from the output — usually what you want when
/// showing the text to a user.
pub fn decode(
    handle: i64,
    ids: &[u32],
    skip_special_tokens: bool,
) -> Result<String, TokenizeError> {
    if ids.len() > MAX_DECODE_IDS {
        return Err(TokenizeError::ResourceLimit {
            resource: "token ids to decode",
            limit: MAX_DECODE_IDS,
        });
    }
    let _permit = reserve_operation()?;
    let tokenizer = get(handle)?;
    let text = tokenizer
        .decode(ids, skip_special_tokens)
        .map_err(|error| TokenizeError::Backend(error.to_string()))?;
    if text.len() > MAX_DECODED_TEXT_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "decoded tokenizer text bytes",
            limit: MAX_DECODED_TEXT_BYTES,
        });
    }
    Ok(text)
}

/// Total vocabulary size (with added tokens if any).
pub fn vocab_size(handle: i64) -> Result<u32, TokenizeError> {
    u32::try_from(get(handle)?.get_vocab_size(true))
        .map_err(|_| TokenizeError::Backend("vocabulary does not fit in u32".to_string()))
}

/// Convert a single token string to its numeric id, if present.
pub fn token_to_id(handle: i64, token: &str) -> Result<Option<u32>, TokenizeError> {
    if token.len() > MAX_TOKEN_BYTES {
        return Err(TokenizeError::ResourceLimit {
            resource: "token bytes",
            limit: MAX_TOKEN_BYTES,
        });
    }
    Ok(get(handle)?.token_to_id(token))
}

/// Convert a numeric id back to its token string, if present.
pub fn id_to_token(handle: i64, id: u32) -> Result<Option<String>, TokenizeError> {
    let token = get(handle)?.id_to_token(id);
    if token.as_ref().is_some_and(|token| token.len() > MAX_TOKEN_BYTES) {
        return Err(TokenizeError::ResourceLimit {
            resource: "token bytes",
            limit: MAX_TOKEN_BYTES,
        });
    }
    Ok(token)
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut reg = crate::native::lock_recover(registry());
    crate::native::remove_runtime_entries(&mut reg.tokenizers, runtime_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_TOKENIZER: &str = r#"{
        "version":"1.0",
        "truncation":null,
        "padding":null,
        "added_tokens":[],
        "normalizer":null,
        "pre_tokenizer":{"type":"Whitespace"},
        "post_processor":null,
        "decoder":null,
        "model":{
            "type":"WordLevel",
            "vocab":{"[UNK]":0,"hello":1,"world":2},
            "unk_token":"[UNK]"
        }
    }"#;

    #[test]
    fn handles_inputs_padding_and_operations_are_bounded() {
        assert!(matches!(
            validate_capacity(0, MAX_RUNTIME_TOKENIZER_SOURCE_BYTES, 1),
            Err(TokenizeError::ResourceLimit {
                resource: "runtime tokenizer source bytes",
                ..
            })
        ));
        assert!(matches!(
            encode(999_999, &"x".repeat(MAX_TEXT_BYTES + 1), false),
            Err(TokenizeError::ResourceLimit { .. })
        ));
        assert!(matches!(
            encode_batch(
                999_999,
                &vec![String::new(); MAX_BATCH_TEXTS + 1],
                false
            ),
            Err(TokenizeError::ResourceLimit { .. })
        ));

        let runtime_id = 8_300_008;
        crate::native::with_runtime_context(runtime_id, || {
            let mut handles = (0..MAX_TOKENIZER_HANDLES)
                .map(|_| from_json(TEST_TOKENIZER).unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                from_json(TEST_TOKENIZER),
                Err(TokenizeError::ResourceLimit {
                    resource: "tokenizer handles",
                    ..
                })
            ));
            assert!(matches!(
                encode_padded(handles[0], "hello", 0, 0, false),
                Err(TokenizeError::InvalidArgument(_))
            ));
            let encoding = encode_padded(handles[0], "hello world", 4, 0, false).unwrap();
            assert_eq!(encoding.ids.len(), 4);
            close(handles.pop().unwrap());
            handles.push(from_json(TEST_TOKENIZER).unwrap());
        });
        assert_eq!(cleanup_runtime(runtime_id), MAX_TOKENIZER_HANDLES);

        crate::native::with_runtime_context(runtime_id, || {
            let permits = (0..MAX_CONCURRENT_TOKENIZER_OPERATIONS)
                .map(|_| reserve_operation().unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                reserve_operation(),
                Err(TokenizeError::ResourceLimit {
                    resource: "concurrent tokenizer operations",
                    ..
                })
            ));
            drop(permits);
        });
        assert!(!crate::native::lock_recover(operation_usage()).contains_key(&runtime_id));
    }

    #[test]
    fn unknown_handle_reports_typed_error() {
        assert!(matches!(
            vocab_size(999_999),
            Err(TokenizeError::UnknownHandle(_))
        ));
        assert!(matches!(
            encode(999_999, "x", true),
            Err(TokenizeError::UnknownHandle(_))
        ));
    }

    #[test]
    fn load_errors_on_missing_file() {
        let r = load("/nonexistent/path/tokenizer.json");
        assert!(r.is_err());
    }

    /// Live test opt-in: set TITAN_TOKENIZER_JSON=/path/to/tokenizer.json
    #[test]
    fn round_trip_when_configured() {
        let Ok(path) = std::env::var("TITAN_TOKENIZER_JSON") else {
            return;
        };
        let h = load(&path).expect("load");
        assert!(vocab_size(h).unwrap() > 100);
        let e = encode(h, "hello world", true).expect("encode");
        assert!(!e.ids.is_empty());
        let decoded = decode(h, &e.ids, true).expect("decode");
        assert!(decoded.to_lowercase().contains("hello"));
        close(h);
    }
}
