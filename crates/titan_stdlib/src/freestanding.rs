//! Bare-metal build helpers (`std::freestanding`) — **real text generation**.
//!
//! These functions generate the two build artifacts a Titan bare-metal
//! target needs: a GNU ld **linker script** and an **assembly `_start`**
//! stub. The output is real, valid text that a cross-toolchain (aarch64-/
//! x86_64-/riscv64-unknown-none) would consume to link a kernel.
//!
//! What this module is **not**: it is not a kernel, an emulator or a
//! hardware interface. There is deliberately **no simulation** of CPU
//! exception tables, MMIO or a frame allocator — those were removed in
//! Phase 41 because pretending to touch hardware with in-memory
//! HashMaps is not honest.

use std::fmt::Write as _;

/// Validates a bare-metal target triple against the supported set.
pub fn validate_target_spec(target: &str) -> bool {
    matches!(
        target.trim(),
        "aarch64-unknown-none" | "x86_64-unknown-none" | "riscv64gc-unknown-none"
    )
}

/// Generates a real GNU ld linker script for the target: output format,
/// `_start` entry, and `.text` / `.rodata` / `.data` / `.bss` sections
/// with 4 KiB alignment, a BSS zeroing range and a stack region.
pub fn generate_linker_script(target_arch: &str, base_addr: u64, stack_size: u64) -> String {
    let output_arch = match target_arch {
        "aarch64-unknown-none" => "aarch64",
        "x86_64-unknown-none" => "i386:x86-64",
        "riscv64gc-unknown-none" => "riscv:rv64",
        _ => "aarch64",
    };

    let mut ld = String::with_capacity(1024);
    let _ = writeln!(ld, "/* Titan Freestanding Linker Script for {} */", target_arch);
    let _ = writeln!(ld, "OUTPUT_FORMAT(\"elf64-{}\")", output_arch);
    let _ = writeln!(ld, "ENTRY(_start)");
    let _ = writeln!(ld, "SECTIONS\n{{");
    let _ = writeln!(ld, "    . = 0x{:x};", base_addr);
    let _ = writeln!(ld, "    __kernel_start = .;");
    let _ = writeln!(ld, "    .text : ALIGN(0x1000) {{\n        *(.text._start)\n        *(.text*)\n    }}");
    let _ = writeln!(ld, "    .rodata : ALIGN(0x1000) {{\n        *(.rodata*)\n    }}");
    let _ = writeln!(ld, "    .data : ALIGN(0x1000) {{\n        *(.data*)\n    }}");
    let _ = writeln!(ld, "    .bss : ALIGN(0x1000) {{\n        __bss_start = .;");
    let _ = writeln!(ld, "        *(.bss*)\n        *(COMMON)\n        __bss_end = .;");
    let _ = writeln!(ld, "    }}");
    let _ = writeln!(ld, "    . = ALIGN(0x1000);");
    let _ = writeln!(ld, "    _stack_bottom = .;");
    let _ = writeln!(ld, "    . += 0x{:x};", stack_size);
    let _ = writeln!(ld, "    _stack_top = .;");
    let _ = writeln!(ld, "    __kernel_end = .;");
    let _ = writeln!(ld, "}}");
    ld
}

/// Generates a real assembly `_start` stub: sets up the stack pointer,
/// zeroes `.bss`, calls `entry_fn`, then parks the CPU.
pub fn generate_startup_asm(target_arch: &str, entry_fn: &str) -> String {
    let mut asm = String::with_capacity(512);
    let _ = writeln!(asm, "/* Titan Bare-Metal Startup (_start) for {} */", target_arch);
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
            let _ = writeln!(asm, "    adrp x1, __bss_start\n    add x1, x1, :lo12:__bss_start");
            let _ = writeln!(asm, "    adrp x2, __bss_end\n    add x2, x2, :lo12:__bss_end");
            let _ = writeln!(asm, "1:  cmp x1, x2\n    b.ge 2f\n    str xzr, [x1], #8\n    b 1b");
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
        "riscv64gc-unknown-none" => {
            let _ = writeln!(asm, "    /* Setup Stack Pointer */");
            let _ = writeln!(asm, "    la sp, _stack_top");
            let _ = writeln!(asm, "    /* Zero-out BSS section */");
            let _ = writeln!(asm, "    la t0, __bss_start\n    la t1, __bss_end");
            let _ = writeln!(asm, "1:  bgeu t0, t1, 2f\n    sd zero, 0(t0)\n    addi t0, t0, 8\n    j 1b");
            let _ = writeln!(asm, "2:  call {}", entry_fn);
            let _ = writeln!(asm, "3:  wfi\n    j 3b");
        }
        _ => {
            let _ = writeln!(asm, "    /* Unsupported architecture startup fallback */");
            let _ = writeln!(asm, "    b .");
        }
    }
    asm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_validation_is_real() {
        assert!(validate_target_spec("aarch64-unknown-none"));
        assert!(!validate_target_spec("invalid-target"));
    }

    #[test]
    fn linker_script_generation_is_real_text() {
        let ld = generate_linker_script("aarch64-unknown-none", 0x80000, 0x10000);
        assert!(ld.contains("ENTRY(_start)"));
        assert!(ld.contains(". = 0x80000;"));
        assert!(ld.contains("_stack_top = .;"));
    }

    #[test]
    fn startup_asm_generation_is_real_text() {
        let asm = generate_startup_asm("aarch64-unknown-none", "kernel_main");
        assert!(asm.contains("mov sp, x0"));
        assert!(asm.contains("bl kernel_main"));

        let asm64 = generate_startup_asm("x86_64-unknown-none", "kmain");
        assert!(asm64.contains("lea _stack_top(%rip), %rsp"));
        assert!(asm64.contains("call kmain"));

        let riscv = generate_startup_asm("riscv64gc-unknown-none", "kernel_main");
        assert!(riscv.contains("la sp, _stack_top"));
        assert!(riscv.contains("call kernel_main"));
    }
}
