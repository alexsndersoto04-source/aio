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
use std::sync::{Mutex, OnceLock};

use thiserror::Error;
use tokenizers::Tokenizer;

#[derive(Debug, Error)]
pub enum TokenizeError {
    #[error("tokenizer error: {0}")]
    Backend(String),
    #[error("unknown tokenizer handle {0}")]
    UnknownHandle(i64),
}

/// A single-sentence encoding result surfaced to `.titan`.
pub struct Encoding {
    pub ids:            Vec<u32>,
    pub tokens:         Vec<String>,
    pub type_ids:       Vec<u32>,
    pub attention_mask: Vec<u32>,
    pub special_tokens_mask: Vec<u32>,
}

// ---- Registry --------------------------------------------------------

struct Registry {
    tokenizers: HashMap<i64, Tokenizer>,
    next_id:    i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry { tokenizers: HashMap::new(), next_id: 1 }))
}

fn insert(t: Tokenizer) -> i64 {
    let mut reg = registry().lock().expect("tokenize registry poisoned");
    let id = reg.next_id;
    reg.next_id += 1;
    reg.tokenizers.insert(id, t);
    id
}

fn with<F, R>(handle: i64, action: F) -> Result<R, TokenizeError>
where F: FnOnce(&Tokenizer) -> Result<R, TokenizeError> {
    let reg = registry().lock().expect("tokenize registry poisoned");
    let t = reg.tokenizers.get(&handle).ok_or(TokenizeError::UnknownHandle(handle))?;
    action(t)
}

// ---- Public API -----------------------------------------------------

/// Load a HuggingFace `tokenizer.json` from `path`. Returns an opaque
/// handle stored in a process-wide registry.
pub fn load(path: &str) -> Result<i64, TokenizeError> {
    let tokenizer = Tokenizer::from_file(path).map_err(|e| TokenizeError::Backend(e.to_string()))?;
    Ok(insert(tokenizer))
}

/// Load a tokenizer from a raw JSON string (handy when the tokenizer
/// definition is embedded in another file or fetched over HTTP).
pub fn from_json(json: &str) -> Result<i64, TokenizeError> {
    let tokenizer: Tokenizer = json.parse().map_err(|e: tokenizers::Error| TokenizeError::Backend(e.to_string()))?;
    Ok(insert(tokenizer))
}

/// Drop a tokenizer. Idempotent.
pub fn close(handle: i64) {
    if let Ok(mut reg) = registry().lock() { reg.tokenizers.remove(&handle); }
}

/// Encode a single `text`. `add_special_tokens` controls whether
/// CLS/SEP-like markers are appended (usually `true` for BERT-style
/// models, `false` for GPT-style tokenizers when concatenating).
pub fn encode(handle: i64, text: &str, add_special_tokens: bool) -> Result<Encoding, TokenizeError> {
    with(handle, |t| {
        let e = t.encode(text, add_special_tokens).map_err(|e| TokenizeError::Backend(e.to_string()))?;
        Ok(Encoding {
            ids:            e.get_ids().to_vec(),
            tokens:         e.get_tokens().to_vec(),
            type_ids:       e.get_type_ids().to_vec(),
            attention_mask: e.get_attention_mask().to_vec(),
            special_tokens_mask: e.get_special_tokens_mask().to_vec(),
        })
    })
}

/// Encode a batch of texts in one call. Uses the tokenizer's internal
/// parallelism (rayon) when the batch is large.
pub fn encode_batch(handle: i64, texts: &[String], add_special_tokens: bool) -> Result<Vec<Encoding>, TokenizeError> {
    with(handle, |t| {
        let inputs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let batch = t.encode_batch(inputs, add_special_tokens).map_err(|e| TokenizeError::Backend(e.to_string()))?;
        Ok(batch.into_iter().map(|e| Encoding {
            ids:            e.get_ids().to_vec(),
            tokens:         e.get_tokens().to_vec(),
            type_ids:       e.get_type_ids().to_vec(),
            attention_mask: e.get_attention_mask().to_vec(),
            special_tokens_mask: e.get_special_tokens_mask().to_vec(),
        }).collect())
    })
}

/// Decode a sequence of `ids` back into a string. `skip_special_tokens`
/// drops CLS/SEP/PAD/etc. from the output — usually what you want when
/// showing the text to a user.
pub fn decode(handle: i64, ids: &[u32], skip_special_tokens: bool) -> Result<String, TokenizeError> {
    with(handle, |t| {
        t.decode(ids, skip_special_tokens).map_err(|e| TokenizeError::Backend(e.to_string()))
    })
}

/// Total vocabulary size (with added tokens if any).
pub fn vocab_size(handle: i64) -> Result<u32, TokenizeError> {
    with(handle, |t| Ok(t.get_vocab_size(true) as u32))
}

/// Convert a single token string to its numeric id, if present.
pub fn token_to_id(handle: i64, token: &str) -> Result<Option<u32>, TokenizeError> {
    with(handle, |t| Ok(t.token_to_id(token)))
}

/// Convert a numeric id back to its token string, if present.
pub fn id_to_token(handle: i64, id: u32) -> Result<Option<String>, TokenizeError> {
    with(handle, |t| Ok(t.id_to_token(id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_handle_reports_typed_error() {
        assert!(matches!(vocab_size(999_999), Err(TokenizeError::UnknownHandle(_))));
        assert!(matches!(encode(999_999, "x", true), Err(TokenizeError::UnknownHandle(_))));
    }

    #[test]
    fn load_errors_on_missing_file() {
        let r = load("/nonexistent/path/tokenizer.json");
        assert!(r.is_err());
    }

    /// Live test opt-in: set TITAN_TOKENIZER_JSON=/path/to/tokenizer.json
    #[test]
    fn round_trip_when_configured() {
        let Ok(path) = std::env::var("TITAN_TOKENIZER_JSON") else { return; };
        let h = load(&path).expect("load");
        assert!(vocab_size(h).unwrap() > 100);
        let e = encode(h, "hello world", true).expect("encode");
        assert!(!e.ids.is_empty());
        let decoded = decode(h, &e.ids, true).expect("decode");
        assert!(decoded.to_lowercase().contains("hello"));
        close(h);
    }
}
