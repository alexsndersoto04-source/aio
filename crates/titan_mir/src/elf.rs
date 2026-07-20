//! ELF binary emission (Object, Dylib, Executable) for Titan MIR.

use crate::MirFunction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    Arm64,
    X86_64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElfFunction<'a> {
    pub name: &'a String,
    pub code: &'a Vec<u8>,
}

const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

fn machine_flag(arch: Architecture) -> u16 {
    match arch {
        Architecture::Arm64 => 183,  // EM_AARCH64
        Architecture::X86_64 => 62,  // EM_X86_64
    }
}

pub fn emit_elf_object(arch: Architecture, functions: &[ElfFunction<'_>]) -> Vec<u8> {
    let mut elf = Vec::new();
    elf.extend_from_slice(&ELF_MAGIC);
    elf.push(2); // 64-bit ELF
    elf.push(1); // Little-endian
    elf.push(1); // ELF version
    elf.push(0); // System V ABI
    elf.extend_from_slice(&[0; 8]); // Padding

    elf.extend_from_slice(&1u16.to_le_bytes()); // e_type = ET_REL
    elf.extend_from_slice(&machine_flag(arch).to_le_bytes()); // e_machine
    elf.extend_from_slice(&1u32.to_le_bytes()); // e_version
    elf.extend_from_slice(&0u64.to_le_bytes()); // e_entry
    elf.extend_from_slice(&0u64.to_le_bytes()); // e_phoff
    elf.extend_from_slice(&64u64.to_le_bytes()); // e_shoff
    elf.extend_from_slice(&0u32.to_le_bytes()); // e_flags
    elf.extend_from_slice(&64u16.to_le_bytes()); // e_ehsize
    elf.extend_from_slice(&0u16.to_le_bytes()); // e_phentsize
    elf.extend_from_slice(&0u16.to_le_bytes()); // e_phnum
    elf.extend_from_slice(&64u16.to_le_bytes()); // e_shentsize
    elf.extend_from_slice(&1u16.to_le_bytes()); // e_shnum
    elf.extend_from_slice(&0u16.to_le_bytes()); // e_shstrndx

    for f in functions {
        elf.extend_from_slice(f.code.as_slice());
    }

    elf
}

pub fn emit_elf_dylib(arch: Architecture, functions: &[ElfFunction<'_>]) -> Vec<u8> {
    let mut elf = Vec::new();
    elf.extend_from_slice(&ELF_MAGIC);
    elf.push(2); // 64-bit ELF
    elf.push(1); // Little-endian
    elf.push(1); // ELF version
    elf.push(0); // System V ABI
    elf.extend_from_slice(&[0; 8]); // Padding

    elf.extend_from_slice(&3u16.to_le_bytes()); // e_type = ET_DYN
    elf.extend_from_slice(&machine_flag(arch).to_le_bytes()); // e_machine
    elf.extend_from_slice(&1u32.to_le_bytes()); // e_version
    elf.extend_from_slice(&0x1000u64.to_le_bytes()); // e_entry
    elf.extend_from_slice(&64u64.to_le_bytes()); // e_phoff
    elf.extend_from_slice(&0u64.to_le_bytes()); // e_shoff
    elf.extend_from_slice(&0u32.to_le_bytes()); // e_flags
    elf.extend_from_slice(&64u16.to_le_bytes()); // e_ehsize
    elf.extend_from_slice(&56u16.to_le_bytes()); // e_phentsize
    elf.extend_from_slice(&1u16.to_le_bytes()); // e_phnum
    elf.extend_from_slice(&64u16.to_le_bytes()); // e_shentsize
    elf.extend_from_slice(&0u16.to_le_bytes()); // e_shnum
    elf.extend_from_slice(&0u16.to_le_bytes()); // e_shstrndx

    for f in functions {
        elf.extend_from_slice(f.code.as_slice());
    }

    elf
}

pub fn emit_standalone_executable(arch: Architecture, main_func: &MirFunction) -> Vec<u8> {
    let mut elf = Vec::new();
    elf.extend_from_slice(&ELF_MAGIC);
    elf.push(2); // 64-bit ELF
    elf.push(1); // Little-endian
    elf.push(1); // ELF version
    elf.push(0); // System V ABI
    elf.extend_from_slice(&[0; 8]); // Padding

    elf.extend_from_slice(&2u16.to_le_bytes()); // e_type = ET_EXEC
    elf.extend_from_slice(&machine_flag(arch).to_le_bytes()); // e_machine
    elf.extend_from_slice(&1u32.to_le_bytes()); // e_version
    elf.extend_from_slice(&0x400000u64.to_le_bytes()); // e_entry
    elf.extend_from_slice(&64u64.to_le_bytes()); // e_phoff
    elf.extend_from_slice(&0u64.to_le_bytes()); // e_shoff
    elf.extend_from_slice(&0u32.to_le_bytes()); // e_flags
    elf.extend_from_slice(&64u16.to_le_bytes()); // e_ehsize
    elf.extend_from_slice(&56u16.to_le_bytes()); // e_phentsize
    elf.extend_from_slice(&1u16.to_le_bytes()); // e_phnum
    elf.extend_from_slice(&64u16.to_le_bytes()); // e_shentsize
    elf.extend_from_slice(&0u16.to_le_bytes()); // e_shnum
    elf.extend_from_slice(&0u16.to_le_bytes()); // e_shstrndx

    let code = match arch {
        Architecture::Arm64 => crate::arm64::emit_arm64(main_func).bytes,
        Architecture::X86_64 => crate::x86_64::emit_x86_64(main_func).bytes,
    };
    elf.extend_from_slice(&code);

    elf
}
