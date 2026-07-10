//! Titan Runtime — Fiber scheduler.

use std::collections::VecDeque;

pub type FiberId = u64;

pub struct Scheduler {
    fibers: Vec<Fiber>,
    run_queue: VecDeque<FiberId>,
    next_id: FiberId,
}

pub struct Fiber {
    pub id: FiberId,
    pub parent: Option<FiberId>,
    pub children: Vec<FiberId>,
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler { fibers: Vec::new(), run_queue: VecDeque::new(), next_id: 1 }
    }

    pub fn spawn(&mut self, parent: Option<FiberId>) -> FiberId {
        let id = self.next_id;
        self.next_id += 1;
        let fiber = Fiber { id, parent, children: Vec::new() };
        if let Some(pid) = parent {
            if let Some(p) = self.fibers.iter_mut().find(|f| f.id == pid) {
                p.children.push(id);
            }
        }
        self.fibers.push(fiber);
        self.run_queue.push_back(id);
        id
    }

    pub fn next(&mut self) -> Option<FiberId> { self.run_queue.pop_front() }
    pub fn wake(&mut self, fiber_id: FiberId) { self.run_queue.push_back(fiber_id); }
}

impl Default for Scheduler {
    fn default() -> Self { Self::new() }
}