//! Embedded key-value store (`std::kv::*`) powered by `sled`.
//!
//! `sled` is a pure-Rust, ACID, log-structured B-tree that persists a
//! whole database to a single directory on disk. `.titan` accesses it
//! through opaque handles partitioned by VM runtime.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use sled::{Db, IVec, Tree};
use thiserror::Error;

const MAX_DB_HANDLES: usize = 8;
const MAX_TREE_HANDLES: usize = 32;
const MAX_NAMED_TREES_PER_DB: usize = 64;
const MAX_PATH_BYTES: usize = 16 * 1024;
const MAX_TREE_NAME_BYTES: usize = 1024;
const MAX_KEY_BYTES: usize = 64 * 1024;
const MAX_VALUE_BYTES: usize = 8 * 1024 * 1024;
const MAX_DB_LOGICAL_BYTES: usize = 128 * 1024 * 1024;
const MAX_RUNTIME_LOGICAL_BYTES: usize = 256 * 1024 * 1024;
const MAX_DB_ENTRIES: usize = 262_144;
const MAX_RUNTIME_ENTRIES: usize = 524_288;
const MAX_LISTED_KEYS: usize = 65_536;
const MAX_LISTED_KEY_BYTES: usize = 8 * 1024 * 1024;
const MAX_DB_CACHE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CONCURRENT_KV_OPERATIONS: usize = 4;
const SLED_DEFAULT_TREE: &[u8] = b"__sled__default";

#[derive(Debug, Error)]
pub enum KvError {
    #[error("sled error: {0}")]
    Sled(#[from] sled::Error),
    #[error("unknown database handle {0}")]
    UnknownDb(i64),
    #[error("unknown tree handle {0}")]
    UnknownTree(i64),
    #[error("database handle {0} is closing or closed")]
    Closed(i64),
    #[error("invalid key-value argument: {0}")]
    InvalidArgument(&'static str),
    #[error("{resource} exceeds limit {limit}")]
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    #[error("key-value handle space exhausted")]
    HandleSpaceExhausted,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum TreeId {
    Default,
    Named(Vec<u8>),
}

#[derive(Clone, Copy, Debug, Default)]
struct TreeUsage {
    bytes: usize,
    entries: usize,
}

struct DatabaseUsage {
    bytes: usize,
    entries: usize,
    trees: HashMap<TreeId, TreeUsage>,
    closed: bool,
}

struct DatabaseState {
    db: Db,
    owner: u64,
    usage: Mutex<DatabaseUsage>,
}

struct TreeEntry {
    tree: Tree,
    parent_handle: i64,
    tree_id: TreeId,
    state: Arc<DatabaseState>,
}

struct Registry {
    dbs: HashMap<(u64, i64), Arc<DatabaseState>>,
    trees: HashMap<(u64, i64), TreeEntry>,
    reserved_dbs: HashMap<u64, usize>,
    reserved_trees: HashMap<u64, usize>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            dbs: HashMap::new(),
            trees: HashMap::new(),
            reserved_dbs: HashMap::new(),
            reserved_trees: HashMap::new(),
            next_id: 1,
        })
    })
}

#[derive(Default)]
struct RuntimeUsage {
    active_operations: usize,
    logical_bytes: usize,
    entries: usize,
}

fn runtime_usage() -> &'static Mutex<HashMap<u64, RuntimeUsage>> {
    static USAGE: OnceLock<Mutex<HashMap<u64, RuntimeUsage>>> = OnceLock::new();
    USAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

struct OperationPermit {
    runtime_id: u64,
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        let mut usage = crate::native::lock_recover(runtime_usage());
        if let Some(runtime) = usage.get_mut(&self.runtime_id) {
            runtime.active_operations = runtime.active_operations.saturating_sub(1);
            if runtime.active_operations == 0 && runtime.logical_bytes == 0 && runtime.entries == 0 {
                usage.remove(&self.runtime_id);
            }
        }
    }
}

fn reserve_operation() -> Result<OperationPermit, KvError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut usage = crate::native::lock_recover(runtime_usage());
    let runtime = usage.entry(runtime_id).or_default();
    if runtime.active_operations >= MAX_CONCURRENT_KV_OPERATIONS {
        return Err(KvError::ResourceLimit {
            resource: "concurrent key-value operations",
            limit: MAX_CONCURRENT_KV_OPERATIONS,
        });
    }
    runtime.active_operations += 1;
    Ok(OperationPermit { runtime_id })
}

#[derive(Clone, Copy)]
enum HandleKind {
    Database,
    Tree,
}

struct HandleReservation {
    runtime_id: u64,
    kind: HandleKind,
    committed: bool,
}

fn active_handles(registry: &Registry, runtime_id: u64, kind: HandleKind) -> usize {
    match kind {
        HandleKind::Database => registry.dbs.keys().filter(|(owner, _)| *owner == runtime_id).count(),
        HandleKind::Tree => registry
            .trees
            .keys()
            .filter(|(owner, _)| *owner == runtime_id)
            .count(),
    }
}

fn reservation_map(registry: &mut Registry, kind: HandleKind) -> &mut HashMap<u64, usize> {
    match kind {
        HandleKind::Database => &mut registry.reserved_dbs,
        HandleKind::Tree => &mut registry.reserved_trees,
    }
}

fn reserve_handle(kind: HandleKind) -> Result<HandleReservation, KvError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(registry());
    let active = active_handles(&registry, runtime_id, kind);
    let reserved = reservation_map(&mut registry, kind)
        .get(&runtime_id)
        .copied()
        .unwrap_or(0);
    let limit = match kind {
        HandleKind::Database => MAX_DB_HANDLES,
        HandleKind::Tree => MAX_TREE_HANDLES,
    };
    if active.saturating_add(reserved) >= limit {
        return Err(KvError::ResourceLimit {
            resource: match kind {
                HandleKind::Database => "key-value database handles",
                HandleKind::Tree => "key-value tree handles",
            },
            limit,
        });
    }
    *reservation_map(&mut registry, kind)
        .entry(runtime_id)
        .or_default() += 1;
    Ok(HandleReservation {
        runtime_id,
        kind,
        committed: false,
    })
}

fn release_handle_reservation(registry: &mut Registry, runtime_id: u64, kind: HandleKind) {
    let reservations = reservation_map(registry, kind);
    if let Some(count) = reservations.get_mut(&runtime_id) {
        *count = count.saturating_sub(1);
        if *count == 0 {
            reservations.remove(&runtime_id);
        }
    }
}

impl HandleReservation {
    fn commit_db(mut self, state: Arc<DatabaseState>) -> Result<i64, KvError> {
        let mut registry = crate::native::lock_recover(registry());
        let id = registry.next_id;
        registry.next_id = id.checked_add(1).ok_or(KvError::HandleSpaceExhausted)?;
        release_handle_reservation(&mut registry, self.runtime_id, self.kind);
        registry.dbs.insert((self.runtime_id, id), state);
        self.committed = true;
        Ok(id)
    }

    fn commit_tree(
        mut self,
        parent_handle: i64,
        tree: Tree,
        tree_id: TreeId,
        state: Arc<DatabaseState>,
    ) -> Result<i64, KvError> {
        let mut registry = crate::native::lock_recover(registry());
        if !registry.dbs.contains_key(&(self.runtime_id, parent_handle)) {
            return Err(KvError::UnknownDb(parent_handle));
        }
        let id = registry.next_id;
        registry.next_id = id.checked_add(1).ok_or(KvError::HandleSpaceExhausted)?;
        release_handle_reservation(&mut registry, self.runtime_id, self.kind);
        registry.trees.insert(
            (self.runtime_id, id),
            TreeEntry {
                tree,
                parent_handle,
                tree_id,
                state,
            },
        );
        self.committed = true;
        Ok(id)
    }
}

impl Drop for HandleReservation {
    fn drop(&mut self) {
        if !self.committed {
            release_handle_reservation(
                &mut crate::native::lock_recover(registry()),
                self.runtime_id,
                self.kind,
            );
        }
    }
}

fn get_db(handle: i64) -> Result<Arc<DatabaseState>, KvError> {
    crate::native::lock_recover(registry())
        .dbs
        .get(&crate::native::runtime_handle_key(handle))
        .cloned()
        .ok_or(KvError::UnknownDb(handle))
}

fn get_tree(handle: i64) -> Result<(Tree, TreeId, Arc<DatabaseState>), KvError> {
    crate::native::lock_recover(registry())
        .trees
        .get(&crate::native::runtime_handle_key(handle))
        .map(|entry| {
            (
                entry.tree.clone(),
                entry.tree_id.clone(),
                Arc::clone(&entry.state),
            )
        })
        .ok_or(KvError::UnknownTree(handle))
}

fn validate_path(path: &str) -> Result<(), KvError> {
    if path.len() > MAX_PATH_BYTES {
        return Err(KvError::ResourceLimit {
            resource: "key-value database path bytes",
            limit: MAX_PATH_BYTES,
        });
    }
    if path.is_empty() {
        return Err(KvError::InvalidArgument("database path must not be empty"));
    }
    Ok(())
}

fn validate_tree_name(name: &[u8]) -> Result<(), KvError> {
    if name.is_empty() {
        return Err(KvError::InvalidArgument("tree name must not be empty"));
    }
    if name.len() > MAX_TREE_NAME_BYTES {
        return Err(KvError::ResourceLimit {
            resource: "key-value tree name bytes",
            limit: MAX_TREE_NAME_BYTES,
        });
    }
    Ok(())
}

fn validate_key(key: &[u8]) -> Result<(), KvError> {
    if key.len() > MAX_KEY_BYTES {
        return Err(KvError::ResourceLimit {
            resource: "key-value key bytes",
            limit: MAX_KEY_BYTES,
        });
    }
    Ok(())
}

fn validate_value(value: &[u8]) -> Result<(), KvError> {
    if value.len() > MAX_VALUE_BYTES {
        return Err(KvError::ResourceLimit {
            resource: "key-value value bytes",
            limit: MAX_VALUE_BYTES,
        });
    }
    Ok(())
}

fn item_bytes(key: &[u8], value: &[u8]) -> Result<usize, KvError> {
    key.len().checked_add(value.len()).ok_or(KvError::ResourceLimit {
        resource: "key-value logical bytes",
        limit: MAX_DB_LOGICAL_BYTES,
    })
}

fn reserve_runtime_storage(runtime_id: u64, bytes: usize, entries: usize) -> Result<(), KvError> {
    let mut usage = crate::native::lock_recover(runtime_usage());
    let runtime = usage.entry(runtime_id).or_default();
    let new_bytes = runtime
        .logical_bytes
        .checked_add(bytes)
        .ok_or(KvError::ResourceLimit {
            resource: "runtime key-value logical bytes",
            limit: MAX_RUNTIME_LOGICAL_BYTES,
        })?;
    let new_entries = runtime
        .entries
        .checked_add(entries)
        .ok_or(KvError::ResourceLimit {
            resource: "runtime key-value entries",
            limit: MAX_RUNTIME_ENTRIES,
        })?;
    if new_bytes > MAX_RUNTIME_LOGICAL_BYTES {
        return Err(KvError::ResourceLimit {
            resource: "runtime key-value logical bytes",
            limit: MAX_RUNTIME_LOGICAL_BYTES,
        });
    }
    if new_entries > MAX_RUNTIME_ENTRIES {
        return Err(KvError::ResourceLimit {
            resource: "runtime key-value entries",
            limit: MAX_RUNTIME_ENTRIES,
        });
    }
    runtime.logical_bytes = new_bytes;
    runtime.entries = new_entries;
    Ok(())
}

fn release_runtime_storage(runtime_id: u64, bytes: usize, entries: usize) {
    let mut usage = crate::native::lock_recover(runtime_usage());
    if let Some(runtime) = usage.get_mut(&runtime_id) {
        runtime.logical_bytes = runtime.logical_bytes.saturating_sub(bytes);
        runtime.entries = runtime.entries.saturating_sub(entries);
        if runtime.active_operations == 0 && runtime.logical_bytes == 0 && runtime.entries == 0 {
            usage.remove(&runtime_id);
        }
    }
}

fn measure_iterator(iterator: sled::Iter) -> Result<TreeUsage, KvError> {
    let mut usage = TreeUsage::default();
    for result in iterator {
        let (key, value) = result?;
        validate_key(&key)?;
        validate_value(&value)?;
        usage.bytes = usage
            .bytes
            .checked_add(item_bytes(&key, &value)?)
            .ok_or(KvError::ResourceLimit {
                resource: "database logical bytes",
                limit: MAX_DB_LOGICAL_BYTES,
            })?;
        usage.entries = usage.entries.checked_add(1).ok_or(KvError::ResourceLimit {
            resource: "database entries",
            limit: MAX_DB_ENTRIES,
        })?;
        if usage.bytes > MAX_DB_LOGICAL_BYTES {
            return Err(KvError::ResourceLimit {
                resource: "database logical bytes",
                limit: MAX_DB_LOGICAL_BYTES,
            });
        }
        if usage.entries > MAX_DB_ENTRIES {
            return Err(KvError::ResourceLimit {
                resource: "database entries",
                limit: MAX_DB_ENTRIES,
            });
        }
    }
    Ok(usage)
}

fn measure_database(db: &Db) -> Result<DatabaseUsage, KvError> {
    let mut trees = HashMap::new();
    let default_usage = measure_iterator(db.iter())?;
    trees.insert(TreeId::Default, default_usage);
    let names = db.tree_names();
    let named_count = names
        .iter()
        .filter(|name| name.as_ref() != SLED_DEFAULT_TREE)
        .count();
    if named_count > MAX_NAMED_TREES_PER_DB {
        return Err(KvError::ResourceLimit {
            resource: "named trees per database",
            limit: MAX_NAMED_TREES_PER_DB,
        });
    }
    for name in names {
        if name.as_ref() == SLED_DEFAULT_TREE {
            continue;
        }
        validate_tree_name(&name)?;
        let tree = db.open_tree(&name)?;
        trees.insert(TreeId::Named(name.to_vec()), measure_iterator(tree.iter())?);
    }
    let (bytes, entries) = trees.values().try_fold(
        (0usize, 0usize),
        |(total_bytes, total_entries), tree| {
            let bytes = total_bytes
                .checked_add(tree.bytes)
                .ok_or(KvError::ResourceLimit {
                    resource: "database logical bytes",
                    limit: MAX_DB_LOGICAL_BYTES,
                })?;
            let entries = total_entries
                .checked_add(tree.entries)
                .ok_or(KvError::ResourceLimit {
                    resource: "database entries",
                    limit: MAX_DB_ENTRIES,
                })?;
            Ok::<_, KvError>((bytes, entries))
        },
    )?;
    if bytes > MAX_DB_LOGICAL_BYTES {
        return Err(KvError::ResourceLimit {
            resource: "database logical bytes",
            limit: MAX_DB_LOGICAL_BYTES,
        });
    }
    if entries > MAX_DB_ENTRIES {
        return Err(KvError::ResourceLimit {
            resource: "database entries",
            limit: MAX_DB_ENTRIES,
        });
    }
    Ok(DatabaseUsage {
        bytes,
        entries,
        trees,
        closed: false,
    })
}

fn ensure_open(usage: &DatabaseUsage, handle: i64) -> Result<(), KvError> {
    if usage.closed {
        Err(KvError::Closed(handle))
    } else {
        Ok(())
    }
}

fn reserve_replacement_growth(
    state: &DatabaseState,
    usage: &DatabaseUsage,
    old_bytes: usize,
    old_entries: usize,
    new_bytes: usize,
    new_entries: usize,
) -> Result<(usize, usize), KvError> {
    let byte_growth = new_bytes.saturating_sub(old_bytes);
    let entry_growth = new_entries.saturating_sub(old_entries);
    if usage.bytes.saturating_add(byte_growth) > MAX_DB_LOGICAL_BYTES {
        return Err(KvError::ResourceLimit {
            resource: "database logical bytes",
            limit: MAX_DB_LOGICAL_BYTES,
        });
    }
    if usage.entries.saturating_add(entry_growth) > MAX_DB_ENTRIES {
        return Err(KvError::ResourceLimit {
            resource: "database entries",
            limit: MAX_DB_ENTRIES,
        });
    }
    reserve_runtime_storage(state.owner, byte_growth, entry_growth)?;
    Ok((byte_growth, entry_growth))
}

fn finish_replacement(
    state: &DatabaseState,
    usage: &mut DatabaseUsage,
    tree_id: &TreeId,
    old_bytes: usize,
    old_entries: usize,
    new_bytes: usize,
    new_entries: usize,
) {
    if let Some(tree) = usage.trees.get_mut(tree_id) {
        tree.bytes = tree.bytes.saturating_sub(old_bytes).saturating_add(new_bytes);
        tree.entries = tree
            .entries
            .saturating_sub(old_entries)
            .saturating_add(new_entries);
    }
    usage.bytes = usage.bytes.saturating_sub(old_bytes).saturating_add(new_bytes);
    usage.entries = usage
        .entries
        .saturating_sub(old_entries)
        .saturating_add(new_entries);
    release_runtime_storage(
        state.owner,
        old_bytes.saturating_sub(new_bytes),
        old_entries.saturating_sub(new_entries),
    );
}

fn insert_value<G, I>(
    handle: i64,
    state: &DatabaseState,
    tree_id: &TreeId,
    key: &[u8],
    value: &[u8],
    get: G,
    insert: I,
) -> Result<Option<Vec<u8>>, KvError>
where
    G: FnOnce() -> Result<Option<IVec>, sled::Error>,
    I: FnOnce() -> Result<Option<IVec>, sled::Error>,
{
    validate_key(key)?;
    validate_value(value)?;
    let mut usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, handle)?;
    if !usage.trees.contains_key(tree_id) {
        return Err(KvError::Closed(handle));
    }
    let previous = get()?;
    let old_bytes = previous
        .as_ref()
        .map(|old| item_bytes(key, old))
        .transpose()?
        .unwrap_or(0);
    let old_entries = if previous.is_some() { 1 } else { 0 };
    let new_bytes = item_bytes(key, value)?;
    let (reserved_bytes, reserved_entries) =
        reserve_replacement_growth(state, &usage, old_bytes, old_entries, new_bytes, 1)?;
    let result = match insert() {
        Ok(result) => result,
        Err(error) => {
            release_runtime_storage(state.owner, reserved_bytes, reserved_entries);
            return Err(error.into());
        }
    };
    finish_replacement(
        state,
        &mut usage,
        tree_id,
        old_bytes,
        old_entries,
        new_bytes,
        1,
    );
    Ok(result.map(|value| value.to_vec()))
}

fn remove_value<R>(
    handle: i64,
    state: &DatabaseState,
    tree_id: &TreeId,
    key: &[u8],
    remove: R,
) -> Result<Option<Vec<u8>>, KvError>
where
    R: FnOnce() -> Result<Option<IVec>, sled::Error>,
{
    validate_key(key)?;
    let mut usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, handle)?;
    if !usage.trees.contains_key(tree_id) {
        return Err(KvError::Closed(handle));
    }
    let removed = remove()?;
    if let Some(value) = &removed {
        let old_bytes = item_bytes(key, value)?;
        finish_replacement(state, &mut usage, tree_id, old_bytes, 1, 0, 0);
    }
    Ok(removed.map(|value| value.to_vec()))
}

fn list_keys(iterator: sled::Iter) -> Result<Vec<String>, KvError> {
    let mut keys = Vec::new();
    let mut bytes = 0usize;
    for result in iterator {
        let (key, _) = result?;
        let Ok(key) = std::str::from_utf8(&key) else {
            continue;
        };
        if keys.len() >= MAX_LISTED_KEYS {
            return Err(KvError::ResourceLimit {
                resource: "listed key count",
                limit: MAX_LISTED_KEYS,
            });
        }
        bytes = bytes.checked_add(key.len()).ok_or(KvError::ResourceLimit {
            resource: "listed key bytes",
            limit: MAX_LISTED_KEY_BYTES,
        })?;
        if bytes > MAX_LISTED_KEY_BYTES {
            return Err(KvError::ResourceLimit {
                resource: "listed key bytes",
                limit: MAX_LISTED_KEY_BYTES,
            });
        }
        keys.push(key.to_owned());
    }
    Ok(keys)
}

// ---------------- Open / close ----------------------------------------

/// Open (or create) a bounded database at `path`.
pub fn open(path: &str) -> Result<i64, KvError> {
    validate_path(path)?;
    let _permit = reserve_operation()?;
    let reservation = reserve_handle(HandleKind::Database)?;
    let db = sled::Config::default()
        .path(path)
        .cache_capacity(MAX_DB_CACHE_BYTES)
        .open()?;
    let usage = measure_database(&db)?;
    let owner = crate::native::current_runtime_id();
    let measured_bytes = usage.bytes;
    let measured_entries = usage.entries;
    reserve_runtime_storage(owner, measured_bytes, measured_entries)?;
    let state = Arc::new(DatabaseState {
        db,
        owner,
        usage: Mutex::new(usage),
    });
    match reservation.commit_db(state) {
        Ok(handle) => Ok(handle),
        Err(error) => {
            release_runtime_storage(owner, measured_bytes, measured_entries);
            Err(error)
        }
    }
}

/// Close a database, invalidate its tree handles, and flush it. Idempotent.
pub fn close(handle: i64) -> Result<(), KvError> {
    let _permit = reserve_operation()?;
    let runtime_id = crate::native::current_runtime_id();
    let (state, _released_trees) = {
        let mut registry = crate::native::lock_recover(registry());
        let Some(state) = registry.dbs.remove(&(runtime_id, handle)) else {
            return Ok(());
        };
        let before = registry.trees.len();
        registry
            .trees
            .retain(|(owner, _), tree| *owner != runtime_id || tree.parent_handle != handle);
        (state, before - registry.trees.len())
    };
    let mut usage = crate::native::lock_recover(&state.usage);
    usage.closed = true;
    let bytes = usage.bytes;
    let entries = usage.entries;
    let flush_result = state.db.flush().map(|_| ()).map_err(KvError::from);
    release_runtime_storage(runtime_id, bytes, entries);
    flush_result
}

/// Explicitly flush pending writes to disk.
pub fn flush(handle: i64) -> Result<u64, KvError> {
    let _permit = reserve_operation()?;
    let state = get_db(handle)?;
    let usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, handle)?;
    Ok(state.db.flush()? as u64)
}

// ---------------- Basic KV on the default tree ------------------------

pub fn insert(handle: i64, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    validate_key(key)?;
    validate_value(value)?;
    let _permit = reserve_operation()?;
    let state = get_db(handle)?;
    insert_value(
        handle,
        &state,
        &TreeId::Default,
        key,
        value,
        || state.db.get(key),
        || state.db.insert(key, value),
    )
}

pub fn get(handle: i64, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    validate_key(key)?;
    let _permit = reserve_operation()?;
    let state = get_db(handle)?;
    let usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, handle)?;
    Ok(state.db.get(key)?.map(|value| value.to_vec()))
}

pub fn remove(handle: i64, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    validate_key(key)?;
    let _permit = reserve_operation()?;
    let state = get_db(handle)?;
    remove_value(handle, &state, &TreeId::Default, key, || state.db.remove(key))
}

pub fn contains(handle: i64, key: &[u8]) -> Result<bool, KvError> {
    validate_key(key)?;
    let _permit = reserve_operation()?;
    let state = get_db(handle)?;
    let usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, handle)?;
    Ok(state.db.contains_key(key)?)
}

pub fn len(handle: i64) -> Result<usize, KvError> {
    let _permit = reserve_operation()?;
    let state = get_db(handle)?;
    let usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, handle)?;
    Ok(usage.trees.get(&TreeId::Default).map_or(0, |tree| tree.entries))
}

/// Delete every entry from the default tree.
pub fn clear(handle: i64) -> Result<(), KvError> {
    let _permit = reserve_operation()?;
    let state = get_db(handle)?;
    let mut usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, handle)?;
    state.db.clear()?;
    let old = usage.trees.insert(TreeId::Default, TreeUsage::default()).unwrap_or_default();
    usage.bytes = usage.bytes.saturating_sub(old.bytes);
    usage.entries = usage.entries.saturating_sub(old.entries);
    release_runtime_storage(state.owner, old.bytes, old.entries);
    Ok(())
}

/// List UTF-8 keys, rejecting output that exceeds the list budget.
pub fn keys(handle: i64) -> Result<Vec<String>, KvError> {
    let _permit = reserve_operation()?;
    let state = get_db(handle)?;
    let usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, handle)?;
    list_keys(state.db.iter())
}

/// Atomic compare-and-swap. Returns `true` when the swap succeeded.
pub fn compare_and_swap(
    handle: i64,
    key: &[u8],
    expected: Option<&[u8]>,
    new_value: Option<&[u8]>,
) -> Result<bool, KvError> {
    validate_key(key)?;
    if let Some(value) = expected {
        validate_value(value)?;
    }
    if let Some(value) = new_value {
        validate_value(value)?;
    }
    let _permit = reserve_operation()?;
    let state = get_db(handle)?;
    let mut usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, handle)?;
    let current = state.db.get(key)?;
    let matches = match (&current, expected) {
        (None, None) => true,
        (Some(current), Some(expected)) => current.as_ref() == expected,
        _ => false,
    };
    if !matches {
        return Ok(false);
    }
    let old_bytes = current
        .as_ref()
        .map(|value| item_bytes(key, value))
        .transpose()?
        .unwrap_or(0);
    let old_entries = if current.is_some() { 1 } else { 0 };
    let new_bytes = new_value.map(|value| item_bytes(key, value)).transpose()?.unwrap_or(0);
    let new_entries = if new_value.is_some() { 1 } else { 0 };
    let (reserved_bytes, reserved_entries) = reserve_replacement_growth(
        &state,
        &usage,
        old_bytes,
        old_entries,
        new_bytes,
        new_entries,
    )?;
    match state.db.compare_and_swap(key, expected, new_value)? {
        Ok(()) => {
            finish_replacement(
                &state,
                &mut usage,
                &TreeId::Default,
                old_bytes,
                old_entries,
                new_bytes,
                new_entries,
            );
            Ok(true)
        }
        Err(_) => {
            release_runtime_storage(state.owner, reserved_bytes, reserved_entries);
            Ok(false)
        }
    }
}

// ---------------- Named sub-trees -------------------------------------

pub fn open_tree(db_handle: i64, name: &str) -> Result<i64, KvError> {
    validate_tree_name(name.as_bytes())?;
    let _permit = reserve_operation()?;
    let reservation = reserve_handle(HandleKind::Tree)?;
    let state = get_db(db_handle)?;
    let tree_id = TreeId::Named(name.as_bytes().to_vec());
    let (tree, created) = {
        let mut usage = crate::native::lock_recover(&state.usage);
        ensure_open(&usage, db_handle)?;
        let created = !usage.trees.contains_key(&tree_id);
        if created
            && usage
                .trees
                .keys()
                .filter(|tree| matches!(tree, TreeId::Named(_)))
                .count()
                >= MAX_NAMED_TREES_PER_DB
        {
            return Err(KvError::ResourceLimit {
                resource: "named trees per database",
                limit: MAX_NAMED_TREES_PER_DB,
            });
        }
        let tree = state.db.open_tree(name.as_bytes())?;
        usage.trees.entry(tree_id.clone()).or_default();
        (tree, created)
    };
    match reservation.commit_tree(db_handle, tree, tree_id.clone(), Arc::clone(&state)) {
        Ok(handle) => Ok(handle),
        Err(error) => {
            if created {
                let mut usage = crate::native::lock_recover(&state.usage);
                usage.trees.remove(&tree_id);
                let _ = state.db.drop_tree(name.as_bytes());
            }
            Err(error)
        }
    }
}

pub fn tree_insert(
    tree_handle: i64,
    key: &[u8],
    value: &[u8],
) -> Result<Option<Vec<u8>>, KvError> {
    validate_key(key)?;
    validate_value(value)?;
    let _permit = reserve_operation()?;
    let (tree, tree_id, state) = get_tree(tree_handle)?;
    insert_value(
        tree_handle,
        &state,
        &tree_id,
        key,
        value,
        || tree.get(key),
        || tree.insert(key, value),
    )
}

pub fn tree_get(tree_handle: i64, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    validate_key(key)?;
    let _permit = reserve_operation()?;
    let (tree, _, state) = get_tree(tree_handle)?;
    let usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, tree_handle)?;
    Ok(tree.get(key)?.map(|value| value.to_vec()))
}

pub fn tree_remove(tree_handle: i64, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
    validate_key(key)?;
    let _permit = reserve_operation()?;
    let (tree, tree_id, state) = get_tree(tree_handle)?;
    remove_value(tree_handle, &state, &tree_id, key, || tree.remove(key))
}

pub fn tree_len(tree_handle: i64) -> Result<usize, KvError> {
    let _permit = reserve_operation()?;
    let (_, tree_id, state) = get_tree(tree_handle)?;
    let usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, tree_handle)?;
    Ok(usage.trees.get(&tree_id).map_or(0, |tree| tree.entries))
}

pub fn tree_keys(tree_handle: i64) -> Result<Vec<String>, KvError> {
    let _permit = reserve_operation()?;
    let (tree, _, state) = get_tree(tree_handle)?;
    let usage = crate::native::lock_recover(&state.usage);
    ensure_open(&usage, tree_handle)?;
    list_keys(tree.iter())
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let (states, released_trees) = {
        let mut registry = crate::native::lock_recover(registry());
        let states = registry
            .dbs
            .iter()
            .filter(|((owner, _), _)| *owner == runtime_id)
            .map(|(_, state)| Arc::clone(state))
            .collect::<Vec<_>>();
        let db_count = states.len();
        registry.dbs.retain(|(owner, _), _| *owner != runtime_id);
        let before = registry.trees.len();
        registry.trees.retain(|(owner, _), _| *owner != runtime_id);
        registry.reserved_dbs.remove(&runtime_id);
        registry.reserved_trees.remove(&runtime_id);
        (states, db_count + before - registry.trees.len())
    };
    for state in states {
        let mut usage = crate::native::lock_recover(&state.usage);
        if !usage.closed {
            usage.closed = true;
            let _ = state.db.flush();
            release_runtime_storage(runtime_id, usage.bytes, usage.entries);
        }
    }
    released_trees
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
        assert_eq!(
            insert(db, b"user:1", b"juan2").unwrap(),
            Some(b"juan".to_vec())
        );
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
        assert!(compare_and_swap(db, b"k", None, Some(b"v")).unwrap());
        assert!(!compare_and_swap(db, b"k", None, Some(b"v2")).unwrap());
        assert!(compare_and_swap(db, b"k", Some(b"v"), Some(b"v2")).unwrap());
        assert_eq!(get(db, b"k").unwrap(), Some(b"v2".to_vec()));
        close(db).unwrap();
        std::fs::remove_dir_all(&path).ok();
    }

    #[test]
    fn named_trees_are_isolated_and_close_with_parent() {
        let path = temp_db_path("trees");
        let db = open(&path).unwrap();
        let sessions = open_tree(db, "sessions").unwrap();
        let cache = open_tree(db, "cache").unwrap();
        tree_insert(sessions, b"user:1", b"token-abc").unwrap();
        tree_insert(cache, b"user:1", b"payload").unwrap();
        assert_eq!(
            tree_get(sessions, b"user:1").unwrap(),
            Some(b"token-abc".to_vec())
        );
        assert_eq!(
            tree_get(cache, b"user:1").unwrap(),
            Some(b"payload".to_vec())
        );
        assert_eq!(tree_len(sessions).unwrap(), 1);
        assert_eq!(tree_len(cache).unwrap(), 1);
        close(db).unwrap();
        assert!(matches!(
            tree_get(sessions, b"user:1"),
            Err(KvError::UnknownTree(_))
        ));
        std::fs::remove_dir_all(&path).ok();
    }

    #[test]
    fn handles_inputs_and_operations_are_bounded() {
        assert!(matches!(
            validate_key(&vec![0; MAX_KEY_BYTES + 1]),
            Err(KvError::ResourceLimit { .. })
        ));
        assert!(matches!(
            validate_value(&vec![0; MAX_VALUE_BYTES + 1]),
            Err(KvError::ResourceLimit { .. })
        ));
        let runtime_id = 8_300_012;
        let paths = crate::native::with_runtime_context(runtime_id, || {
            let paths = (0..MAX_DB_HANDLES)
                .map(|index| temp_db_path(&format!("quota-{index}")))
                .collect::<Vec<_>>();
            let mut handles = paths.iter().map(|path| open(path).unwrap()).collect::<Vec<_>>();
            assert!(matches!(
                open(&temp_db_path("quota-overflow")),
                Err(KvError::ResourceLimit {
                    resource: "key-value database handles",
                    ..
                })
            ));
            close(handles.pop().unwrap()).unwrap();
            handles.push(open(&temp_db_path("quota-replacement")).unwrap());
            paths
        });
        assert_eq!(cleanup_runtime(runtime_id), MAX_DB_HANDLES);
        for path in paths {
            std::fs::remove_dir_all(path).ok();
        }
        std::fs::remove_dir_all(temp_db_path("quota-overflow")).ok();
        std::fs::remove_dir_all(temp_db_path("quota-replacement")).ok();

        crate::native::with_runtime_context(runtime_id, || {
            let permits = (0..MAX_CONCURRENT_KV_OPERATIONS)
                .map(|_| reserve_operation().unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                reserve_operation(),
                Err(KvError::ResourceLimit {
                    resource: "concurrent key-value operations",
                    ..
                })
            ));
            drop(permits);
        });
        assert!(!crate::native::lock_recover(runtime_usage()).contains_key(&runtime_id));
    }

    #[test]
    fn tree_handles_and_runtime_storage_are_bounded() {
        let runtime_id = 8_300_013;
        let path = temp_db_path("tree-quota");
        crate::native::with_runtime_context(runtime_id, || {
            let db = open(&path).unwrap();
            let handles = (0..MAX_TREE_HANDLES)
                .map(|_| open_tree(db, "shared").unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                open_tree(db, "overflow"),
                Err(KvError::ResourceLimit {
                    resource: "key-value tree handles",
                    ..
                })
            ));
            close(db).unwrap();
            assert!(matches!(
                tree_len(handles[0]),
                Err(KvError::UnknownTree(_))
            ));
        });
        assert_eq!(cleanup_runtime(runtime_id), 0);
        std::fs::remove_dir_all(path).ok();

        crate::native::with_runtime_context(runtime_id, || {
            reserve_runtime_storage(MAX_RUNTIME_LOGICAL_BYTES, MAX_RUNTIME_ENTRIES).unwrap();
            assert!(matches!(
                reserve_runtime_storage(1, 0),
                Err(KvError::ResourceLimit {
                    resource: "runtime key-value logical bytes",
                    ..
                })
            ));
            assert!(matches!(
                reserve_runtime_storage(0, 1),
                Err(KvError::ResourceLimit {
                    resource: "runtime key-value entries",
                    ..
                })
            ));
            release_runtime_storage(MAX_RUNTIME_LOGICAL_BYTES, MAX_RUNTIME_ENTRIES);
        });
        assert!(!crate::native::lock_recover(runtime_usage()).contains_key(&runtime_id));
    }

    #[test]
    fn oversized_existing_value_is_rejected_while_opening() {
        let path = temp_db_path("oversized-existing");
        let raw = sled::Config::default()
            .path(&path)
            .cache_capacity(MAX_DB_CACHE_BYTES)
            .open()
            .unwrap();
        raw.insert(b"oversized", vec![0; MAX_VALUE_BYTES + 1]).unwrap();
        raw.flush().unwrap();
        drop(raw);
        assert!(matches!(
            open(&path),
            Err(KvError::ResourceLimit {
                resource: "key-value value bytes",
                ..
            })
        ));
        std::fs::remove_dir_all(path).ok();
    }

    #[test]
    fn unknown_handle_reports_typed_error() {
        assert!(matches!(get(999_999, b"x"), Err(KvError::UnknownDb(_))));
        assert!(matches!(
            tree_get(999_999, b"x"),
            Err(KvError::UnknownTree(_))
        ));
    }
}
