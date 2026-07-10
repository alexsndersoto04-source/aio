//! Titan MIR — Mid-level IR with optimizations.

use titan_hir::HirProgram;

pub struct MirModule {
    pub functions: Vec<MirFunction>,
}

pub struct MirFunction {
    pub name: String,
    pub blocks: Vec<MirBlock>,
    pub entry: usize,
}

pub struct MirBlock {
    pub id: usize,
    pub instructions: Vec<MirInst>,
}

pub enum MirInst {
    Move { dst: usize, src: MirOperand },
    BinOp { dst: usize, op: BinOp, lhs: MirOperand, rhs: MirOperand },
    Ret(Option<MirOperand>),
    Jump(usize),
    Comment(String),
}

pub enum MirOperand {
    Reg(usize),
    Imm(MirImmediate),
    Undef,
}

pub enum MirImmediate {
    Int(i64),
    Float(f64),
    Bool(bool),
    Nil,
}

pub enum BinOp {
    Add, Sub, Mul, Div, Eq, Neq, Lt, Gt,
}

pub fn lower_hir_to_mir(_hir: &HirProgram) -> MirModule {
    MirModule { functions: Vec::new() }
}