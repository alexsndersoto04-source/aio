//! Native ARM64 (AArch64) code emission from Titan MIR.
//! Implements AAPCS64 calling convention, 16-byte stack alignment (`SP % 16 == 0`),
//! frame pointer establishment, and argument registers X0..X7.

use crate::{BinOp, MirFunction, MirImmediate, MirInst, MirOperand};

#[derive(Debug, Clone, PartialEq)]
pub struct MachineCode {
    pub bytes: Vec<u8>,
}

/// Emits standard AAPCS64 function prologue:
/// STP X29, X30, [SP, #-16]!  (Save FP and LR, decrement SP by 16 preserving 16-byte alignment)
/// MOV X29, SP                (Establish frame pointer)
fn emit_prologue(bytes: &mut Vec<u8>) {
    // STP X29, X30, [SP, #-16]! => 0xA9BF7BFD
    bytes.extend_from_slice(&0xA9BF7BFDu32.to_le_bytes());
    // MOV X29, SP (ADD X29, SP, #0) => 0x910003FD
    bytes.extend_from_slice(&0x910003FDu32.to_le_bytes());
}

/// Emits standard AAPCS64 function epilogue:
/// LDP X29, X30, [SP], #16    (Restore FP and LR, increment SP by 16)
/// RET                        (Return using X30/LR)
fn emit_epilogue(bytes: &mut Vec<u8>) {
    // LDP X29, X30, [SP], #16 => 0xA8C17BFD
    bytes.extend_from_slice(&0xA8C17BFDu32.to_le_bytes());
    // RET X30 => 0xD65F03C0
    bytes.extend_from_slice(&0xD65F03C0u32.to_le_bytes());
}

/// Loads an operand into an AArch64 register (`Xn`).
fn emit_load_operand(bytes: &mut Vec<u8>, target_reg: u32, operand: &MirOperand) {
    let reg = target_reg & 0x1F;
    match operand {
        MirOperand::Imm(MirImmediate::Int(imm)) => {
            let val = (*imm as u64) & 0xFFFF;
            let ins: u32 = 0xD2800000 | ((val as u32) << 5) | reg;
            bytes.extend_from_slice(&ins.to_le_bytes());
        }
        MirOperand::Reg(src_reg) => {
            let s = (*src_reg as u32) & 0x1F;
            if s != reg {
                // ORR Xd, XZr, Xm (MOV Xd, Xm)
                let ins: u32 = 0xAA0003E0 | (s << 16) | reg;
                bytes.extend_from_slice(&ins.to_le_bytes());
            }
        }
        _ => {}
    }
}

pub fn emit_arm64(func: &MirFunction) -> MachineCode {
    let mut bytes = Vec::new();
    emit_prologue(&mut bytes);

    for block in &func.blocks {
        for inst in &block.instructions {
            match inst {
                MirInst::Move { dst, src } => {
                    emit_load_operand(&mut bytes, *dst as u32, src);
                }
                MirInst::BinOp { dst, op, lhs, rhs } => {
                    let dst_reg = (*dst as u32) & 0x1F;
                    if let (MirOperand::Reg(l), MirOperand::Reg(r)) = (lhs, rhs) {
                        let l_reg = (*l as u32) & 0x1F;
                        let r_reg = (*r as u32) & 0x1F;
                        let ins: u32 = match op {
                            BinOp::Add => 0x8B000000 | (r_reg << 16) | (l_reg << 5) | dst_reg,
                            BinOp::Sub => 0xCB000000 | (r_reg << 16) | (l_reg << 5) | dst_reg,
                            BinOp::Mul => 0x9B007C00 | (r_reg << 16) | (l_reg << 5) | dst_reg,
                            _ => 0xD503201F,
                        };
                        bytes.extend_from_slice(&ins.to_le_bytes());
                    }
                }
                MirInst::Call { dst, args, .. } | MirInst::CallExtern { dst, args, .. } => {
                    // AAPCS64 passes first 8 arguments in X0..X7
                    for (i, arg) in args.iter().enumerate().take(8) {
                        emit_load_operand(&mut bytes, i as u32, arg);
                    }
                    // BL #0 => 0x94000000 (Symbol relocation spot)
                    bytes.extend_from_slice(&0x94000000u32.to_le_bytes());
                    if let Some(d) = dst {
                        let d_reg = (*d as u32) & 0x1F;
                        if d_reg != 0 {
                            // ORR Xd, XZr, X0 (MOV Xd, X0)
                            let ins: u32 = 0xAA0003E0 | d_reg;
                            bytes.extend_from_slice(&ins.to_le_bytes());
                        }
                    }
                }
                MirInst::Ret(Some(MirOperand::Reg(r))) => {
                    let r_reg = (*r as u32) & 0x1F;
                    if r_reg != 0 {
                        // ORR X0, XZr, Xr (MOV X0, Xr)
                        let ins: u32 = 0xAA0003E0 | (r_reg << 16);
                        bytes.extend_from_slice(&ins.to_le_bytes());
                    }
                    emit_epilogue(&mut bytes);
                }
                MirInst::Ret(_) => {
                    emit_epilogue(&mut bytes);
                }
                _ => {}
            }
        }
    }

    if bytes.is_empty() || !bytes.ends_with(&0xD65F03C0u32.to_le_bytes()) {
        emit_epilogue(&mut bytes);
    }

    MachineCode { bytes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MirBlock, MirImmediate, MirOperand};

    #[test]
    fn test_arm64_prologue_epilogue_alignment() {
        let func = MirFunction {
            name: "empty".to_string(),
            entry: 0,
            blocks: vec![MirBlock {
                id: 0,
                instructions: vec![MirInst::Ret(None)],
            }],
        };
        let code = emit_arm64(&func);
        assert_eq!(code.bytes.len(), 16); // 8 bytes prologue + 8 bytes epilogue
        assert_eq!(&code.bytes[0..4], &0xA9BF7BFDu32.to_le_bytes()); // STP X29, X30, [SP, #-16]!
        assert_eq!(&code.bytes[4..8], &0x910003FDu32.to_le_bytes()); // MOV X29, SP
        assert_eq!(&code.bytes[8..12], &0xA8C17BFDu32.to_le_bytes()); // LDP X29, X30, [SP], #16
        assert_eq!(&code.bytes[12..16], &0xD65F03C0u32.to_le_bytes()); // RET
    }

    #[test]
    fn test_arm64_call_extern_registers() {
        let func = MirFunction {
            name: "call_puts".to_string(),
            entry: 0,
            blocks: vec![MirBlock {
                id: 0,
                instructions: vec![
                    MirInst::CallExtern {
                        dst: Some(1),
                        target: "puts".to_string(),
                        abi: "C".to_string(),
                        args: vec![MirOperand::Imm(MirImmediate::Int(42))],
                    },
                    MirInst::Ret(Some(MirOperand::Reg(1))),
                ],
            }],
        };
        let code = emit_arm64(&func);
        assert!(code.bytes.len() >= 24);
    }
}
