//! Optimization passes over Titan MIR.

use std::collections::HashSet;
use crate::{BinOp, MirFunction, MirImmediate, MirInst, MirModule, MirOperand};

/// Pasa de optimización: Plegado de constantes (Constant Folding).
pub fn fold_constants(function: &mut MirFunction) -> bool {
    let mut changed = false;
    for block in &mut function.blocks {
        for inst in &mut block.instructions {
            if let MirInst::BinOp { dst, op, lhs, rhs } = inst {
                if let (MirOperand::Imm(MirImmediate::Int(a_ref)), MirOperand::Imm(MirImmediate::Int(b_ref))) = (lhs, rhs) {
                    let (a, b) = (*a_ref, *b_ref);
                    let folded = match op {
                        BinOp::Add => Some(MirImmediate::Int(a.saturating_add(b))),
                        BinOp::Sub => Some(MirImmediate::Int(a.saturating_sub(b))),
                        BinOp::Mul => Some(MirImmediate::Int(a.saturating_mul(b))),
                        BinOp::Div => {
                            if b == 0 || (a == i64::MIN && b == -1) {
                                None
                            } else {
                                Some(MirImmediate::Int(a / b))
                            }
                        }
                        BinOp::Eq => Some(MirImmediate::Bool(a == b)),
                        BinOp::Neq => Some(MirImmediate::Bool(a != b)),
                        BinOp::Lt => Some(MirImmediate::Bool(a < b)),
                        BinOp::Gt => Some(MirImmediate::Bool(a > b)),
                    };
                    if let Some(imm) = folded {
                        *inst = MirInst::Move { dst: *dst, src: MirOperand::Imm(imm) };
                        changed = true;
                    }
                }
            }
        }
    }
    changed
}

/// Pasa de optimización: Eliminación de código muerto (Dead Code Elimination - DCE).
pub fn eliminate_dead_code(function: &mut MirFunction) -> bool {
    let mut used_regs = HashSet::new();
    for block in &function.blocks {
        for inst in &block.instructions {
            match inst {
                MirInst::Move { src: MirOperand::Reg(r), .. } => {
                    used_regs.insert(*r);
                }
                MirInst::BinOp { lhs, rhs, .. } => {
                    if let MirOperand::Reg(r) = lhs {
                        used_regs.insert(*r);
                    }
                    if let MirOperand::Reg(r) = rhs {
                        used_regs.insert(*r);
                    }
                }
                MirInst::Call { args, .. } | MirInst::CallExtern { args, .. } => {
                    for arg in args {
                        if let MirOperand::Reg(r) = arg {
                            used_regs.insert(*r);
                        }
                    }
                }
                MirInst::Ret(Some(MirOperand::Reg(r))) => {
                    used_regs.insert(*r);
                }
                _ => {}
            }
        }
    }

    let mut changed = false;
    for block in &mut function.blocks {
        let orig_len = block.instructions.len();
        block.instructions.retain(|inst| match inst {
            MirInst::Move { dst, .. } | MirInst::BinOp { dst, .. } => used_regs.contains(dst),
            _ => true,
        });
        if block.instructions.len() != orig_len {
            changed = true;
        }
    }
    changed
}

/// Pasa de optimización: Simplificación del grafo de control de flujo (CFG Simplification).
pub fn simplify_cfg(function: &mut MirFunction) -> bool {
    let mut reachable = HashSet::new();
    reachable.insert(function.entry);
    for block in &function.blocks {
        for inst in &block.instructions {
            if let MirInst::Jump(target) = inst {
                reachable.insert(*target);
            }
        }
    }

    let orig_len = function.blocks.len();
    function.blocks.retain(|block| reachable.contains(&block.id));
    function.blocks.len() != orig_len
}

/// Aplica todas las pasadas de optimización en una función MIR en un bucle hasta punto fijo.
pub fn optimize_function(function: &mut MirFunction) {
    loop {
        let mut progress = false;
        if fold_constants(function) {
            progress = true;
        }
        if eliminate_dead_code(function) {
            progress = true;
        }
        if simplify_cfg(function) {
            progress = true;
        }
        if !progress {
            break;
        }
    }
}

/// Aplica todas las optimizaciones a un módulo completo MIR.
pub fn optimize_module(module: &mut MirModule) {
    for function in &mut module.functions {
        optimize_function(function);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MirBlock;

    #[test]
    fn test_constant_folding() {
        let mut func = MirFunction {
            name: "test_fold".to_string(),
            entry: 0,
            blocks: vec![MirBlock {
                id: 0,
                instructions: vec![
                    MirInst::BinOp {
                        dst: 1,
                        op: BinOp::Add,
                        lhs: MirOperand::Imm(MirImmediate::Int(20)),
                        rhs: MirOperand::Imm(MirImmediate::Int(22)),
                    },
                    MirInst::Ret(Some(MirOperand::Reg(1))),
                ],
            }],
        };

        let changed = fold_constants(&mut func);
        assert!(changed);
        assert_eq!(
            func.blocks[0].instructions[0],
            MirInst::Move {
                dst: 1,
                src: MirOperand::Imm(MirImmediate::Int(42)),
            }
        );
    }

    #[test]
    fn test_dead_code_elimination() {
        let mut func = MirFunction {
            name: "test_dce".to_string(),
            entry: 0,
            blocks: vec![MirBlock {
                id: 0,
                instructions: vec![
                    MirInst::Move {
                        dst: 10,
                        src: MirOperand::Imm(MirImmediate::Int(999)),
                    },
                    MirInst::Move {
                        dst: 1,
                        src: MirOperand::Imm(MirImmediate::Int(42)),
                    },
                    MirInst::Ret(Some(MirOperand::Reg(1))),
                ],
            }],
        };

        let changed = eliminate_dead_code(&mut func);
        assert!(changed);
        assert_eq!(func.blocks[0].instructions.len(), 2);
        assert!(matches!(func.blocks[0].instructions[0], MirInst::Move { dst: 1, .. }));
    }

    #[test]
    fn test_dce_preserves_calls() {
        let mut func = MirFunction {
            name: "test_call_dce".to_string(),
            entry: 0,
            blocks: vec![MirBlock {
                id: 0,
                instructions: vec![
                    MirInst::CallExtern {
                        dst: Some(10),
                        target: "puts".to_string(),
                        abi: "C".to_string(),
                        args: vec![MirOperand::Imm(MirImmediate::Int(0))],
                    },
                    MirInst::Ret(None),
                ],
            }],
        };
        let changed = eliminate_dead_code(&mut func);
        assert!(!changed);
        assert_eq!(func.blocks[0].instructions.len(), 2);
    }

    #[test]
    fn test_simplify_cfg_unreachable_block() {
        let mut func = MirFunction {
            name: "test_cfg".to_string(),
            entry: 0,
            blocks: vec![
                MirBlock {
                    id: 0,
                    instructions: vec![MirInst::Ret(Some(MirOperand::Imm(MirImmediate::Int(0))))],
                },
                MirBlock {
                    id: 99,
                    instructions: vec![MirInst::Comment("Orphan block".to_string())],
                },
            ],
        };

        let changed = simplify_cfg(&mut func);
        assert!(changed);
        assert_eq!(func.blocks.len(), 1);
        assert_eq!(func.blocks[0].id, 0);
    }

    #[test]
    fn test_optimize_function_full_pipeline() {
        let mut func = MirFunction {
            name: "test_full".to_string(),
            entry: 0,
            blocks: vec![
                MirBlock {
                    id: 0,
                    instructions: vec![
                        MirInst::BinOp {
                            dst: 2,
                            op: BinOp::Mul,
                            lhs: MirOperand::Imm(MirImmediate::Int(10)),
                            rhs: MirOperand::Imm(MirImmediate::Int(10)),
                        },
                        MirInst::BinOp {
                            dst: 1,
                            op: BinOp::Add,
                            lhs: MirOperand::Imm(MirImmediate::Int(30)),
                            rhs: MirOperand::Imm(MirImmediate::Int(12)),
                        },
                        MirInst::Ret(Some(MirOperand::Reg(1))),
                    ],
                },
                MirBlock {
                    id: 5,
                    instructions: vec![MirInst::Comment("dead block".to_string())],
                },
            ],
        };

        optimize_function(&mut func);

        assert_eq!(func.blocks.len(), 1);
        assert_eq!(func.blocks[0].instructions.len(), 2);
        assert_eq!(
            func.blocks[0].instructions[0],
            MirInst::Move {
                dst: 1,
                src: MirOperand::Imm(MirImmediate::Int(42)),
            }
        );
    }

    #[test]
    fn test_optimize_module() {
        let mut module = MirModule {
            functions: vec![MirFunction {
                name: "main".to_string(),
                entry: 0,
                blocks: vec![MirBlock {
                    id: 0,
                    instructions: vec![
                        MirInst::BinOp {
                            dst: 1,
                            op: BinOp::Add,
                            lhs: MirOperand::Imm(MirImmediate::Int(5)),
                            rhs: MirOperand::Imm(MirImmediate::Int(5)),
                        },
                        MirInst::Ret(Some(MirOperand::Reg(1))),
                    ],
                }],
            }],
        };
        optimize_module(&mut module);
        assert_eq!(
            module.functions[0].blocks[0].instructions[0],
            MirInst::Move {
                dst: 1,
                src: MirOperand::Imm(MirImmediate::Int(10)),
            }
        );
    }
}
