//! Native ARM64 (AArch64) code emission from Titan MIR.

use crate::{BinOp, MirFunction, MirImmediate, MirInst, MirOperand};

#[derive(Debug, Clone, PartialEq)]
pub struct MachineCode {
    pub bytes: Vec<u8>,
}

pub fn emit_arm64(func: &MirFunction) -> MachineCode {
    let mut bytes = Vec::new();

    for block in &func.blocks {
        for inst in &block.instructions {
            match inst {
                MirInst::Move { dst, src } => {
                    let dst_reg = (*dst as u32) & 0x1F;
                    match src {
                        MirOperand::Imm(MirImmediate::Int(imm)) => {
                            let val = (*imm as u64) & 0xFFFF;
                            let ins: u32 = 0xD2800000 | ((val as u32) << 5) | dst_reg;
                            bytes.extend_from_slice(&ins.to_le_bytes());
                        }
                        MirOperand::Reg(src_reg) => {
                            let s = (*src_reg as u32) & 0x1F;
                            let ins: u32 = 0xAA0003E0 | (s << 16) | dst_reg;
                            bytes.extend_from_slice(&ins.to_le_bytes());
                        }
                        _ => {}
                    }
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
                MirInst::Ret(Some(MirOperand::Reg(r))) => {
                    let r_reg = (*r as u32) & 0x1F;
                    if r_reg != 0 {
                        let ins: u32 = 0xAA0003E0 | (r_reg << 16);
                        bytes.extend_from_slice(&ins.to_le_bytes());
                    }
                    let ret: u32 = 0xD65F03C0;
                    bytes.extend_from_slice(&ret.to_le_bytes());
                }
                MirInst::Ret(_) => {
                    let ret: u32 = 0xD65F03C0;
                    bytes.extend_from_slice(&ret.to_le_bytes());
                }
                _ => {}
            }
        }
    }

    if bytes.is_empty() {
        bytes.extend_from_slice(&0xD65F03C0u32.to_le_bytes());
    }

    MachineCode { bytes }
}
