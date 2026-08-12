//! URL router (`std::router::*`) powered by `matchit` 0.8 — the same
//! high-performance radix-tree router that lives at the heart of axum.
//!
//! Titan code creates one or more routers, inserts patterns with
//! optional named / catch-all parameters, and then matches an incoming
//! path to recover the pattern name plus a `{name: value, …}` map of
//! extracted parameters.
//!
//! ```titan
//! let r = std::router::new()
//! std::router::insert(r, "/users/{id}",         "show_user")
//! std::router::insert(r, "/posts/{id}/edit",    "edit_post")
//! std::router::insert(r, "/static/{*rest}",     "static_files")
//!
//! let m = std::router::at(r, "/users/42")
//! # -> {"pattern": "show_user", "params": {"id": "42"}}
//! ```
//!
//! Handles are opaque `i64` values so multiple routers can coexist per
//! process (one for HTTP routes, one for WS routes, one for admin, …).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

use matchit::Router;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("router error: {0}")]
    Insert(String),
    #[error("unknown router handle {0}")]
    UnknownHandle(i64),
}

struct Registry {
    routers: HashMap<(u64, i64), Router<String>>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry { routers: HashMap::new(), next_id: 1 }))
}

fn handle_key(handle: i64) -> (u64, i64) { crate::native::runtime_handle_key(handle) }

/// Create a fresh empty router. Returns an opaque handle.
pub fn new() -> i64 {
    let mut r = registry().lock().expect("router registry poisoned");
    let id = r.next_id; r.next_id += 1;
    r.routers.insert(handle_key(id), Router::new());
    id
}

/// Drop a router. Idempotent.
pub fn drop_router(handle: i64) {
    if let Ok(mut r) = registry().lock() { r.routers.remove(&handle_key(handle)); }
}

/// Register a route. `pattern` follows matchit syntax:
///
/// * `/users` — static segment
/// * `/users/{id}` — named parameter
/// * `/files/{*rest}` — catch-all (must be the last segment)
///
/// `value` is an arbitrary string tag returned by `at()` when the
/// pattern matches — typically a handler name.
pub fn insert(handle: i64, pattern: &str, value: &str) -> Result<(), RouterError> {
    let mut r = registry().lock().expect("router registry poisoned");
    let router = r.routers.get_mut(&handle_key(handle)).ok_or(RouterError::UnknownHandle(handle))?;
    router.insert(pattern.to_string(), value.to_string())
        .map_err(|e| RouterError::Insert(e.to_string()))?;
    Ok(())
}

/// Look up `path` in the router. Returns `Some((tag, params))` if a
/// route matched, `None` otherwise. `params` is an ordered map of
/// extracted names to values (percent-decoded by matchit).
pub fn at(handle: i64, path: &str) -> Result<Option<(String, BTreeMap<String, String>)>, RouterError> {
    let r = registry().lock().expect("router registry poisoned");
    let router = r.routers.get(&handle_key(handle)).ok_or(RouterError::UnknownHandle(handle))?;
    match router.at(path) {
        Ok(m) => {
            let mut params = BTreeMap::new();
            for (k, v) in m.params.iter() {
                params.insert(k.to_string(), v.to_string());
            }
            Ok(Some((m.value.to_string(), params)))
        }
        Err(_) => Ok(None),
    }
}

/// Convenience: match `path` and return `true` iff any route matches
/// (no parameters returned). Handy for feature flags.
pub fn matches(handle: i64, path: &str) -> Result<bool, RouterError> {
    let r = registry().lock().expect("router registry poisoned");
    let router = r.routers.get(&handle_key(handle)).ok_or(RouterError::UnknownHandle(handle))?;
    Ok(router.at(path).is_ok())
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut reg = crate::native::lock_recover(registry());
    crate::native::remove_runtime_entries(&mut reg.routers, runtime_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_match_static_and_named() {
        let r = new();
        insert(r, "/",              "root").unwrap();
        insert(r, "/users/{id}",    "show_user").unwrap();
        insert(r, "/files/{*rest}", "files").unwrap();

        let (tag, p) = at(r, "/").unwrap().expect("root matches");
        assert_eq!(tag, "root");
        assert!(p.is_empty());

        let (tag, p) = at(r, "/users/42").unwrap().expect("user matches");
        assert_eq!(tag, "show_user");
        assert_eq!(p.get("id").map(String::as_str), Some("42"));

        let (tag, p) = at(r, "/files/a/b/c.txt").unwrap().expect("catch-all matches");
        assert_eq!(tag, "files");
        assert_eq!(p.get("rest").map(String::as_str), Some("a/b/c.txt"));

        assert!(at(r, "/nope").unwrap().is_none());
        drop_router(r);
    }

    #[test]
    fn duplicate_pattern_reports_typed_error() {
        let r = new();
        insert(r, "/x", "one").unwrap();
        assert!(insert(r, "/x", "two").is_err());
        drop_router(r);
    }

    #[test]
    fn unknown_handle_reports_typed_error() {
        assert!(matches!(at(999_999, "/"), Err(RouterError::UnknownHandle(_))));
    }
}
