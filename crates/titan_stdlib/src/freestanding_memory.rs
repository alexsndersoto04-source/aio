use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

pub const PAGE_SIZE: u64 = 4096; // 0x1000 bytes

struct MemoryState {
    initialized: bool,
    base_paddr: u64,
    total_frames: u64,
    allocated_frames: HashSet<u64>,
    free_frames: Vec<u64>,
    page_table: HashMap<u64, (u64, u32)>, // vaddr -> (paddr, flags)
}

impl MemoryState {
    fn new() -> Self {
        Self {
            initialized: false,
            base_paddr: 0,
            total_frames: 0,
            allocated_frames: HashSet::new(),
            free_frames: Vec::new(),
            page_table: HashMap::new(),
        }
    }
}

static MEMORY_STATE: OnceLock<Mutex<MemoryState>> = OnceLock::new();

fn get_memory_state() -> &'static Mutex<MemoryState> {
    MEMORY_STATE.get_or_init(|| Mutex::new(MemoryState::new()))
}

pub fn init_frame_allocator(base_paddr: u64, total_size_bytes: u64) -> bool {
    if let Ok(mut state) = get_memory_state().lock() {
        let aligned_base = (base_paddr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let total_frames = total_size_bytes / PAGE_SIZE;
        if total_frames == 0 {
            return false;
        }
        state.base_paddr = aligned_base;
        state.total_frames = total_frames;
        state.allocated_frames.clear();
        state.free_frames.clear();
        state.page_table.clear();

        // Inicializar pool LIFO de frames libres (en orden reverso para asignar desde el inicio)
        for i in (0..total_frames).rev() {
            state
                .free_frames
                .push(aligned_base.saturating_add(i.saturating_mul(PAGE_SIZE)));
        }
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
        if let Some(frame) = state.free_frames.pop() {
            state.allocated_frames.insert(frame);
            return frame;
        }
    }
    0
}

pub fn deallocate_frame(paddr: u64) -> bool {
    if let Ok(mut state) = get_memory_state().lock() {
        if !state.initialized || !paddr.is_multiple_of(PAGE_SIZE) {
            return false;
        }
        if state.allocated_frames.remove(&paddr) {
            state.free_frames.push(paddr);
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
            return state.free_frames.len() as u64;
        }
    }
    0
}

pub fn shutdown() -> bool {
    if let Ok(mut state) = get_memory_state().lock() {
        state.free_frames.clear();
        state.allocated_frames.clear();
        state.page_table.clear();
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
