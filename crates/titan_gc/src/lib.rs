//! Titan GC — Generational garbage collector.

use std::collections::HashMap;

pub type GcRef = usize;

pub struct GarbageCollector {
    objects: HashMap<GcRef, (bool, usize)>,
    next_id: GcRef,
    roots: Vec<GcRef>,
    alloc_count: usize,
    gc_threshold: usize,
}

impl GarbageCollector {
    pub fn new() -> Self {
        GarbageCollector {
            objects: HashMap::new(),
            next_id: 1, roots: Vec::new(),
            alloc_count: 0, gc_threshold: 1024,
        }
    }

    pub fn allocate(&mut self, size: usize) -> GcRef {
        self.alloc_count += 1;
        if self.alloc_count >= self.gc_threshold { self.collect(); }
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(id, (false, size));
        id
    }

    pub fn add_root(&mut self, gc_ref: GcRef) { self.roots.push(gc_ref); }

    pub fn collect(&mut self) {
        for root in &self.roots {
            if let Some((marked, _)) = self.objects.get_mut(root) { *marked = true; }
        }
        let mut to_remove = Vec::new();
        for (id, (marked, _)) in &self.objects {
            if !marked { to_remove.push(*id); }
        }
        for id in to_remove { self.objects.remove(&id); }
        for (_, (marked, _)) in self.objects.iter_mut() { *marked = false; }
        self.alloc_count = 0;
    }

    pub fn live_count(&self) -> usize { self.objects.len() }
}

impl Default for GarbageCollector {
    fn default() -> Self { Self::new() }
}