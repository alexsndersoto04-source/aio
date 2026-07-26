//! Embedded key-value store (`std::kv::*`) powered by `sled`.
//!
//! `sled` is a pure-Rust, ACID, log-structured B-tree that persists a
//! whole database to a single directory on disk. Ideal for caches,
//! sessions, counters, feature flags and other "I just need to save
//! things without a server" needs. Works everywhere Rust builds — no
//! native deps.
//!
//! `.titan` uses this module through opaque `i64` handles kept in a
//! process-wide registry so several databases and trees can coexist.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use sled::{Db, IVec, Tree};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KvError {
    #[error("sled error: {0}")]
    Sled(#[from] sled::Error),
    #[error("unknown database handle {0}")]
    UnknownDb(i64),
    #[error("unknown tree handle {0}")]
    UnknownTree(i64),
    #[error("bytes for '{0}' are not valid UTF-8")]
    Utf8(&'static str),
}

struct Registry {
    dbs:    HashMap<i64, Db>,
    trees:  HashMap<i64, Tree>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry { dbs: HashMap::new(), trees: HashMap::new(), next_id: 1 }))
}

fn insert_db(db: Db) -> i64 {
    let mut reg = registry().lock().expect("kv registry poisoned");
    let id = reg.next_id;
    reg.next_id += 1;
    reg.dbs.insert(id, db);
    id
}

fn insert_tree(tree: Tree) -> i64 {
    let mut reg = registry().lock().expect("kv registry poisoned");
    let id = reg.next_id;
    reg.next_id += 1;
    reg.trees.insert(id, tree);
    id
}

fn with_db<F, R>(handle: i64, action: F) -> Result<R, KvError>
where F: FnOnce(&Db) -> Result<R, KvError> {
    let reg = registry().lock().expect("kv registry poisoned");
    let db = reg.dbs.get(&handle).ok_or(KvError::UnknownDb(handle))?;
    action(db)
}

fn with_tree<F, R>(handle: i64, action: F) -> Result<R, KvError>
where F: FnOnce(&Tree) -> Result<R, KvError> {
    let reg = registry().lock().expect("kv registry poisoned");
    let tree = reg.trees.get(&handle).ok_or(KvError::UnknownTree(handle))?;
    action(tree)
}

fn ivec_to_bytes(v: IVec) -> Vec<u8> { v.to_vec() }

// ---------------- Open / close ----------------------------------------

/// Open (or create) a database at `path`. Returns an opaque handle.
pub fn open(path: &str) -> Result<i64, KvError> {
    let db = sled::open(path)?;
    Ok(insert_db(db))
}

/// Close a database. Flushes to disk. Idempotent.
pub fn close(handle: i64) -> Result<(), KvError> {
    let mut reg = registry().lock().expect("kv registry poisoned");
    if let Some(db) = reg.dbs.remove(&handle) {
        // Best-effort flush before drop.
        let _ = db.flush();
    }
    // Also drop any trees that came from this db is not possible without
    // tracking them; `sled::Tree` is a lightweight handle, so leaking it
    // for the rest of the process is fine and it's freed when the last
    // clone is dropped.
    Ok(())
}

/// Explicitly flush pending writes to disk.
pub fn flush(handle: i64) -> Result<u64, KvError> {
    with_db(handle, |db| Ok(db.flush()? as u64))
}

// ---------------- Basic KV on the default tree ------------------------

pub fn insert(handle: i64, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    with_db(handle, |db| Ok(db.insert(key, value)?.map(ivec_to_bytes)))
}

pub fn get(handle: i64, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    with_db(handle, |db| Ok(db.get(key)?.map(ivec_to_bytes)))
}

pub fn remove(handle: i64, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    with_db(handle, |db| Ok(db.remove(key)?.map(ivec_to_bytes)))
}

pub fn contains(handle: i64, key: &[u8]) -> Result<bool, KvError> {
    with_db(handle, |db| Ok(db.contains_key(key)?))
}

pub fn len(handle: i64) -> Result<usize, KvError> {
    with_db(handle, |db| Ok(db.len()))
}

/// Delete every entry from the default tree.
pub fn clear(handle: i64) -> Result<(), KvError> {
    with_db(handle, |db| { db.clear()?; Ok(()) })
}

/// List all keys as strings (skips entries whose keys are not valid UTF-8).
pub fn keys(handle: i64) -> Result<Vec<String>, KvError> {
    with_db(handle, |db| Ok(db.iter()
        .filter_map(|res| res.ok())
        .filter_map(|(k, _)| std::str::from_utf8(&k).ok().map(str::to_string))
        .collect()))
}

/// Atomic compare-and-swap. Returns `true` when the swap succeeded.
pub fn compare_and_swap(
    handle: i64,
    key: &[u8],
    expected: Option<&[u8]>,
    new_value: Option<&[u8]>,
) -> Result<bool, KvError> {
    with_db(handle, |db| Ok(db.compare_and_swap(key, expected, new_value)?.is_ok()))
}

// ---------------- Named "sub-buckets" (Trees) -------------------------

/// Open a named sub-tree (bucket). Useful for organising data by domain,
/// e.g. "sessions", "cache", "config".
pub fn open_tree(db_handle: i64, name: &str) -> Result<i64, KvError> {
    let name = name.to_string();
    let tree = with_db(db_handle, |db| Ok(db.open_tree(name.as_bytes())?))?;
    Ok(insert_tree(tree))
}

pub fn tree_insert(tree_handle: i64, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    with_tree(tree_handle, |tree| Ok(tree.insert(key, value)?.map(ivec_to_bytes)))
}
pub fn tree_get(tree_handle: i64, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    with_tree(tree_handle, |tree| Ok(tree.get(key)?.map(ivec_to_bytes)))
}
pub fn tree_remove(tree_handle: i64, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    with_tree(tree_handle, |tree| Ok(tree.remove(key)?.map(ivec_to_bytes)))
}
pub fn tree_len(tree_handle: i64) -> Result<usize, KvError> {
    with_tree(tree_handle, |tree| Ok(tree.len()))
}
pub fn tree_keys(tree_handle: i64) -> Result<Vec<String>, KvError> {
    with_tree(tree_handle, |tree| Ok(tree.iter()
        .filter_map(|res| res.ok())
        .filter_map(|(k, _)| std::str::from_utf8(&k).ok().map(str::to_string))
        .collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path(tag: &str) -> String {
        let path = std::env::temp_dir().join(format!("titan-kv-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn open_insert_get_remove_round_trip() {
        let path = temp_db_path("basic");
        let db = open(&path).unwrap();

        assert_eq!(insert(db, b"user:1", b"juan").unwrap(), None);
        assert_eq!(insert(db, b"user:1", b"juan2").unwrap(), Some(b"juan".to_vec()));
        assert_eq!(get(db, b"user:1").unwrap(), Some(b"juan2".to_vec()));
        assert!(contains(db, b"user:1").unwrap());
        assert_eq!(len(db).unwrap(), 1);
        assert_eq!(remove(db, b"user:1").unwrap(), Some(b"juan2".to_vec()));
        assert_eq!(get(db, b"user:1").unwrap(), None);

        close(db).unwrap();
        std::fs::remove_dir_all(&path).ok();
    }

    #[test]
    fn survives_close_and_reopen() {
        let path = temp_db_path("persist");
        let db = open(&path).unwrap();
        insert(db, b"greeting", b"hola titan").unwrap();
        flush(db).unwrap();
        close(db).unwrap();

        let db2 = open(&path).unwrap();
        assert_eq!(get(db2, b"greeting").unwrap(), Some(b"hola titan".to_vec()));
        close(db2).unwrap();
        std::fs::remove_dir_all(&path).ok();
    }

    #[test]
    fn compare_and_swap_semantics() {
        let path = temp_db_path("cas");
        let db = open(&path).unwrap();

        // Insert only if absent.
        assert!(compare_and_swap(db, b"k", None, Some(b"v")).unwrap());
        // Fails because the current value is now Some(b"v"), not None.
        assert!(!compare_and_swap(db, b"k", None, Some(b"v2")).unwrap());
        // Updates when expected matches.
        assert!(compare_and_swap(db, b"k", Some(b"v"), Some(b"v2")).unwrap());
        assert_eq!(get(db, b"k").unwrap(), Some(b"v2".to_vec()));

        close(db).unwrap();
        std::fs::remove_dir_all(&path).ok();
    }

    #[test]
    fn named_trees_are_isolated() {
        let path = temp_db_path("trees");
        let db = open(&path).unwrap();
        let sessions = open_tree(db, "sessions").unwrap();
        let cache    = open_tree(db, "cache").unwrap();

        tree_insert(sessions, b"user:1", b"token-abc").unwrap();
        tree_insert(cache, b"user:1", b"payload").unwrap();

        assert_eq!(tree_get(sessions, b"user:1").unwrap(), Some(b"token-abc".to_vec()));
        assert_eq!(tree_get(cache,    b"user:1").unwrap(), Some(b"payload".to_vec()));
        assert_eq!(tree_len(sessions).unwrap(), 1);
        assert_eq!(tree_len(cache).unwrap(),    1);

        close(db).unwrap();
        std::fs::remove_dir_all(&path).ok();
    }

    #[test]
    fn unknown_handle_reports_typed_error() {
        assert!(matches!(get(999_999, b"x"), Err(KvError::UnknownDb(_))));
        assert!(matches!(tree_get(999_999, b"x"), Err(KvError::UnknownTree(_))));
    }
}
