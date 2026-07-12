//! Deterministic tracing mark-and-sweep heap metadata for the Titan runtime.

use std::collections::{HashMap, HashSet};

pub type GcRef = usize;

#[derive(Debug)]
struct Object { size: usize, references: Vec<GcRef> }

pub struct GarbageCollector {
    objects: HashMap<GcRef, Object>,
    next_id: GcRef,
    roots: HashSet<GcRef>,
    allocated_bytes: usize,
    gc_threshold: usize,
}

impl GarbageCollector {
    pub fn new() -> Self { Self { objects: HashMap::new(), next_id: 1, roots: HashSet::new(), allocated_bytes: 0, gc_threshold: 1024 * 1024 } }
    pub fn with_threshold(bytes: usize) -> Self { Self { gc_threshold: bytes.max(1), ..Self::new() } }

    pub fn allocate(&mut self, size: usize) -> GcRef {
        if self.allocated_bytes.saturating_add(size) >= self.gc_threshold { self.collect(); }
        let id = self.next_id; self.next_id += 1;
        self.objects.insert(id, Object { size, references: Vec::new() });
        self.allocated_bytes = self.allocated_bytes.saturating_add(size); id
    }
    pub fn contains(&self, reference: GcRef) -> bool { self.objects.contains_key(&reference) }
    pub fn set_references(&mut self, object: GcRef, references: Vec<GcRef>) -> bool {
        if references.iter().any(|r| !self.objects.contains_key(r)) { return false; }
        if let Some(value) = self.objects.get_mut(&object) { value.references = references; true } else { false }
    }
    pub fn add_root(&mut self, reference: GcRef) -> bool { self.objects.contains_key(&reference) && self.roots.insert(reference) }
    pub fn remove_root(&mut self, reference: GcRef) -> bool { self.roots.remove(&reference) }

    pub fn collect(&mut self) -> usize {
        let mut marked = HashSet::new(); let mut pending: Vec<_> = self.roots.iter().copied().collect();
        while let Some(reference) = pending.pop() {
            if !marked.insert(reference) { continue; }
            if let Some(object) = self.objects.get(&reference) { pending.extend(object.references.iter().copied()); }
        }
        let before = self.objects.len(); self.objects.retain(|reference, _| marked.contains(reference));
        self.roots.retain(|reference| self.objects.contains_key(reference));
        self.allocated_bytes = self.objects.values().map(|object| object.size).sum(); before - self.objects.len()
    }
    pub fn live_count(&self) -> usize { self.objects.len() }
    pub fn allocated_bytes(&self) -> usize { self.allocated_bytes }
}

impl Default for GarbageCollector { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn traces_transitive_references() {
        let mut gc = GarbageCollector::new(); let root = gc.allocate(8); let child = gc.allocate(8); let dead = gc.allocate(8);
        assert!(gc.set_references(root, vec![child])); assert!(gc.add_root(root)); assert_eq!(gc.collect(), 1);
        assert!(gc.contains(root) && gc.contains(child) && !gc.contains(dead));
        gc.remove_root(root); assert_eq!(gc.collect(), 2);
    }
}
