//! Titan AST - Abstract Syntax Tree types.
//! Uses titan_lexer::Span for source locations.
//! Box<> used for recursive types to prevent E0072 errors.

mod expr;
pub use expr::*;

use titan_lexer::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Function(FunctionDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplBlock),
    Module(ModuleDecl),
    Import(ImportDecl),
    Const(ConstDecl),
    /// Phase 28: `type Name = ExistingType` — pure type-level rename.
    /// No runtime cost; the typechecker resolves aliases before doing
    /// compatibility checks. Cannot be recursive (T = T is rejected).
    TypeAlias(TypeAliasDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAliasDecl {
    pub name: String,
    pub target: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    /// Canonical source path assigned by the project loader.
    pub source_file: Option<String>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Option<Block>,
    pub is_extern: bool,
    pub abi: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub mutable: bool,
    pub type_ann: Option<TypeExpr>,
    pub default: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<StructField>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: String,
    pub type_ann: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraitDecl {
    pub name: String,
    pub methods: Vec<TraitMethod>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    /// Phase 22: optional default body. When present, structs that
    /// `impl Trait for Type` without providing this method get the
    /// default automatically. When None, the impl must provide it.
    pub body: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplBlock {
    pub trait_name: Option<String>,
    pub target_type: TypeExpr,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    pub name: String,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub path: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
    pub value: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named {
        name: String,
        generics: Vec<TypeExpr>,
    },
    Reference {
        inner: Box<TypeExpr>,
        is_mut: bool,
    },
    Slice {
        inner: Box<TypeExpr>,
    },
    Array {
        inner: Box<TypeExpr>,
        size: Box<Expr>,
    },
    Tuple {
        elements: Vec<TypeExpr>,
    },
    Function {
        params: Vec<TypeExpr>,
        return_type: Box<TypeExpr>,
    },
    Unit,
    Never,
    Infer(usize),
}
