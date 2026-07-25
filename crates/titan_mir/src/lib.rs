//! Titan MIR — Mid-level IR skeleton.
//!
//! STATUS: EXPERIMENTAL / INCOMPLETE.
//! The public data types (`MirModule`, `MirFunction`, `MirInst`, etc.) and the
//! constant-folding pass in `optimize::` are implemented, but `lower_hir_to_mir`
//! is a stub that returns an empty module. The `arm64` and `x86_64` backends and
//! the `elf` writer emit only a small set of instructions and a partial ELF header;
//! they are NOT suitable for producing loadable object files, dynamic libraries
//! or executables today. Use `titan_codegen` (bytecode) or `titan_wasm` for
//! real end-to-end compilation.

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
    Call { dst: Option<usize>, target: String, args: Vec<MirOperand> },
    CallExtern { dst: Option<usize>, target: String, abi: String, args: Vec<MirOperand> },
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
