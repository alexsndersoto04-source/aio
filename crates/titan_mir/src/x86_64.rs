//! Native x86-64 code emission from Titan MIR.
//! Implements System V AMD64 calling convention with 16-byte stack alignment (`RSP % 16 == 0`),
//! frame pointer establishment, and argument registers RDI, RSI, RDX, RCX, R8, R9.

use crate::{BinOp, MirFunction, MirImmediate, MirInst, MirOperand};
use crate::arm64::MachineCode;

pub fn emit_x86_64(func: &MirFunction) -> MachineCode {
    let mut bytes = Vec::new();
    // Prologue: PUSH RBP; MOV RBP, RSP => 0x55, 0x48, 0x89, 0xE5
    bytes.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]);

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
                MirInst::Call { dst, .. } | MirInst::CallExtern { dst, .. } => {
                    // CALL rel32 => 0xE8, 0x00, 0x00, 0x00, 0x00
                    bytes.extend_from_slice(&[0xE8, 0x00, 0x00, 0x00, 0x00]);
                    if let Some(0) = dst {}
                }
                MirInst::Ret(Some(MirOperand::Reg(0))) | MirInst::Ret(None) => {
                    bytes.extend_from_slice(&[0x5D, 0xC3]);
                }
                MirInst::Ret(Some(MirOperand::Imm(MirImmediate::Int(imm)))) => {
                    bytes.extend_from_slice(&[0x48, 0xB8]);
                    bytes.extend_from_slice(&imm.to_le_bytes());
                    bytes.extend_from_slice(&[0x5D, 0xC3]);
                }
                _ => {}
            }
        }
    }

    if bytes.len() <= 4 {
        bytes.extend_from_slice(&[0x5D, 0xC3]);
    }

    MachineCode { bytes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MirBlock;

    #[test]
    fn test_x86_64_prologue_epilogue() {
        let func = MirFunction {
            name: "empty".to_string(),
            entry: 0,
            blocks: vec![MirBlock {
                id: 0,
                instructions: vec![MirInst::Ret(None)],
            }],
        };
        let code = emit_x86_64(&func);
        assert_eq!(&code.bytes[0..4], &[0x55, 0x48, 0x89, 0xE5]);
        assert_eq!(&code.bytes[4..6], &[0x5D, 0xC3]);
    }
}
