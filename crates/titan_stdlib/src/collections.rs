//! Generic collection helpers backed by Rust's memory-safe standard library.

use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
use std::hash::Hash;

pub type Vec<T> = std::vec::Vec<T>;
pub type HashMap<K, V> = std::collections::HashMap<K, V>;
pub type HashSet<T> = std::collections::HashSet<T>;
pub type OrderedMap<K, V> = BTreeMap<K, V>;
pub type OrderedSet<T> = BTreeSet<T>;
pub type Deque<T> = VecDeque<T>;
pub type MaxHeap<T> = BinaryHeap<T>;

pub fn vec_new<T>() -> Vec<T> {
    Vec::new()
}
pub fn vec_with_capacity<T>(capacity: usize) -> Vec<T> {
    Vec::with_capacity(capacity)
}
pub fn vec_push<T>(values: &mut Vec<T>, value: T) {
    values.push(value);
}
pub fn vec_pop<T>(values: &mut Vec<T>) -> Option<T> {
    values.pop()
}
pub fn vec_len<T>(values: &[T]) -> usize {
    values.len()
}
pub fn deduplicate<T: Eq + Hash + Clone>(values: &[T]) -> Vec<T> {
    let mut seen = HashSet::new();
    values
        .iter()
        .filter(|value| seen.insert((*value).clone()))
        .cloned()
        .collect()
}
pub fn frequencies<T: Eq + Hash + Clone>(values: &[T]) -> HashMap<T, usize> {
    let mut counts = HashMap::new();
    for value in values {
        *counts.entry(value.clone()).or_insert(0) += 1;
    }
    counts
}
pub fn group_by<T, K: Eq + Hash>(
    values: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> K,
) -> HashMap<K, Vec<T>> {
    let mut groups = HashMap::new();
    for value in values {
        groups
            .entry(key(&value))
            .or_insert_with(Vec::new)
            .push(value);
    }
    groups
}
pub fn partition<T>(
    values: impl IntoIterator<Item = T>,
    predicate: impl Fn(&T) -> bool,
) -> (Vec<T>, Vec<T>) {
    values.into_iter().partition(predicate)
}
pub fn chunks<T: Clone>(values: &[T], size: usize) -> Vec<Vec<T>> {
    if size == 0 {
        Vec::new()
    } else {
        values.chunks(size).map(|chunk| chunk.to_vec()).collect()
    }
}
pub fn windows<T: Clone>(values: &[T], size: usize) -> Vec<Vec<T>> {
    if size == 0 {
        Vec::new()
    } else {
        values.windows(size).map(|window| window.to_vec()).collect()
    }
}
pub fn zip<A: Clone, B: Clone>(left: &[A], right: &[B]) -> Vec<(A, B)> {
    left.iter().cloned().zip(right.iter().cloned()).collect()
}
pub fn binary_search<T: Ord>(values: &[T], target: &T) -> Result<usize, usize> {
    values.binary_search(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collection_algorithms() {
        assert_eq!(deduplicate(&[1, 2, 1, 3]), vec![1, 2, 3]);
        assert_eq!(frequencies(&["a", "b", "a"])["a"], 2);
        assert_eq!(chunks(&[1, 2, 3], 2), vec![vec![1, 2], vec![3]]);
    }
}
