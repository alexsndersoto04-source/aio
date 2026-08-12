use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

pub const PAGE_SIZE: u64 = 4096; // 0x1000 bytes

struct MemoryState {
    initialized: bool,
    base_paddr: u64,
    total_frames: u64,
    allocated_frames: HashSet<u64>,
    recycled_frames: Vec<u64>,
    next_frame: u64,
    page_table: HashMap<u64, (u64, u32)>, // vaddr -> (paddr, flags)
}

impl MemoryState {
    fn new() -> Self {
        Self {
            initialized: false,
            base_paddr: 0,
            total_frames: 0,
            allocated_frames: HashSet::new(),
            recycled_frames: Vec::new(),
            next_frame: 0,
            page_table: HashMap::new(),
        }
    }
}

fn memory_states() -> &'static Mutex<HashMap<u64, Arc<Mutex<MemoryState>>>> {
    static STATES: OnceLock<Mutex<HashMap<u64, Arc<Mutex<MemoryState>>>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_memory_state() -> Arc<Mutex<MemoryState>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut states = crate::native::lock_recover(memory_states());
    Arc::clone(states.entry(runtime_id).or_insert_with(|| Arc::new(Mutex::new(MemoryState::new()))))
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(crate::native::lock_recover(memory_states()).remove(&runtime_id).is_some())
}

pub fn init_frame_allocator(base_paddr: u64, total_size_bytes: u64) -> bool {
    let Some(aligned_base) = base_paddr
        .checked_add(PAGE_SIZE - 1)
        .map(|address| address & !(PAGE_SIZE - 1))
    else {
        return false;
    };
    let total_frames = total_size_bytes / PAGE_SIZE;
    let Some(last_offset) = total_frames
        .checked_sub(1)
        .and_then(|frames| frames.checked_mul(PAGE_SIZE))
    else {
        return false;
    };
    if aligned_base.checked_add(last_offset).is_none() {
        return false;
    }

    if let Ok(mut state) = get_memory_state().lock() {
        state.base_paddr = aligned_base;
        state.total_frames = total_frames;
        state.allocated_frames.clear();
        state.recycled_frames.clear();
        state.next_frame = 0;
        state.page_table.clear();
        state.initialized = true;
        true
    } else {
        false
    }
}

pub fn allocate_frame() -> u64 {
    if let Ok(mut state) = get_memory_state().lock() {
        if !state.initialized {
            return 0;
        }
        let frame = if let Some(frame) = state.recycled_frames.pop() {
            frame
        } else if state.next_frame < state.total_frames {
            let frame = state.base_paddr + state.next_frame * PAGE_SIZE;
            state.next_frame += 1;
            frame
        } else {
            return 0;
        };
        state.allocated_frames.insert(frame);
        return frame;
    }
    0
}

pub fn deallocate_frame(paddr: u64) -> bool {
    if let Ok(mut state) = get_memory_state().lock() {
        if !state.initialized || !paddr.is_multiple_of(PAGE_SIZE) {
            return false;
        }
        if state.allocated_frames.remove(&paddr) {
            state.recycled_frames.push(paddr);
            return true;
        }
    }
    false
}

pub fn map_page(vaddr: u64, paddr: u64, flags: u32) -> bool {
    if let Ok(mut state) = get_memory_state().lock() {
        if !state.initialized
            || !vaddr.is_multiple_of(PAGE_SIZE)
            || !paddr.is_multiple_of(PAGE_SIZE)
        {
            return false;
        }
        state.page_table.insert(vaddr, (paddr, flags));
        true
    } else {
        false
    }
}

pub fn translate_page(vaddr: u64) -> u64 {
    if let Ok(state) = get_memory_state().lock() {
        let aligned_vaddr = vaddr & !(PAGE_SIZE - 1);
        if let Some(&(paddr, _flags)) = state.page_table.get(&aligned_vaddr) {
            let offset = vaddr & (PAGE_SIZE - 1);
            return paddr + offset;
        }
    }
    0
}

pub fn free_frames_count() -> u64 {
    if let Ok(state) = get_memory_state().lock() {
        if state.initialized {
            return state
                .total_frames
                .saturating_sub(state.allocated_frames.len() as u64);
        }
    }
    0
}

pub fn shutdown() -> bool {
    if let Ok(mut state) = get_memory_state().lock() {
        state.recycled_frames.clear();
        state.allocated_frames.clear();
        state.page_table.clear();
        state.next_frame = 0;
        state.initialized = false;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bare_metal_paging_and_frame_allocator() {
        assert!(init_frame_allocator(0x100000, 0x10000)); // 64KB = 16 frames de 4KB
        assert_eq!(free_frames_count(), 16);

        let frame1 = allocate_frame();
        assert_eq!(frame1, 0x100000);
        assert_eq!(free_frames_count(), 15);

        let frame2 = allocate_frame();
        assert_eq!(frame2, 0x101000);
        assert_eq!(free_frames_count(), 14);

        assert!(map_page(0x400000, frame1, 3)); // vaddr 0x400000 -> paddr 0x100000 (flags 3 = Present+RW)
        assert_eq!(translate_page(0x400000), 0x100000);
        assert_eq!(translate_page(0x400042), 0x100042); // verificar traducción con offset +0x42

        assert!(deallocate_frame(frame1));
        assert_eq!(free_frames_count(), 15);
        assert!(shutdown());
    }
}
