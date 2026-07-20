//! Titan MIR — Mid-level IR with optimizations and native emission.

pub mod optimize;
pub mod arm64;
pub mod x86_64;
pub mod elf;

use titan_hir::HirProgram;

#[derive(Debug, Clone, PartialEq)]
pub struct MirModule {
    pub functions: Vec<MirFunction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirFunction {
    pub name: String,
    pub blocks: Vec<MirBlock>,
    pub entry: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirBlock {
    pub id: usize,
    pub instructions: Vec<MirInst>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirInst {
    Move { dst: usize, src: MirOperand },
    BinOp { dst: usize, op: BinOp, lhs: MirOperand, rhs: MirOperand },
    Ret(Option<MirOperand>),
    Jump(usize),
    Comment(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirOperand {
    Reg(usize),
    Imm(MirImmediate),
    Undef,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirImmediate {
    Int(i64),
    Float(f64),
    Bool(bool),
    Nil,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Neq,
    Lt,
    Gt,
}

pub fn lower_hir_to_mir(_hir: &HirProgram) -> MirModule {
    MirModule { functions: Vec::new() }
}
