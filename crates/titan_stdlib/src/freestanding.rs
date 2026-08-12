use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, OnceLock};

struct FreestandingState {
    initialized: bool,
    active_target: String,
}

impl FreestandingState {
    fn new() -> Self {
        Self {
            initialized: false,
            active_target: String::new(),
        }
    }
}

fn freestanding_states() -> &'static Mutex<HashMap<u64, Arc<Mutex<FreestandingState>>>> {
    static STATES: OnceLock<Mutex<HashMap<u64, Arc<Mutex<FreestandingState>>>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_freestanding_state() -> Arc<Mutex<FreestandingState>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut states = crate::native::lock_recover(freestanding_states());
    Arc::clone(states.entry(runtime_id).or_insert_with(|| Arc::new(Mutex::new(FreestandingState::new()))))
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(crate::native::lock_recover(freestanding_states()).remove(&runtime_id).is_some())
}

pub fn validate_target_spec(target: &str) -> bool {
    matches!(
        target.trim(),
        "aarch64-unknown-none" | "x86_64-unknown-none" | "riscv64gc-unknown-none"
    )
}

pub fn init(target_arch: &str) -> bool {
    if !validate_target_spec(target_arch) {
        return false;
    }
    if let Ok(mut state) = get_freestanding_state().lock() {
        state.active_target = target_arch.to_string();
        state.initialized = true;
        true
    } else {
        false
    }
}

pub fn generate_linker_script(target_arch: &str, base_addr: u64, stack_size: u64) -> String {
    let output_arch = match target_arch {
        "aarch64-unknown-none" => "aarch64",
        "x86_64-unknown-none" => "i386:x86-64",
        "riscv64gc-unknown-none" => "riscv:rv64",
        _ => "aarch64",
    };

    let mut ld = String::with_capacity(1024);
    let _ = writeln!(
        ld,
        "/* Titan Freestanding Linker Script for {} */",
        target_arch
    );
    let _ = writeln!(ld, "OUTPUT_FORMAT(\"elf64-{}\")", output_arch);
    let _ = writeln!(ld, "ENTRY(_start)");
    let _ = writeln!(ld, "SECTIONS\n{{");
    let _ = writeln!(ld, "    . = 0x{:x};", base_addr);
    let _ = writeln!(ld, "    __kernel_start = .;");
    let _ = writeln!(
        ld,
        "    .text : ALIGN(0x1000) {{\n        *(.text._start)\n        *(.text*)\n    }}"
    );
    let _ = writeln!(
        ld,
        "    .rodata : ALIGN(0x1000) {{\n        *(.rodata*)\n    }}"
    );
    let _ = writeln!(
        ld,
        "    .data : ALIGN(0x1000) {{\n        *(.data*)\n    }}"
    );
    let _ = writeln!(ld, "    .bss : ALIGN(0x1000) {{\n        __bss_start = .;");
    let _ = writeln!(
        ld,
        "        *(.bss*)\n        *(COMMON)\n        __bss_end = .;"
    );
    let _ = writeln!(ld, "    }}");
    let _ = writeln!(ld, "    . = ALIGN(0x1000);");
    let _ = writeln!(ld, "    _stack_bottom = .;");
    let _ = writeln!(ld, "    . += 0x{:x};", stack_size);
    let _ = writeln!(ld, "    _stack_top = .;");
    let _ = writeln!(ld, "    __kernel_end = .;");
    let _ = writeln!(ld, "}}");
    ld
}

pub fn generate_startup_asm(target_arch: &str, entry_fn: &str) -> String {
    let mut asm = String::with_capacity(512);
    let _ = writeln!(
        asm,
        "/* Titan Bare-Metal Startup (_start) for {} */",
        target_arch
    );
    let _ = writeln!(asm, ".global _start");
    let _ = writeln!(asm, ".section .text._start");
    let _ = writeln!(asm, "_start:");

    match target_arch {
        "aarch64-unknown-none" => {
            let _ = writeln!(asm, "    /* Setup Stack Pointer */");
            let _ = writeln!(asm, "    adrp x0, _stack_top");
            let _ = writeln!(asm, "    add x0, x0, :lo12:_stack_top");
            let _ = writeln!(asm, "    mov sp, x0");
            let _ = writeln!(asm, "    /* Zero-out BSS section */");
            let _ = writeln!(
                asm,
                "    adrp x1, __bss_start\n    add x1, x1, :lo12:__bss_start"
            );
            let _ = writeln!(
                asm,
                "    adrp x2, __bss_end\n    add x2, x2, :lo12:__bss_end"
            );
            let _ = writeln!(
                asm,
                "1:  cmp x1, x2\n    b.ge 2f\n    str xzr, [x1], #8\n    b 1b"
            );
            let _ = writeln!(asm, "2:  bl {}", entry_fn);
            let _ = writeln!(asm, "3:  wfe\n    b 3b");
        }
        "x86_64-unknown-none" => {
            let _ = writeln!(asm, "    /* Setup Stack Pointer */");
            let _ = writeln!(asm, "    lea _stack_top(%rip), %rsp");
            let _ = writeln!(asm, "    /* Zero-out BSS section */");
            let _ = writeln!(asm, "    lea __bss_start(%rip), %rdi");
            let _ = writeln!(asm, "    lea __bss_end(%rip), %rsi");
            let _ = writeln!(asm, "1:  cmp %rsi, %rdi\n    jge 2f\n    movq $0, (%rdi)\n    addq $8, %rdi\n    jmp 1b");
            let _ = writeln!(asm, "2:  call {}\n3:  cli\n    hlt\n    jmp 3b", entry_fn);
        }
        _ => {
            let _ = writeln!(asm, "    /* Unsupported architecture startup fallback */");
            let _ = writeln!(asm, "    b .");
        }
    }
    asm
}

pub fn get_active_target() -> String {
    if let Ok(state) = get_freestanding_state().lock() {
        if state.initialized {
            return state.active_target.clone();
        }
    }
    String::new()
}

pub fn shutdown() -> bool {
    if let Ok(mut state) = get_freestanding_state().lock() {
        state.active_target.clear();
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
    fn test_freestanding_target_and_linker_generation() {
        assert!(validate_target_spec("aarch64-unknown-none"));
        assert!(!validate_target_spec("invalid-target"));
        assert!(init("aarch64-unknown-none"));
        assert_eq!(get_active_target(), "aarch64-unknown-none");

        let ld = generate_linker_script("aarch64-unknown-none", 0x80000, 0x10000);
        assert!(ld.contains("ENTRY(_start)"));
        assert!(ld.contains(". = 0x80000;"));
        assert!(ld.contains("_stack_top = .;"));

        let asm = generate_startup_asm("aarch64-unknown-none", "kernel_main");
        assert!(asm.contains("mov sp, x0"));
        assert!(asm.contains("bl kernel_main"));
        assert!(shutdown());
    }
}
