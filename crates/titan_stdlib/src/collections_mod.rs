//! std::collections — Set, Deque, PriorityQueue, OrderedMap, Counter, Graph.
//!
//! Estructuras de datos serias que faltaban en la stdlib para escribir
//! algoritmos profesionales. Cada una implementada con la primitiva
//! correcta de Rust std / indexmap y con las operaciones completas que
//! espera un dev: no son wrappers minimales, son abstracciones útiles.

use std::collections::{BTreeSet, BinaryHeap, HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use indexmap::IndexMap;

// ---------------- Global handle registry ----------------
//
// Todas las estructuras se guardan en un registro global con handle
// int64 — igual patron que sqlite/websocket handles en Titan.

const MAX_COLLECTION_HANDLES: usize = 256;
const MAX_COLLECTION_ENTRIES: usize = 65_536;
const MAX_ENTRIES_PER_HANDLE: usize = 4_096;
const MAX_COLLECTION_BYTES: usize = 16 * 1024 * 1024;
const MAX_ITEM_BYTES: usize = 64 * 1024;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct CollectionUsage {
    handles: usize,
    entries: usize,
    bytes: usize,
}

static NEXT_HANDLE: OnceLock<AtomicU64> = OnceLock::new();
static USAGE: OnceLock<Mutex<HashMap<u64, CollectionUsage>>> = OnceLock::new();

fn usage() -> &'static Mutex<HashMap<u64, CollectionUsage>> {
    USAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_handle() -> Result<u64, String> {
    NEXT_HANDLE
        .get_or_init(|| AtomicU64::new(1))
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |handle| {
            (handle <= i64::MAX as u64).then(|| handle + 1)
        })
        .map_err(|_| "collection handle space exhausted".to_string())
}

fn handle_key(handle: u64) -> (u64, u64) {
    crate::native::runtime_handle_key(handle)
}

fn current_runtime_id() -> u64 {
    crate::native::current_runtime_id()
}

#[derive(Debug)]
struct UsagePermit {
    runtime_id: u64,
    handles: usize,
    entries: usize,
    bytes: usize,
    committed: bool,
}

impl UsagePermit {
    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for UsagePermit {
    fn drop(&mut self) {
        if !self.committed {
            release_usage(self.runtime_id, self.handles, self.entries, self.bytes);
        }
    }
}

fn reserve_usage(handles: usize, entries: usize, bytes: usize) -> Result<UsagePermit, String> {
    let runtime_id = current_runtime_id();
    let mut usages = crate::native::lock_recover(usage());
    let current = usages.get(&runtime_id).copied().unwrap_or_default();
    let requested = CollectionUsage {
        handles: current
            .handles
            .checked_add(handles)
            .ok_or_else(|| "collection handle count overflow".to_string())?,
        entries: current
            .entries
            .checked_add(entries)
            .ok_or_else(|| "collection entry count overflow".to_string())?,
        bytes: current
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| "collection byte count overflow".to_string())?,
    };
    if requested.handles > MAX_COLLECTION_HANDLES {
        return Err(format!(
            "collection handle quota exceeded (limit {MAX_COLLECTION_HANDLES})"
        ));
    }
    if requested.entries > MAX_COLLECTION_ENTRIES {
        return Err(format!(
            "collection entry quota exceeded (limit {MAX_COLLECTION_ENTRIES})"
        ));
    }
    if requested.bytes > MAX_COLLECTION_BYTES {
        return Err(format!(
            "collection byte quota exceeded (limit {MAX_COLLECTION_BYTES})"
        ));
    }
    usages.insert(runtime_id, requested);
    Ok(UsagePermit {
        runtime_id,
        handles,
        entries,
        bytes,
        committed: false,
    })
}

fn release_usage(runtime_id: u64, handles: usize, entries: usize, bytes: usize) {
    let mut usages = crate::native::lock_recover(usage());
    let remove = if let Some(current) = usages.get_mut(&runtime_id) {
        debug_assert!(
            current.handles >= handles && current.entries >= entries && current.bytes >= bytes,
            "collection usage counter underflow"
        );
        current.handles = current.handles.saturating_sub(handles);
        current.entries = current.entries.saturating_sub(entries);
        current.bytes = current.bytes.saturating_sub(bytes);
        *current == CollectionUsage::default()
    } else {
        false
    };
    if remove {
        usages.remove(&runtime_id);
    }
}

#[cfg(test)]
fn runtime_usage(runtime_id: u64) -> CollectionUsage {
    crate::native::lock_recover(usage())
        .get(&runtime_id)
        .copied()
        .unwrap_or_default()
}

fn validate_item(item: &str) -> Result<(), String> {
    if item.len() > MAX_ITEM_BYTES {
        return Err(format!(
            "collection item exceeds byte limit {MAX_ITEM_BYTES}"
        ));
    }
    Ok(())
}

fn ensure_handle_entries(current: usize, added: usize) -> Result<(), String> {
    if current.saturating_add(added) > MAX_ENTRIES_PER_HANDLE {
        return Err(format!(
            "collection handle entry quota exceeded (limit {MAX_ENTRIES_PER_HANDLE})"
        ));
    }
    Ok(())
}

fn count_true(value: bool) -> usize {
    if value {
        1
    } else {
        0
    }
}

fn string_bytes<'a>(items: impl IntoIterator<Item = &'a String>) -> Result<usize, String> {
    items.into_iter().try_fold(0usize, |total, item| {
        validate_item(item)?;
        total
            .checked_add(item.len())
            .ok_or_else(|| "collection byte count overflow".to_string())
    })
}

// ---------------- Set (BTreeSet<String>) ----------------

static SETS: OnceLock<Mutex<HashMap<(u64, u64), BTreeSet<String>>>> = OnceLock::new();
fn sets() -> &'static Mutex<HashMap<(u64, u64), BTreeSet<String>>> {
    SETS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_set(values: BTreeSet<String>) -> Result<u64, String> {
    ensure_handle_entries(0, values.len())?;
    let bytes = string_bytes(values.iter())?;
    let permit = reserve_usage(1, values.len(), bytes)?;
    let handle = next_handle()?;
    sets().lock().unwrap().insert(handle_key(handle), values);
    permit.commit();
    Ok(handle)
}

pub fn set_new() -> Result<u64, String> {
    register_set(BTreeSet::new())
}

pub fn set_from(items: Vec<String>) -> Result<u64, String> {
    ensure_handle_entries(0, items.len())?;
    for item in &items {
        validate_item(item)?;
    }
    register_set(items.into_iter().collect())
}

pub fn set_add(h: u64, item: String) -> Result<bool, String> {
    validate_item(&item)?;
    let mut sets = sets().lock().unwrap();
    let set = sets
        .get_mut(&handle_key(h))
        .ok_or_else(|| format!("unknown set {h}"))?;
    if set.contains(&item) {
        return Ok(false);
    }
    ensure_handle_entries(set.len(), 1)?;
    let permit = reserve_usage(0, 1, item.len())?;
    let inserted = set.insert(item);
    debug_assert!(inserted);
    permit.commit();
    Ok(true)
}

pub fn set_remove(h: u64, item: &str) -> Result<bool, String> {
    let removed = {
        let mut sets = sets().lock().unwrap();
        let set = sets
            .get_mut(&handle_key(h))
            .ok_or_else(|| format!("unknown set {h}"))?;
        set.take(item)
    };
    if let Some(removed) = removed {
        release_usage(current_runtime_id(), 0, 1, removed.len());
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn set_contains(h: u64, item: &str) -> Result<bool, String> {
    let sets = sets().lock().unwrap();
    let set = sets
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown set {h}"))?;
    Ok(set.contains(item))
}

pub fn set_len(h: u64) -> Result<usize, String> {
    let sets = sets().lock().unwrap();
    let set = sets
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown set {h}"))?;
    Ok(set.len())
}

pub fn set_to_array(h: u64) -> Result<Vec<String>, String> {
    let sets = sets().lock().unwrap();
    let set = sets
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown set {h}"))?;
    Ok(set.iter().cloned().collect())
}

pub fn set_union(a: u64, b: u64) -> Result<u64, String> {
    let sets = sets().lock().unwrap();
    let left = sets
        .get(&handle_key(a))
        .ok_or_else(|| format!("unknown set {a}"))?;
    let right = sets
        .get(&handle_key(b))
        .ok_or_else(|| format!("unknown set {b}"))?;
    let merged = left.union(right).cloned().collect();
    drop(sets);
    register_set(merged)
}

pub fn set_intersect(a: u64, b: u64) -> Result<u64, String> {
    let sets = sets().lock().unwrap();
    let left = sets
        .get(&handle_key(a))
        .ok_or_else(|| format!("unknown set {a}"))?;
    let right = sets
        .get(&handle_key(b))
        .ok_or_else(|| format!("unknown set {b}"))?;
    let merged = left.intersection(right).cloned().collect();
    drop(sets);
    register_set(merged)
}

pub fn set_difference(a: u64, b: u64) -> Result<u64, String> {
    let sets = sets().lock().unwrap();
    let left = sets
        .get(&handle_key(a))
        .ok_or_else(|| format!("unknown set {a}"))?;
    let right = sets
        .get(&handle_key(b))
        .ok_or_else(|| format!("unknown set {b}"))?;
    let merged = left.difference(right).cloned().collect();
    drop(sets);
    register_set(merged)
}

pub fn set_is_subset(a: u64, b: u64) -> Result<bool, String> {
    let sets = sets().lock().unwrap();
    let left = sets
        .get(&handle_key(a))
        .ok_or_else(|| format!("unknown set {a}"))?;
    let right = sets
        .get(&handle_key(b))
        .ok_or_else(|| format!("unknown set {b}"))?;
    Ok(left.is_subset(right))
}

pub fn set_drop(h: u64) -> bool {
    let removed = sets().lock().unwrap().remove(&handle_key(h));
    if let Some(set) = removed {
        let bytes = set.iter().map(String::len).sum();
        release_usage(current_runtime_id(), 1, set.len(), bytes);
        true
    } else {
        false
    }
}

// ---------------- Deque (VecDeque<String>) ----------------

static DEQUES: OnceLock<Mutex<HashMap<(u64, u64), VecDeque<String>>>> = OnceLock::new();
fn deques() -> &'static Mutex<HashMap<(u64, u64), VecDeque<String>>> {
    DEQUES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn deque_new() -> Result<u64, String> {
    let permit = reserve_usage(1, 0, 0)?;
    let handle = next_handle()?;
    deques()
        .lock()
        .unwrap()
        .insert(handle_key(handle), VecDeque::new());
    permit.commit();
    Ok(handle)
}

fn deque_push(h: u64, item: String, front: bool) -> Result<(), String> {
    validate_item(&item)?;
    let mut deques = deques().lock().unwrap();
    let deque = deques
        .get_mut(&handle_key(h))
        .ok_or_else(|| format!("unknown deque {h}"))?;
    ensure_handle_entries(deque.len(), 1)?;
    let permit = reserve_usage(0, 1, item.len())?;
    if front {
        deque.push_front(item);
    } else {
        deque.push_back(item);
    }
    permit.commit();
    Ok(())
}

pub fn deque_push_front(h: u64, item: String) -> Result<(), String> {
    deque_push(h, item, true)
}

pub fn deque_push_back(h: u64, item: String) -> Result<(), String> {
    deque_push(h, item, false)
}

fn deque_pop(h: u64, front: bool) -> Result<Option<String>, String> {
    let removed = {
        let mut deques = deques().lock().unwrap();
        let deque = deques
            .get_mut(&handle_key(h))
            .ok_or_else(|| format!("unknown deque {h}"))?;
        if front {
            deque.pop_front()
        } else {
            deque.pop_back()
        }
    };
    if let Some(item) = &removed {
        release_usage(current_runtime_id(), 0, 1, item.len());
    }
    Ok(removed)
}

pub fn deque_pop_front(h: u64) -> Result<Option<String>, String> {
    deque_pop(h, true)
}

pub fn deque_pop_back(h: u64) -> Result<Option<String>, String> {
    deque_pop(h, false)
}

pub fn deque_len(h: u64) -> Result<usize, String> {
    let deques = deques().lock().unwrap();
    let deque = deques
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown deque {h}"))?;
    Ok(deque.len())
}

pub fn deque_to_array(h: u64) -> Result<Vec<String>, String> {
    let deques = deques().lock().unwrap();
    let deque = deques
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown deque {h}"))?;
    Ok(deque.iter().cloned().collect())
}

pub fn deque_drop(h: u64) -> bool {
    let removed = deques().lock().unwrap().remove(&handle_key(h));
    if let Some(deque) = removed {
        let bytes = deque.iter().map(String::len).sum();
        release_usage(current_runtime_id(), 1, deque.len(), bytes);
        true
    } else {
        false
    }
}

// ---------------- PriorityQueue (BinaryHeap) ----------------
//
// Wrapper con flag min/max. Internamente usa BinaryHeap<(prioridad, seq, item)>.
// El seq garantiza FIFO cuando hay empate — comportamiento estable esperado.

struct PQ {
    is_min: bool,
    heap: BinaryHeap<(i64, i64, String)>, // prioridad_ajustada, -seq, item
    next_seq: i64,
}

static PQS: OnceLock<Mutex<HashMap<(u64, u64), PQ>>> = OnceLock::new();
fn pqs() -> &'static Mutex<HashMap<(u64, u64), PQ>> {
    PQS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pq_new(is_min: bool) -> Result<u64, String> {
    let permit = reserve_usage(1, 0, 0)?;
    let handle = next_handle()?;
    pqs().lock().unwrap().insert(
        handle_key(handle),
        PQ {
            is_min,
            heap: BinaryHeap::new(),
            next_seq: 0,
        },
    );
    permit.commit();
    Ok(handle)
}

pub fn pq_new_max() -> Result<u64, String> {
    pq_new(false)
}

pub fn pq_new_min() -> Result<u64, String> {
    pq_new(true)
}

pub fn pq_push(h: u64, item: String, priority: i64) -> Result<(), String> {
    validate_item(&item)?;
    let mut queues = pqs().lock().unwrap();
    let queue = queues
        .get_mut(&handle_key(h))
        .ok_or_else(|| format!("unknown pq {h}"))?;
    ensure_handle_entries(queue.heap.len(), 1)?;
    let adjusted = if queue.is_min {
        priority
            .checked_neg()
            .ok_or_else(|| "minimum priority cannot be i64::MIN".to_string())?
    } else {
        priority
    };
    let sequence = queue.next_seq;
    let stable_sequence = sequence
        .checked_neg()
        .ok_or_else(|| "priority queue sequence overflow".to_string())?;
    let next_sequence = sequence
        .checked_add(1)
        .ok_or_else(|| "priority queue sequence overflow".to_string())?;
    let permit = reserve_usage(0, 1, item.len())?;
    queue.next_seq = next_sequence;
    queue.heap.push((adjusted, stable_sequence, item));
    permit.commit();
    Ok(())
}

pub fn pq_pop(h: u64) -> Result<Option<String>, String> {
    let removed = {
        let mut queues = pqs().lock().unwrap();
        let queue = queues
            .get_mut(&handle_key(h))
            .ok_or_else(|| format!("unknown pq {h}"))?;
        queue.heap.pop().map(|(_, _, item)| item)
    };
    if let Some(item) = &removed {
        release_usage(current_runtime_id(), 0, 1, item.len());
    }
    Ok(removed)
}

pub fn pq_peek(h: u64) -> Result<Option<String>, String> {
    let queues = pqs().lock().unwrap();
    let queue = queues
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown pq {h}"))?;
    Ok(queue.heap.peek().map(|(_, _, item)| item.clone()))
}

pub fn pq_len(h: u64) -> Result<usize, String> {
    let queues = pqs().lock().unwrap();
    let queue = queues
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown pq {h}"))?;
    Ok(queue.heap.len())
}

pub fn pq_drop(h: u64) -> bool {
    let removed = pqs().lock().unwrap().remove(&handle_key(h));
    if let Some(queue) = removed {
        let bytes = queue.heap.iter().map(|(_, _, item)| item.len()).sum();
        release_usage(current_runtime_id(), 1, queue.heap.len(), bytes);
        true
    } else {
        false
    }
}

// ---------------- OrderedMap (IndexMap<String, serde_json::Value>) ----------------
//
// Map que preserva orden de inserción. Los valores son serde_json::Value
// para poder guardar cualquier cosa; el Value se convierte a Titan Value
// en el layer del VM.

static OMAPS: OnceLock<Mutex<HashMap<(u64, u64), IndexMap<String, serde_json::Value>>>> =
    OnceLock::new();
fn omaps() -> &'static Mutex<HashMap<(u64, u64), IndexMap<String, serde_json::Value>>> {
    OMAPS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct JsonSize {
    bytes: usize,
    exceeded: bool,
}

impl std::io::Write for JsonSize {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let bytes = self
            .bytes
            .checked_add(buffer.len())
            .ok_or_else(|| std::io::Error::other("JSON size overflow"))?;
        if bytes > MAX_ITEM_BYTES {
            self.exceeded = true;
            return Err(std::io::Error::other("JSON item too large"));
        }
        self.bytes = bytes;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn json_bytes(value: &serde_json::Value) -> Result<usize, String> {
    let mut size = JsonSize {
        bytes: 0,
        exceeded: false,
    };
    match serde_json::to_writer(&mut size, value) {
        Ok(()) => Ok(size.bytes),
        Err(_) if size.exceeded => Ok(MAX_ITEM_BYTES + 1),
        Err(error) => Err(format!("cannot measure ordered-map value: {error}")),
    }
}

pub fn omap_new() -> Result<u64, String> {
    let permit = reserve_usage(1, 0, 0)?;
    let handle = next_handle()?;
    omaps()
        .lock()
        .unwrap()
        .insert(handle_key(handle), IndexMap::new());
    permit.commit();
    Ok(handle)
}

pub fn omap_insert(h: u64, key: String, value: serde_json::Value) -> Result<(), String> {
    validate_item(&key)?;
    let value_bytes = json_bytes(&value)?;
    let new_bytes = key
        .len()
        .checked_add(value_bytes)
        .ok_or_else(|| "ordered-map item size overflow".to_string())?;
    if new_bytes > MAX_ITEM_BYTES {
        return Err(format!(
            "ordered-map item exceeds byte limit {MAX_ITEM_BYTES}"
        ));
    }
    let mut maps = omaps().lock().unwrap();
    let map = maps
        .get_mut(&handle_key(h))
        .ok_or_else(|| format!("unknown omap {h}"))?;
    let old_bytes = map
        .get(&key)
        .map(json_bytes)
        .transpose()?
        .map(|value_bytes| key.len() + value_bytes);
    let added_entries = count_true(old_bytes.is_none());
    ensure_handle_entries(map.len(), added_entries)?;
    let added_bytes = new_bytes.saturating_sub(old_bytes.unwrap_or(0));
    let released_bytes = old_bytes.unwrap_or(0).saturating_sub(new_bytes);
    let permit = reserve_usage(0, added_entries, added_bytes)?;
    map.insert(key, value);
    permit.commit();
    if released_bytes > 0 {
        release_usage(current_runtime_id(), 0, 0, released_bytes);
    }
    Ok(())
}

pub fn omap_get(h: u64, key: &str) -> Result<Option<serde_json::Value>, String> {
    let maps = omaps().lock().unwrap();
    let map = maps
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown omap {h}"))?;
    Ok(map.get(key).cloned())
}

pub fn omap_remove(h: u64, key: &str) -> Result<bool, String> {
    let removed = {
        let mut maps = omaps().lock().unwrap();
        let map = maps
            .get_mut(&handle_key(h))
            .ok_or_else(|| format!("unknown omap {h}"))?;
        map.shift_remove_entry(key)
    };
    if let Some((key, value)) = removed {
        let bytes = key.len().saturating_add(json_bytes(&value)?);
        release_usage(current_runtime_id(), 0, 1, bytes);
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn omap_keys(h: u64) -> Result<Vec<String>, String> {
    let maps = omaps().lock().unwrap();
    let map = maps
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown omap {h}"))?;
    Ok(map.keys().cloned().collect())
}

pub fn omap_len(h: u64) -> Result<usize, String> {
    let maps = omaps().lock().unwrap();
    let map = maps
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown omap {h}"))?;
    Ok(map.len())
}

pub fn omap_drop(h: u64) -> bool {
    let removed = omaps().lock().unwrap().remove(&handle_key(h));
    if let Some(map) = removed {
        let bytes = map
            .iter()
            .map(|(key, value)| {
                key.len() + json_bytes(value).expect("stored ordered-map values have valid sizes")
            })
            .sum();
        release_usage(current_runtime_id(), 1, map.len(), bytes);
        true
    } else {
        false
    }
}

// ---------------- Counter (frecuencia de items) ----------------
//
// Encima de HashMap<String, i64>. Ops típicas: from_array, count,
// most_common(n), total.

static COUNTERS: OnceLock<Mutex<HashMap<(u64, u64), HashMap<String, i64>>>> = OnceLock::new();
fn counters() -> &'static Mutex<HashMap<(u64, u64), HashMap<String, i64>>> {
    COUNTERS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn counter_from(items: Vec<String>) -> Result<u64, String> {
    ensure_handle_entries(0, items.len())?;
    let mut counts = HashMap::new();
    for item in items {
        validate_item(&item)?;
        let count = counts.entry(item).or_insert(0i64);
        *count = count
            .checked_add(1)
            .ok_or_else(|| "counter value overflow".to_string())?;
    }
    ensure_handle_entries(0, counts.len())?;
    let bytes = string_bytes(counts.keys())?;
    let permit = reserve_usage(1, counts.len(), bytes)?;
    let handle = next_handle()?;
    counters()
        .lock()
        .unwrap()
        .insert(handle_key(handle), counts);
    permit.commit();
    Ok(handle)
}

pub fn counter_add(h: u64, item: String, delta: i64) -> Result<(), String> {
    validate_item(&item)?;
    let mut counters = counters().lock().unwrap();
    let counter = counters
        .get_mut(&handle_key(h))
        .ok_or_else(|| format!("unknown counter {h}"))?;
    if let Some(value) = counter.get_mut(&item) {
        *value = value
            .checked_add(delta)
            .ok_or_else(|| "counter value overflow".to_string())?;
        return Ok(());
    }
    ensure_handle_entries(counter.len(), 1)?;
    let permit = reserve_usage(0, 1, item.len())?;
    counter.insert(item, delta);
    permit.commit();
    Ok(())
}

pub fn counter_count(h: u64, item: &str) -> Result<i64, String> {
    let counters = counters().lock().unwrap();
    let counter = counters
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown counter {h}"))?;
    Ok(*counter.get(item).unwrap_or(&0))
}

pub fn counter_most_common(h: u64, n: usize) -> Result<Vec<(String, i64)>, String> {
    let counters = counters().lock().unwrap();
    let counter = counters
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown counter {h}"))?;
    let mut values = counter
        .iter()
        .map(|(item, count)| (item.clone(), *count))
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    values.truncate(n);
    Ok(values)
}

pub fn counter_total(h: u64) -> Result<i64, String> {
    let counters = counters().lock().unwrap();
    let counter = counters
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown counter {h}"))?;
    let total = counter
        .values()
        .map(|value| i128::from(*value))
        .sum::<i128>();
    i64::try_from(total).map_err(|_| "counter total overflow".to_string())
}

pub fn counter_drop(h: u64) -> bool {
    let removed = counters().lock().unwrap().remove(&handle_key(h));
    if let Some(counter) = removed {
        let bytes = counter.keys().map(String::len).sum();
        release_usage(current_runtime_id(), 1, counter.len(), bytes);
        true
    } else {
        false
    }
}

// ---------------- Graph (directed/undirected + algoritmos) ----------------

struct Graph {
    directed: bool,
    edges: HashMap<String, Vec<(String, i64)>>, // node -> [(vecino, peso)]
    nodes: BTreeSet<String>,
}

static GRAPHS: OnceLock<Mutex<HashMap<(u64, u64), Graph>>> = OnceLock::new();
fn graphs() -> &'static Mutex<HashMap<(u64, u64), Graph>> {
    GRAPHS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn graph_usage(graph: &Graph) -> (usize, usize) {
    let edge_count = graph.edges.values().map(Vec::len).sum::<usize>();
    let bytes = graph.nodes.iter().map(String::len).sum::<usize>()
        + graph.edges.keys().map(String::len).sum::<usize>()
        + graph
            .edges
            .values()
            .flatten()
            .map(|(node, _)| node.len())
            .sum::<usize>();
    (graph.nodes.len() + edge_count, bytes)
}

pub fn graph_new(directed: bool) -> Result<u64, String> {
    let permit = reserve_usage(1, 0, 0)?;
    let handle = next_handle()?;
    graphs().lock().unwrap().insert(
        handle_key(handle),
        Graph {
            directed,
            edges: HashMap::new(),
            nodes: BTreeSet::new(),
        },
    );
    permit.commit();
    Ok(handle)
}

pub fn graph_add_node(h: u64, node: String) -> Result<(), String> {
    validate_item(&node)?;
    let mut graphs = graphs().lock().unwrap();
    let graph = graphs
        .get_mut(&handle_key(h))
        .ok_or_else(|| format!("unknown graph {h}"))?;
    if graph.nodes.contains(&node) {
        return Ok(());
    }
    let (entries, _) = graph_usage(graph);
    ensure_handle_entries(entries, 1)?;
    let bytes = node
        .len()
        .checked_mul(2)
        .ok_or_else(|| "graph byte count overflow".to_string())?;
    let permit = reserve_usage(0, 1, bytes)?;
    graph.nodes.insert(node.clone());
    graph.edges.insert(node, Vec::new());
    permit.commit();
    Ok(())
}

pub fn graph_add_edge(h: u64, from: String, to: String, weight: i64) -> Result<(), String> {
    validate_item(&from)?;
    validate_item(&to)?;
    if weight < 0 {
        return Err("graph edge weight must be nonnegative for Dijkstra".into());
    }
    let mut graphs = graphs().lock().unwrap();
    let graph = graphs
        .get_mut(&handle_key(h))
        .ok_or_else(|| format!("unknown graph {h}"))?;
    let new_from = !graph.nodes.contains(&from);
    let new_to = !graph.nodes.contains(&to);
    let added_nodes = count_true(new_from) + count_true(new_to && from != to);
    let added_edges = if graph.directed { 1 } else { 2 };
    let (entries, _) = graph_usage(graph);
    ensure_handle_entries(entries, added_nodes + added_edges)?;
    let mut node_bytes = 0usize;
    if new_from {
        node_bytes = from
            .len()
            .checked_mul(2)
            .ok_or_else(|| "graph byte count overflow".to_string())?;
    }
    if new_to && from != to {
        node_bytes = node_bytes
            .checked_add(
                to.len()
                    .checked_mul(2)
                    .ok_or_else(|| "graph byte count overflow".to_string())?,
            )
            .ok_or_else(|| "graph byte count overflow".to_string())?;
    }
    let edge_bytes = if graph.directed {
        to.len()
    } else {
        from.len()
            .checked_add(to.len())
            .ok_or_else(|| "graph byte count overflow".to_string())?
    };
    let bytes = node_bytes
        .checked_add(edge_bytes)
        .ok_or_else(|| "graph byte count overflow".to_string())?;
    let permit = reserve_usage(0, added_nodes + added_edges, bytes)?;
    graph.nodes.insert(from.clone());
    graph.nodes.insert(to.clone());
    graph
        .edges
        .entry(from.clone())
        .or_default()
        .push((to.clone(), weight));
    if graph.directed {
        graph.edges.entry(to).or_default();
    } else {
        graph.edges.entry(to).or_default().push((from, weight));
    }
    permit.commit();
    Ok(())
}

pub fn graph_neighbors(h: u64, node: &str) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown graph {h}"))?;
    Ok(graph
        .edges
        .get(node)
        .map(|v| v.iter().map(|(n, _)| n.clone()).collect())
        .unwrap_or_default())
}

/// BFS: retorna nodos en orden de visita.
pub fn graph_bfs(h: u64, start: &str) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown graph {h}"))?;
    if !graph.nodes.contains(start) {
        return Ok(Vec::new());
    }
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut order: Vec<String> = Vec::new();
    queue.push_back(start.to_string());
    visited.insert(start.to_string());
    while let Some(n) = queue.pop_front() {
        order.push(n.clone());
        if let Some(nbrs) = graph.edges.get(&n) {
            let mut sorted: Vec<&(String, i64)> = nbrs.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            for (nbr, _) in sorted {
                if !visited.contains(nbr) {
                    visited.insert(nbr.clone());
                    queue.push_back(nbr.clone());
                }
            }
        }
    }
    Ok(order)
}

/// DFS iterativo con stack.
pub fn graph_dfs(h: u64, start: &str) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown graph {h}"))?;
    if !graph.nodes.contains(start) {
        return Ok(Vec::new());
    }
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = vec![start.to_string()];
    let mut order: Vec<String> = Vec::new();
    while let Some(n) = stack.pop() {
        if visited.contains(&n) {
            continue;
        }
        visited.insert(n.clone());
        order.push(n.clone());
        if let Some(nbrs) = graph.edges.get(&n) {
            let mut sorted: Vec<&(String, i64)> = nbrs.iter().collect();
            sorted.sort_by(|a, b| b.0.cmp(&a.0)); // reverse para que DFS visite en orden alfabético
            for (nbr, _) in sorted {
                if !visited.contains(nbr) {
                    stack.push(nbr.clone());
                }
            }
        }
    }
    Ok(order)
}

/// Dijkstra: shortest path desde start hasta end. Retorna la lista de
/// nodos en el camino (incluye start y end). Vacío si no hay camino.
pub fn graph_shortest_path(h: u64, start: &str, end: &str) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown graph {h}"))?;
    if !graph.nodes.contains(start) || !graph.nodes.contains(end) {
        return Ok(Vec::new());
    }

    let mut dist: HashMap<String, i64> = HashMap::new();
    let mut prev: HashMap<String, String> = HashMap::new();
    let mut heap: BinaryHeap<(std::cmp::Reverse<i64>, String)> = BinaryHeap::new();
    dist.insert(start.to_string(), 0);
    heap.push((std::cmp::Reverse(0), start.to_string()));

    while let Some((std::cmp::Reverse(d), u)) = heap.pop() {
        if u == end {
            break;
        }
        if d > *dist.get(&u).unwrap_or(&i64::MAX) {
            continue;
        }
        if let Some(nbrs) = graph.edges.get(&u) {
            for (v, w) in nbrs {
                let alt = d.saturating_add(*w);
                if alt < *dist.get(v).unwrap_or(&i64::MAX) {
                    dist.insert(v.clone(), alt);
                    prev.insert(v.clone(), u.clone());
                    heap.push((std::cmp::Reverse(alt), v.clone()));
                }
            }
        }
    }

    if !dist.contains_key(end) {
        return Ok(Vec::new());
    }
    let mut path: Vec<String> = Vec::new();
    let mut cur = end.to_string();
    loop {
        path.push(cur.clone());
        if cur == start {
            break;
        }
        match prev.get(&cur) {
            Some(p) => cur = p.clone(),
            None => return Ok(Vec::new()),
        }
    }
    path.reverse();
    Ok(path)
}

/// Topological sort. Retorna orden válido o Err si hay ciclo.
pub fn graph_topological_sort(h: u64) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown graph {h}"))?;
    if !graph.directed {
        return Err("topological_sort requires directed graph".into());
    }
    let mut in_degree: HashMap<String, i64> = HashMap::new();
    for n in &graph.nodes {
        in_degree.insert(n.clone(), 0);
    }
    for (_, nbrs) in &graph.edges {
        for (v, _) in nbrs {
            *in_degree.entry(v.clone()).or_insert(0) += 1;
        }
    }
    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(n, _)| n.clone())
        .collect();
    let mut sorted_queue: Vec<String> = queue.iter().cloned().collect();
    sorted_queue.sort();
    queue = sorted_queue.into_iter().collect();
    let mut order: Vec<String> = Vec::new();
    while let Some(n) = queue.pop_front() {
        order.push(n.clone());
        if let Some(nbrs) = graph.edges.get(&n) {
            let mut nbrs_sorted: Vec<&(String, i64)> = nbrs.iter().collect();
            nbrs_sorted.sort_by(|a, b| a.0.cmp(&b.0));
            for (v, _) in nbrs_sorted {
                let d = in_degree.entry(v.clone()).or_insert(0);
                *d -= 1;
                if *d == 0 {
                    queue.push_back(v.clone());
                }
            }
        }
    }
    if order.len() != graph.nodes.len() {
        return Err("cycle detected: topological sort not possible".into());
    }
    Ok(order)
}

/// Detecta si el grafo tiene ciclo (funciona en directed y undirected).
pub fn graph_has_cycle(h: u64) -> Result<bool, String> {
    let g = graphs().lock().unwrap();
    let graph = g
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown graph {h}"))?;
    if graph.directed {
        // DFS iterativo con tres estados para que una cadena válida no pueda
        // desbordar la pila nativa de Rust.
        let mut state = graph
            .nodes
            .iter()
            .map(|node| (node.clone(), 0u8))
            .collect::<HashMap<_, _>>();
        for start in &graph.nodes {
            if state.get(start).copied().unwrap_or(0) != 0 {
                continue;
            }
            state.insert(start.clone(), 1);
            let mut stack = vec![(start.clone(), 0usize)];
            while !stack.is_empty() {
                let next = {
                    let (node, index) = stack.last_mut().expect("stack is nonempty");
                    let neighbor = graph
                        .edges
                        .get(node)
                        .and_then(|neighbors| neighbors.get(*index))
                        .map(|(neighbor, _)| neighbor.clone());
                    if neighbor.is_some() {
                        *index += 1;
                    }
                    neighbor
                };
                if let Some(neighbor) = next {
                    match state.get(&neighbor).copied().unwrap_or(0) {
                        1 => return Ok(true),
                        2 => {}
                        _ => {
                            state.insert(neighbor.clone(), 1);
                            stack.push((neighbor, 0));
                        }
                    }
                } else {
                    let (completed, _) = stack.pop().expect("stack is nonempty");
                    state.insert(completed, 2);
                }
            }
        }
        Ok(false)
    } else {
        // BFS/DFS tracking parent — si visitamos un nodo ya visitado
        // que no es el parent, hay ciclo.
        let mut visited: BTreeSet<String> = BTreeSet::new();
        for start in &graph.nodes {
            if visited.contains(start) {
                continue;
            }
            let mut queue: VecDeque<(String, Option<String>)> = VecDeque::new();
            queue.push_back((start.clone(), None));
            while let Some((node, parent)) = queue.pop_front() {
                if visited.contains(&node) {
                    return Ok(true);
                }
                visited.insert(node.clone());
                if let Some(neighbors) = graph.edges.get(&node) {
                    for (neighbor, _) in neighbors {
                        if parent.as_ref() != Some(neighbor) {
                            queue.push_back((neighbor.clone(), Some(node.clone())));
                        }
                    }
                }
            }
        }
        Ok(false)
    }
}

pub fn graph_nodes(h: u64) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g
        .get(&handle_key(h))
        .ok_or_else(|| format!("unknown graph {h}"))?;
    Ok(graph.nodes.iter().cloned().collect())
}

pub fn graph_drop(h: u64) -> bool {
    let removed = graphs().lock().unwrap().remove(&handle_key(h));
    if let Some(graph) = removed {
        let (entries, bytes) = graph_usage(&graph);
        release_usage(current_runtime_id(), 1, entries, bytes);
        true
    } else {
        false
    }
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut released =
        crate::native::remove_runtime_entries(&mut crate::native::lock_recover(sets()), runtime_id);
    released += crate::native::remove_runtime_entries(
        &mut crate::native::lock_recover(deques()),
        runtime_id,
    );
    released +=
        crate::native::remove_runtime_entries(&mut crate::native::lock_recover(pqs()), runtime_id);
    released += crate::native::remove_runtime_entries(
        &mut crate::native::lock_recover(omaps()),
        runtime_id,
    );
    released += crate::native::remove_runtime_entries(
        &mut crate::native::lock_recover(counters()),
        runtime_id,
    );
    released += crate::native::remove_runtime_entries(
        &mut crate::native::lock_recover(graphs()),
        runtime_id,
    );
    crate::native::lock_recover(usage()).remove(&runtime_id);
    released
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_basics() {
        let s = set_new().unwrap();
        assert!(set_add(s, "a".into()).unwrap());
        assert!(!set_add(s, "a".into()).unwrap());
        assert_eq!(set_len(s).unwrap(), 1);
        assert!(set_contains(s, "a").unwrap());
        assert!(set_drop(s));
    }

    #[test]
    fn set_union_intersect() {
        let a = set_from(vec!["1".into(), "2".into(), "3".into()]).unwrap();
        let b = set_from(vec!["2".into(), "3".into(), "4".into()]).unwrap();
        let u = set_union(a, b).unwrap();
        let i = set_intersect(a, b).unwrap();
        assert_eq!(set_len(u).unwrap(), 4);
        assert_eq!(set_len(i).unwrap(), 2);
        for handle in [a, b, u, i] {
            assert!(set_drop(handle));
        }
    }

    #[test]
    fn pq_min_returns_smallest_first() {
        let pq = pq_new_min().unwrap();
        pq_push(pq, "b".into(), 5).unwrap();
        pq_push(pq, "a".into(), 1).unwrap();
        pq_push(pq, "c".into(), 3).unwrap();
        assert_eq!(pq_pop(pq).unwrap().unwrap(), "a");
        assert_eq!(pq_pop(pq).unwrap().unwrap(), "c");
        assert_eq!(pq_pop(pq).unwrap().unwrap(), "b");
        assert!(pq_drop(pq));
    }

    #[test]
    fn counter_most_common_sorted() {
        let c = counter_from(vec![
            "a".into(),
            "b".into(),
            "a".into(),
            "c".into(),
            "a".into(),
            "b".into(),
        ])
        .unwrap();
        let top = counter_most_common(c, 2).unwrap();
        assert_eq!(top[0], ("a".into(), 3));
        assert_eq!(top[1], ("b".into(), 2));
        assert!(counter_drop(c));
    }

    #[test]
    fn graph_dijkstra_finds_shortest() {
        let g = graph_new(false).unwrap();
        graph_add_edge(g, "a".into(), "b".into(), 1).unwrap();
        graph_add_edge(g, "b".into(), "c".into(), 1).unwrap();
        graph_add_edge(g, "a".into(), "c".into(), 5).unwrap();
        let path = graph_shortest_path(g, "a", "c").unwrap();
        assert_eq!(path, vec!["a", "b", "c"]);
        assert!(graph_drop(g));
    }

    #[test]
    fn graph_toposort_valid() {
        let g = graph_new(true).unwrap();
        graph_add_edge(g, "a".into(), "b".into(), 0).unwrap();
        graph_add_edge(g, "b".into(), "c".into(), 0).unwrap();
        graph_add_edge(g, "a".into(), "c".into(), 0).unwrap();
        let order = graph_topological_sort(g).unwrap();
        assert_eq!(order[0], "a");
        assert_eq!(order[order.len() - 1], "c");
        assert!(graph_drop(g));
    }

    #[test]
    fn graph_cycle_detected() {
        let g = graph_new(true).unwrap();
        graph_add_edge(g, "a".into(), "b".into(), 0).unwrap();
        graph_add_edge(g, "b".into(), "a".into(), 0).unwrap();
        assert!(graph_has_cycle(g).unwrap());
        assert!(graph_drop(g));
    }

    #[test]
    fn handle_quota_is_shared_and_recovers_after_drop() {
        let runtime_id = 8_100_001;
        crate::native::with_runtime_context(runtime_id, || {
            let mut handles = (0..MAX_COLLECTION_HANDLES)
                .map(|_| set_new().unwrap())
                .collect::<Vec<_>>();
            assert!(set_new().unwrap_err().contains("handle quota"));
            assert!(set_drop(handles.pop().unwrap()));
            handles.push(deque_new().unwrap());
            assert_eq!(runtime_usage(runtime_id).handles, MAX_COLLECTION_HANDLES);
        });
        assert_eq!(cleanup_runtime(runtime_id), MAX_COLLECTION_HANDLES);
        assert_eq!(runtime_usage(runtime_id), CollectionUsage::default());
    }

    #[test]
    fn per_handle_entry_quota_rejects_and_recovers() {
        let runtime_id = 8_100_002;
        crate::native::with_runtime_context(runtime_id, || {
            let set = set_new().unwrap();
            for index in 0..MAX_ENTRIES_PER_HANDLE {
                assert!(set_add(set, index.to_string()).unwrap());
            }
            assert!(set_add(set, "overflow".into())
                .unwrap_err()
                .contains("handle entry quota"));
            assert!(set_remove(set, "0").unwrap());
            assert!(set_add(set, "recovered".into()).unwrap());
            assert_eq!(set_len(set).unwrap(), MAX_ENTRIES_PER_HANDLE);
            assert!(set_drop(set));
        });
        assert_eq!(runtime_usage(runtime_id), CollectionUsage::default());
    }

    #[test]
    fn aggregate_byte_quota_rejects_and_recovers() {
        fn full_item(index: usize) -> String {
            let mut item = "x".repeat(MAX_ITEM_BYTES);
            item.replace_range(..8, &format!("{index:08x}"));
            item
        }

        let runtime_id = 8_100_003;
        crate::native::with_runtime_context(runtime_id, || {
            let set = set_new().unwrap();
            let first = full_item(0);
            assert!(set_add(set, first.clone()).unwrap());
            for index in 1..(MAX_COLLECTION_BYTES / MAX_ITEM_BYTES) {
                assert!(set_add(set, full_item(index)).unwrap());
            }
            let replacement = full_item(MAX_COLLECTION_BYTES / MAX_ITEM_BYTES);
            assert!(set_add(set, replacement.clone())
                .unwrap_err()
                .contains("byte quota"));
            assert!(set_remove(set, &first).unwrap());
            assert!(set_add(set, replacement).unwrap());
            assert_eq!(runtime_usage(runtime_id).bytes, MAX_COLLECTION_BYTES);
            assert!(set_drop(set));
        });
        assert_eq!(runtime_usage(runtime_id), CollectionUsage::default());
    }

    #[test]
    fn aggregate_entry_quota_is_atomic_and_recovers() {
        let runtime_id = 8_100_006;
        crate::native::with_runtime_context(runtime_id, || {
            let full = reserve_usage(0, MAX_COLLECTION_ENTRIES, 0).unwrap();
            full.commit();
            assert!(reserve_usage(0, 1, 0).unwrap_err().contains("entry quota"));
            release_usage(runtime_id, 0, 1, 0);
            let replacement = reserve_usage(0, 1, 0).unwrap();
            replacement.commit();
            release_usage(runtime_id, 0, MAX_COLLECTION_ENTRIES, 0);
        });
        assert_eq!(runtime_usage(runtime_id), CollectionUsage::default());
    }

    #[test]
    fn removals_and_replacements_return_bytes_across_collection_families() {
        let runtime_id = 8_100_007;
        crate::native::with_runtime_context(runtime_id, || {
            let deque = deque_new().unwrap();
            deque_push_back(deque, "deque".into()).unwrap();
            assert_eq!(deque_pop_front(deque).unwrap().as_deref(), Some("deque"));

            let queue = pq_new_max().unwrap();
            pq_push(queue, "queue".into(), 1).unwrap();
            assert_eq!(pq_pop(queue).unwrap().as_deref(), Some("queue"));

            let map = omap_new().unwrap();
            omap_insert(map, "key".into(), serde_json::json!("a long value")).unwrap();
            let before = runtime_usage(runtime_id).bytes;
            omap_insert(map, "key".into(), serde_json::Value::Null).unwrap();
            assert!(runtime_usage(runtime_id).bytes < before);
            assert!(omap_remove(map, "key").unwrap());

            assert!(deque_drop(deque));
            assert!(pq_drop(queue));
            assert!(omap_drop(map));
        });
        assert_eq!(runtime_usage(runtime_id), CollectionUsage::default());
    }

    #[test]
    fn oversized_items_and_arithmetic_overflow_are_rejected() {
        let runtime_id = 8_100_004;
        crate::native::with_runtime_context(runtime_id, || {
            let set = set_new().unwrap();
            assert!(set_add(set, "x".repeat(MAX_ITEM_BYTES + 1))
                .unwrap_err()
                .contains("item exceeds"));

            let queue = pq_new_min().unwrap();
            assert!(pq_push(queue, "minimum".into(), i64::MIN)
                .unwrap_err()
                .contains("i64::MIN"));

            let counter = counter_from(vec!["x".into()]).unwrap();
            counter_add(counter, "x".into(), i64::MAX - 1).unwrap();
            assert!(counter_add(counter, "x".into(), 1)
                .unwrap_err()
                .contains("overflow"));

            let total = counter_from(vec!["a".into(), "b".into()]).unwrap();
            counter_add(total, "a".into(), i64::MAX - 1).unwrap();
            counter_add(total, "b".into(), i64::MAX - 1).unwrap();
            assert!(counter_total(total).unwrap_err().contains("total overflow"));

            assert!(set_drop(set));
            assert!(pq_drop(queue));
            assert!(counter_drop(counter));
            assert!(counter_drop(total));
        });
        assert_eq!(runtime_usage(runtime_id), CollectionUsage::default());
    }

    #[test]
    fn graphs_reject_negative_weights_and_detect_deep_cycles_iteratively() {
        let runtime_id = 8_100_005;
        crate::native::with_runtime_context(runtime_id, || {
            let graph = graph_new(true).unwrap();
            assert!(graph_add_edge(graph, "a".into(), "b".into(), -1)
                .unwrap_err()
                .contains("nonnegative"));
            for index in 0..1_500 {
                graph_add_node(graph, index.to_string()).unwrap();
                if index > 0 {
                    graph_add_edge(graph, (index - 1).to_string(), index.to_string(), 1).unwrap();
                }
            }
            assert!(!graph_has_cycle(graph).unwrap());
            graph_add_edge(graph, "1499".into(), "0".into(), 1).unwrap();
            assert!(graph_has_cycle(graph).unwrap());
            assert!(graph_drop(graph));
        });
        assert_eq!(runtime_usage(runtime_id), CollectionUsage::default());
    }
}
