use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub const VECTOR_TABLE_ALIGNMENT: u64 = 1024; // VBAR_EL1 en AArch64 requiere alineación de 1KB (0x400)

pub const VECTOR_SYNC_EXCEPTION: u32 = 0;
pub const VECTOR_IRQ: u32 = 1;
pub const VECTOR_FIQ: u32 = 2;
pub const VECTOR_SERROR: u32 = 3;

struct CpuState {
    initialized: bool,
    vbar_base: u64,
    exception_handlers: HashMap<u32, u64>, // vector_id -> handler_vaddr
    syscall_handlers: HashMap<u32, u64>,   // syscall_num -> handler_vaddr
    last_fault_addr: u64,
    last_error_code: u64,
}

impl CpuState {
    fn new() -> Self {
        Self {
            initialized: false,
            vbar_base: 0,
            exception_handlers: HashMap::new(),
            syscall_handlers: HashMap::new(),
            last_fault_addr: 0,
            last_error_code: 0,
        }
    }
}

fn cpu_states() -> &'static Mutex<HashMap<u64, Arc<Mutex<CpuState>>>> {
    static STATES: OnceLock<Mutex<HashMap<u64, Arc<Mutex<CpuState>>>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_cpu_state() -> Arc<Mutex<CpuState>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut states = crate::native::lock_recover(cpu_states());
    Arc::clone(
        states
            .entry(runtime_id)
            .or_insert_with(|| Arc::new(Mutex::new(CpuState::new()))),
    )
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(
        crate::native::lock_recover(cpu_states())
            .remove(&runtime_id)
            .is_some(),
    )
}

pub fn init_exception_table(base_vbar: u64) -> bool {
    if let Ok(mut state) = get_cpu_state().lock() {
        if !base_vbar.is_multiple_of(VECTOR_TABLE_ALIGNMENT) {
            return false;
        }
        state.vbar_base = base_vbar;
        state.exception_handlers.clear();
        state.syscall_handlers.clear();
        state.last_fault_addr = 0;
        state.last_error_code = 0;
        state.initialized = true;
        true
    } else {
        false
    }
}

pub fn register_exception_handler(vector_id: u32, handler_vaddr: u64) -> bool {
    if let Ok(mut state) = get_cpu_state().lock() {
        if !state.initialized || handler_vaddr == 0 || vector_id > VECTOR_SERROR {
            return false;
        }
        state.exception_handlers.insert(vector_id, handler_vaddr);
        true
    } else {
        false
    }
}

pub fn dispatch_exception(vector_id: u32, fault_addr: u64, error_code: u64) -> u64 {
    if let Ok(mut state) = get_cpu_state().lock() {
        if !state.initialized {
            return 0;
        }
        state.last_fault_addr = fault_addr;
        state.last_error_code = error_code;
        if let Some(&handler) = state.exception_handlers.get(&vector_id) {
            // Devuelve la dirección del manejador o el código de confirmación del vector procesado
            return handler ^ fault_addr ^ error_code;
        }
    }
    0
}

pub fn register_syscall_handler(syscall_num: u32, handler_vaddr: u64) -> bool {
    if let Ok(mut state) = get_cpu_state().lock() {
        if !state.initialized || handler_vaddr == 0 {
            return false;
        }
        state.syscall_handlers.insert(syscall_num, handler_vaddr);
        true
    } else {
        false
    }
}

pub fn invoke_syscall(syscall_num: u32, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    if let Ok(state) = get_cpu_state().lock() {
        if !state.initialized {
            return 0;
        }
        if let Some(&handler) = state.syscall_handlers.get(&syscall_num) {
            // Simular respuesta determinista del manejador kernel procesando los argumentos del trap
            return handler
                .saturating_add(arg0)
                .saturating_add(arg1)
                .saturating_add(arg2);
        }
    }
    0
}

pub fn get_last_fault_addr() -> u64 {
    if let Ok(state) = get_cpu_state().lock() {
        if state.initialized {
            return state.last_fault_addr;
        }
    }
    0
}

pub fn shutdown() -> bool {
    if let Ok(mut state) = get_cpu_state().lock() {
        state.exception_handlers.clear();
        state.syscall_handlers.clear();
        state.vbar_base = 0;
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
    fn test_bare_metal_exception_table_and_syscalls() {
        assert!(!init_exception_table(0x1005)); // Debe fallar si no está alineado a 1KB
        assert!(init_exception_table(0x8000_0000)); // 2GB base alineado exactamente a 1KB

        assert!(register_exception_handler(
            VECTOR_SYNC_EXCEPTION,
            0xFFFF_0000_8000_1000
        ));
        assert_eq!(
            dispatch_exception(VECTOR_SYNC_EXCEPTION, 0x4000_1234, 0x05),
            0xFFFF_0000_8000_1000 ^ 0x4000_1234 ^ 0x05
        );
        assert_eq!(get_last_fault_addr(), 0x4000_1234);

        assert!(register_syscall_handler(1, 0x9000_0000));
        assert_eq!(invoke_syscall(1, 10, 20, 30), 0x9000_0000 + 60);
        assert_eq!(invoke_syscall(999, 10, 20, 30), 0); // Syscall no registrada retorna 0

        assert!(shutdown());
    }
}
