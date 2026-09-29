use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

pub const PAGE_SIZE: i64 = 4096; // 0x1000 bytes

const MAX_ALLOCATED_FRAMES: usize = 16_384;

/// Error for the operations that need the real hardware. They exist for
/// real in programs built for the bare-metal target (`aarch64-none`), where
/// the Titan runtime writes the CPU page tables, installs exception vectors
/// and touches device registers. A hosted program (and this VM) runs on top
/// of an operating system that owns all of that, so it gets this error
/// instead of a simulated answer.
pub const BARE_METAL_ONLY: &str = "requires a bare-metal program (build with target aarch64-none): a hosted program cannot use page tables, exception vectors or hardware registers";

// Physical frame allocator: pure bookkeeping of page-sized address ranges,
// the same in hosted and bare-metal programs.
struct MemoryState {
    initialized: bool,
    base_paddr: i64,
    total_frames: i64,
    allocated_frames: HashSet<i64>,
    recycled_frames: Vec<i64>,
    next_frame: i64,
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
    Arc::clone(
        states
            .entry(runtime_id)
            .or_insert_with(|| Arc::new(Mutex::new(MemoryState::new()))),
    )
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(
        crate::native::lock_recover(memory_states())
            .remove(&runtime_id)
            .is_some(),
    )
}

/// Frames start at `base_paddr` rounded up to a page and every frame
/// address must be a nonnegative Int.
pub fn init_frame_allocator(base_paddr: i64, total_size_bytes: i64) -> Result<bool, String> {
    if base_paddr < 0 {
        return Err(format!(
            "base address must be nonnegative, got {}",
            base_paddr
        ));
    }
    if total_size_bytes < 0 {
        return Err(format!("size must be nonnegative, got {}", total_size_bytes));
    }
    let Some(aligned_base) = base_paddr
        .checked_add(PAGE_SIZE - 1)
        .map(|address| address & !(PAGE_SIZE - 1))
    else {
        return Ok(false);
    };
    let total_frames = total_size_bytes / PAGE_SIZE;
    if total_frames == 0 {
        return Ok(false);
    }
    let Some(last_offset) = (total_frames - 1).checked_mul(PAGE_SIZE) else {
        return Ok(false);
    };
    if aligned_base.checked_add(last_offset).is_none() {
        return Ok(false);
    }
    let state = get_memory_state();
    let mut state = crate::native::lock_recover(&state);
    state.base_paddr = aligned_base;
    state.total_frames = total_frames;
    state.allocated_frames.clear();
    state.recycled_frames.clear();
    state.next_frame = 0;
    state.initialized = true;
    Ok(true)
}

pub fn allocate_frame() -> i64 {
    let state = get_memory_state();
    let mut state = crate::native::lock_recover(&state);
    if !state.initialized || state.allocated_frames.len() >= MAX_ALLOCATED_FRAMES {
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
    frame
}

pub fn deallocate_frame(paddr: i64) -> bool {
    let state = get_memory_state();
    let mut state = crate::native::lock_recover(&state);
    if !state.initialized || paddr % PAGE_SIZE != 0 {
        return false;
    }
    if state.allocated_frames.remove(&paddr) {
        state.recycled_frames.push(paddr);
        return true;
    }
    false
}

/// Writes the CPU page tables: only in bare-metal programs.
pub fn map_page(_vaddr: i64, _paddr: i64, _flags: i64) -> Result<bool, String> {
    Err(BARE_METAL_ONLY.into())
}

/// Walks the CPU page tables: only in bare-metal programs.
pub fn translate_page(_vaddr: i64) -> Result<i64, String> {
    Err(BARE_METAL_ONLY.into())
}

pub fn free_frames_count() -> i64 {
    let state = get_memory_state();
    let state = crate::native::lock_recover(&state);
    if state.initialized {
        return (state.total_frames - state.allocated_frames.len() as i64).max(0);
    }
    0
}

pub fn shutdown() -> bool {
    let state = get_memory_state();
    let mut state = crate::native::lock_recover(&state);
    state.recycled_frames.clear();
    state.allocated_frames.clear();
    state.next_frame = 0;
    state.initialized = false;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_allocator_and_hosted_paging_error() {
        let runtime_id = 84_000;
        crate::native::with_runtime_context(runtime_id, || {
            assert!(init_frame_allocator(0x100000, 0x10000).unwrap()); // 16 frames de 4 KiB
            assert_eq!(free_frames_count(), 16);
            let frame1 = allocate_frame();
            assert_eq!(frame1, 0x100000);
            assert_eq!(allocate_frame(), 0x101000);
            assert_eq!(free_frames_count(), 14);
            // Las tablas de páginas solo existen de verdad sin sistema operativo.
            assert_eq!(map_page(0x400000, frame1, 3), Err(BARE_METAL_ONLY.into()));
            assert_eq!(translate_page(0x400000), Err(BARE_METAL_ONLY.into()));
            assert!(deallocate_frame(frame1));
            assert!(!deallocate_frame(frame1));
            assert!(!deallocate_frame(frame1 + 1));
            assert_eq!(free_frames_count(), 15);
            assert_eq!(allocate_frame(), frame1);
            assert!(shutdown());
            assert_eq!(free_frames_count(), 0);
            assert_eq!(allocate_frame(), 0);
        });
        assert_eq!(cleanup_runtime(runtime_id), 1);
    }

    #[test]
    fn frame_allocator_limits_and_invalid_arguments() {
        let runtime_id = 84_001;
        crate::native::with_runtime_context(runtime_id, || {
            let modeled_size = 1_i64 << 40;
            assert!(init_frame_allocator(0x1000, modeled_size).unwrap());
            assert_eq!(free_frames_count(), modeled_size / PAGE_SIZE);
            assert_eq!(allocate_frame(), 0x1000);
            assert!(!init_frame_allocator(i64::MAX - 100, PAGE_SIZE).unwrap());
            assert!(!init_frame_allocator(0, PAGE_SIZE - 1).unwrap());
            assert!(!init_frame_allocator(i64::MAX - 2 * PAGE_SIZE, 4 * PAGE_SIZE).unwrap());
            assert!(init_frame_allocator(-1, PAGE_SIZE).is_err());
            assert!(init_frame_allocator(0, -1).is_err());

            let frames = (MAX_ALLOCATED_FRAMES + 1) as i64;
            assert!(init_frame_allocator(0x1000, frames * PAGE_SIZE).unwrap());
            let first = allocate_frame();
            for _ in 1..MAX_ALLOCATED_FRAMES {
                assert_ne!(allocate_frame(), 0);
            }
            assert_eq!(allocate_frame(), 0);
            assert!(deallocate_frame(first));
            assert_eq!(allocate_frame(), first);
        });
        assert_eq!(cleanup_runtime(runtime_id), 1);
    }
}
