//! Capacity-bounded least-recently-used cache.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

#[derive(Debug, Clone)] pub struct LruCache<K, V> { capacity: usize, values: HashMap<K, V>, order: VecDeque<K> }
impl<K: Eq + Hash + Clone, V> LruCache<K, V> {
    pub fn new(capacity: usize) -> Self { Self { capacity, values: HashMap::with_capacity(capacity), order: VecDeque::with_capacity(capacity) } }
    pub fn capacity(&self) -> usize { self.capacity }
    pub fn len(&self) -> usize { self.values.len() }
    pub fn is_empty(&self) -> bool { self.values.is_empty() }
    pub fn contains_key(&self, key: &K) -> bool { self.values.contains_key(key) }
    pub fn get(&mut self, key: &K) -> Option<&V> { if self.values.contains_key(key) { self.touch(key); self.values.get(key) } else { None } }
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> { if self.values.contains_key(key) { self.touch(key); self.values.get_mut(key) } else { None } }
    pub fn insert(&mut self, key: K, value: V) -> Option<(K, V)> {
        if self.capacity == 0 { return Some((key, value)); }
        if self.values.contains_key(&key) { self.values.insert(key.clone(), value); self.touch(&key); return None; }
        self.order.push_back(key.clone()); self.values.insert(key, value);
        if self.values.len() > self.capacity { let oldest = self.order.pop_front().unwrap(); let value = self.values.remove(&oldest).unwrap(); Some((oldest, value)) } else { None }
    }
    pub fn remove(&mut self, key: &K) -> Option<V> { self.order.retain(|item| item != key); self.values.remove(key) }
    pub fn clear(&mut self) { self.order.clear(); self.values.clear(); }
    pub fn iter_mru(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> { self.order.iter().rev().filter_map(|key| self.values.get_key_value(key)) }
    fn touch(&mut self, key: &K) { self.order.retain(|item| item != key); self.order.push_back(key.clone()); }
}

#[cfg(test)] mod tests { use super::*; #[test] fn evicts_least_recently_used() { let mut cache = LruCache::new(2); cache.insert("a", 1); cache.insert("b", 2); assert_eq!(cache.get(&"a"), Some(&1)); assert_eq!(cache.insert("c", 3), Some(("b", 2))); assert!(!cache.contains_key(&"b")); } }
