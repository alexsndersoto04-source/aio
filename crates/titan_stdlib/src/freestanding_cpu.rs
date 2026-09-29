// std::freestanding_cpu: exception vectors and traps. They are real only in
// programs built for the bare-metal target (`aarch64-none`): there the Titan
// runtime copies a real AArch64 vector table to the requested address, loads
// VBAR_EL1, and real exceptions (and `invoke_syscall`, a real `svc`) call the
// Titan functions registered as handlers. A hosted program — and this VM —
// runs on top of an operating system that owns the exception vectors, so
// those operations return an honest error instead of a simulated result.

use crate::freestanding_memory::BARE_METAL_ONLY;

/// VBAR_EL1 bits [10:0] are RES0: the table must be 2 KiB aligned.
pub const VECTOR_TABLE_ALIGNMENT: i64 = 2048;
pub const VECTOR_SYNC_EXCEPTION: i64 = 0;
pub const VECTOR_IRQ: i64 = 1;
pub const VECTOR_FIQ: i64 = 2;
pub const VECTOR_SERROR: i64 = 3;

pub fn init_exception_table(_base_vbar: i64) -> Result<bool, String> {
    Err(BARE_METAL_ONLY.into())
}

pub fn register_exception_handler(_vector_id: i64) -> Result<bool, String> {
    Err(BARE_METAL_ONLY.into())
}

pub fn dispatch_exception(_vector_id: i64, _fault_addr: i64, _error_code: i64) -> Result<i64, String> {
    Err(BARE_METAL_ONLY.into())
}

pub fn register_syscall_handler(_syscall_num: i64) -> Result<bool, String> {
    Err(BARE_METAL_ONLY.into())
}

pub fn invoke_syscall(_syscall_num: i64, _arg0: i64, _arg1: i64, _arg2: i64) -> Result<i64, String> {
    Err(BARE_METAL_ONLY.into())
}

/// Last fault address recorded by a real exception (always 0 when hosted).
pub fn get_last_fault_addr() -> i64 {
    0
}

pub fn shutdown() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosted_programs_get_an_honest_error_instead_of_simulated_traps() {
        assert_eq!(init_exception_table(0x8000_0000), Err(BARE_METAL_ONLY.into()));
        assert_eq!(register_exception_handler(VECTOR_SYNC_EXCEPTION), Err(BARE_METAL_ONLY.into()));
        assert_eq!(dispatch_exception(VECTOR_SYNC_EXCEPTION, 0x4000_1234, 5), Err(BARE_METAL_ONLY.into()));
        assert_eq!(register_syscall_handler(1), Err(BARE_METAL_ONLY.into()));
        assert_eq!(invoke_syscall(1, 10, 20, 30), Err(BARE_METAL_ONLY.into()));
        assert_eq!(get_last_fault_addr(), 0);
        assert!(shutdown());
        assert_eq!(VECTOR_TABLE_ALIGNMENT, 2048);
        assert!(VECTOR_IRQ < VECTOR_FIQ && VECTOR_FIQ < VECTOR_SERROR);
    }
}
