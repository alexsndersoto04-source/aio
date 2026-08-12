//! Cooperative fiber scheduling primitives for the Titan runtime.

use std::collections::VecDeque;

pub type FiberId = u64;

pub struct Scheduler {
    fibers: Vec<Fiber>,
    run_queue: VecDeque<FiberId>,
    next_id: FiberId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fiber {
    pub id: FiberId,
    pub parent: Option<FiberId>,
    pub children: Vec<FiberId>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            fibers: Vec::new(),
            run_queue: VecDeque::new(),
            next_id: 1,
        }
    }

    pub fn spawn(&mut self, requested_parent: Option<FiberId>) -> FiberId {
        let id = self.next_id;
        self.next_id += 1;
        let parent =
            requested_parent.filter(|parent| self.fibers.iter().any(|fiber| fiber.id == *parent));
        if let Some(parent) = parent {
            if let Some(fiber) = self.fibers.iter_mut().find(|fiber| fiber.id == parent) {
                fiber.children.push(id);
            }
        }
        self.fibers.push(Fiber {
            id,
            parent,
            children: Vec::new(),
        });
        self.run_queue.push_back(id);
        id
    }

    pub fn wake(&mut self, fiber_id: FiberId) -> bool {
        if !self.fibers.iter().any(|fiber| fiber.id == fiber_id)
            || self.run_queue.contains(&fiber_id)
        {
            return false;
        }
        self.run_queue.push_back(fiber_id);
        true
    }

    pub fn fiber(&self, fiber_id: FiberId) -> Option<&Fiber> {
        self.fibers.iter().find(|fiber| fiber.id == fiber_id)
    }
    pub fn is_queued(&self, fiber_id: FiberId) -> bool {
        self.run_queue.contains(&fiber_id)
    }
    pub fn fiber_count(&self) -> usize {
        self.fibers.len()
    }
    pub fn queued_count(&self) -> usize {
        self.run_queue.len()
    }
}

impl Iterator for Scheduler {
    type Item = FiberId;
    fn next(&mut self) -> Option<Self::Item> {
        self.run_queue.pop_front()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_fifo_without_duplicate_wakes() {
        let mut scheduler = Scheduler::new();
        let first = scheduler.spawn(None);
        let second = scheduler.spawn(None);
        assert_eq!(scheduler.next(), Some(first));
        assert!(scheduler.wake(first));
        assert!(!scheduler.wake(first));
        assert_eq!(scheduler.collect::<Vec<_>>(), vec![second, first]);
    }

    #[test]
    fn records_only_valid_parent_relationships() {
        let mut scheduler = Scheduler::new();
        let parent = scheduler.spawn(None);
        let child = scheduler.spawn(Some(parent));
        let orphan = scheduler.spawn(Some(999));
        assert_eq!(scheduler.fiber(parent).unwrap().children, vec![child]);
        assert_eq!(scheduler.fiber(child).unwrap().parent, Some(parent));
        assert_eq!(scheduler.fiber(orphan).unwrap().parent, None);
    }
}
