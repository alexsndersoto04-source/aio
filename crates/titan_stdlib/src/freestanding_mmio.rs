// std::freestanding_mmio: device registers and the serial port. Register
// reads/writes and the UART are real only in programs built for the
// bare-metal target (`aarch64-none`): there the Titan runtime does volatile
// 32-bit accesses and programs a real PL011 UART. A hosted program — and
// this VM — cannot touch hardware registers, so those operations return an
// honest error instead of reading a simulated register map. The list of
// declared regions is plain bookkeeping and works everywhere.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::freestanding_memory::BARE_METAL_ONLY;

const MAX_MMIO_REGIONS: usize = 256;

struct MmioState {
    regions: Vec<(i64, i64)>, // (base_paddr, size_bytes)
}

fn mmio_states() -> &'static Mutex<HashMap<u64, Arc<Mutex<MmioState>>>> {
    static STATES: OnceLock<Mutex<HashMap<u64, Arc<Mutex<MmioState>>>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_mmio_state() -> Arc<Mutex<MmioState>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut states = crate::native::lock_recover(mmio_states());
    Arc::clone(
        states
            .entry(runtime_id)
            .or_insert_with(|| Arc::new(Mutex::new(MmioState { regions: Vec::new() }))),
    )
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(
        crate::native::lock_recover(mmio_states())
            .remove(&runtime_id)
            .is_some(),
    )
}

pub fn init_mmio_region(base_paddr: i64, size_bytes: i64) -> Result<bool, String> {
    if base_paddr < 0 {
        return Err(format!(
            "base address must be nonnegative, got {}",
            base_paddr
        ));
    }
    if size_bytes < 0 {
        return Err(format!("size must be nonnegative, got {}", size_bytes));
    }
    let state = get_mmio_state();
    let mut state = crate::native::lock_recover(&state);
    if size_bytes == 0
        || base_paddr.checked_add(size_bytes).is_none()
        || state.regions.len() >= MAX_MMIO_REGIONS
    {
        return Ok(false);
    }
    state.regions.push((base_paddr, size_bytes));
    Ok(true)
}

pub fn read_mmio_u32(_paddr: i64) -> Result<i64, String> {
    Err(BARE_METAL_ONLY.into())
}

pub fn write_mmio_u32(_paddr: i64, _value: i64) -> Result<bool, String> {
    Err(BARE_METAL_ONLY.into())
}

pub fn serial_init(_uart_base_paddr: i64, _baudrate: i64) -> Result<bool, String> {
    Err(BARE_METAL_ONLY.into())
}

pub fn serial_write_str(_text: &str) -> Result<i64, String> {
    Err(BARE_METAL_ONLY.into())
}

/// Bytes sent through the UART by this program (never any when hosted).
pub fn serial_get_buffer() -> String {
    String::new()
}

pub fn shutdown() -> bool {
    let state = get_mmio_state();
    crate::native::lock_recover(&state).regions.clear();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_are_bookkeeping_and_register_access_is_bare_metal_only() {
        let runtime_id = 85_009;
        crate::native::with_runtime_context(runtime_id, || {
            for region in 0..MAX_MMIO_REGIONS {
                assert!(init_mmio_region(region as i64 * 0x1_0000, 0x1_0000).unwrap());
            }
            assert!(!init_mmio_region(0x1_0000_0000, 0x1000).unwrap());
            assert!(shutdown());
            assert!(init_mmio_region(0x3F00_0000, 0x1000).unwrap());
            assert!(!init_mmio_region(0x1000, 0).unwrap());
            assert!(!init_mmio_region(i64::MAX, 2).unwrap());
            assert!(init_mmio_region(-1, 16).is_err());
            assert!(init_mmio_region(0, -16).is_err());
            assert_eq!(write_mmio_u32(0x3F00_0004, 0xDEAD_BEEF), Err(BARE_METAL_ONLY.into()));
            assert_eq!(read_mmio_u32(0x3F00_0004), Err(BARE_METAL_ONLY.into()));
            assert_eq!(serial_init(0x0900_0000, 115_200), Err(BARE_METAL_ONLY.into()));
            assert_eq!(serial_write_str("hola"), Err(BARE_METAL_ONLY.into()));
            assert_eq!(serial_get_buffer(), "");
        });
        assert_eq!(cleanup_runtime(runtime_id), 1);
    }
}
