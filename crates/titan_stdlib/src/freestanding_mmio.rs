use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

struct MmioState {
    initialized: bool,
    regions: Vec<(u64, u64)>, // (base_paddr, size_bytes)
    regs: HashMap<u64, u32>,  // paddr -> valor simulado de registro de hardware
    uart_base: Option<u64>,
    uart_output_buffer: String, // buffer que captura los bytes enviados a la UART serial
}

impl MmioState {
    fn new() -> Self {
        Self {
            initialized: false,
            regions: Vec::new(),
            regs: HashMap::new(),
            uart_base: None,
            uart_output_buffer: String::new(),
        }
    }
}

fn mmio_states() -> &'static Mutex<HashMap<u64, Arc<Mutex<MmioState>>>> {
    static STATES: OnceLock<Mutex<HashMap<u64, Arc<Mutex<MmioState>>>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_mmio_state() -> Arc<Mutex<MmioState>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut states = crate::native::lock_recover(mmio_states());
    Arc::clone(states.entry(runtime_id).or_insert_with(|| Arc::new(Mutex::new(MmioState::new()))))
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(crate::native::lock_recover(mmio_states()).remove(&runtime_id).is_some())
}

pub fn init_mmio_region(base_paddr: u64, size_bytes: u64) -> bool {
    if let Ok(mut state) = get_mmio_state().lock() {
        if size_bytes == 0 {
            return false;
        }
        state.regions.push((base_paddr, size_bytes));
        state.initialized = true;
        true
    } else {
        false
    }
}

fn is_in_region(state: &MmioState, paddr: u64) -> bool {
    for &(base, size) in &state.regions {
        if paddr >= base && paddr < base.saturating_add(size) {
            return true;
        }
    }
    false
}

pub fn read_mmio_u32(paddr: u64) -> u32 {
    if let Ok(state) = get_mmio_state().lock() {
        if !state.initialized || !is_in_region(&state, paddr) {
            return 0;
        }
        if let Some(&val) = state.regs.get(&paddr) {
            return val;
        }
    }
    0
}

pub fn write_mmio_u32(paddr: u64, value: u32) -> bool {
    if let Ok(mut state) = get_mmio_state().lock() {
        if !state.initialized || !is_in_region(&state, paddr) {
            return false;
        }
        state.regs.insert(paddr, value);
        // Si la escritura es en el registro de transmisión de datos del UART, añadir al buffer
        if let Some(uart_base) = state.uart_base {
            if paddr == uart_base {
                if let Some(ch) = char::from_u32(value & 0xFF) {
                    state.uart_output_buffer.push(ch);
                }
            }
        }
        true
    } else {
        false
    }
}

pub fn serial_init(uart_base_paddr: u64, baudrate: u32) -> bool {
    if let Ok(mut state) = get_mmio_state().lock() {
        if baudrate == 0 {
            return false;
        }
        state.regions.push((uart_base_paddr, 0x1000));
        state.uart_base = Some(uart_base_paddr);
        state.uart_output_buffer.clear();
        state.initialized = true;
        true
    } else {
        false
    }
}

pub fn serial_write_str(text: &str) -> usize {
    if let Ok(mut state) = get_mmio_state().lock() {
        if !state.initialized || state.uart_base.is_none() {
            return 0;
        }
        let uart_base = state.uart_base.unwrap();
        let mut count: usize = 0;
        for byte in text.bytes() {
            state.regs.insert(uart_base, byte as u32);
            state.uart_output_buffer.push(byte as char);
            count = count.saturating_add(1);
        }
        count
    } else {
        0
    }
}

pub fn serial_get_buffer() -> String {
    if let Ok(state) = get_mmio_state().lock() {
        if state.initialized {
            return state.uart_output_buffer.clone();
        }
    }
    String::new()
}

pub fn shutdown() -> bool {
    if let Ok(mut state) = get_mmio_state().lock() {
        state.regions.clear();
        state.regs.clear();
        state.uart_base = None;
        state.uart_output_buffer.clear();
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
    fn test_bare_metal_mmio_and_serial_driver() {
        assert!(init_mmio_region(0x3F00_0000, 0x1000)); // Registro MMIO genérico de periféricos
        assert!(write_mmio_u32(0x3F00_0004, 0xDEAD_BEEF));
        assert_eq!(read_mmio_u32(0x3F00_0004), 0xDEAD_BEEF);
        assert!(!write_mmio_u32(0x9999_0000, 0x1234)); // Debe fallar por estar fuera de región MMIO

        assert!(serial_init(0x1000_0000, 115200)); // UART base QEMU ARM64 PL011 a 115200 baudios
        assert_eq!(serial_write_str("Hello Bare-Metal TITAN Kernel!\n"), 31);
        assert_eq!(serial_get_buffer(), "Hello Bare-Metal TITAN Kernel!\n");

        assert!(shutdown());
    }
}
