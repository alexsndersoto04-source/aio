//! Titan AST - Expressions, Patterns, Blocks, Statements.
//! All recursive types use Box<> to prevent infinite size errors (E0072).

use titan_lexer::Span;
use crate::{Item, TypeExpr, Param};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int { value: i64, span: Span },
    Float { value: f64, span: Span },
    String { value: String, span: Span },
    Char { value: char, span: Span },
    Bool { value: bool, span: Span },
    Nil { span: Span },
    Ident { name: String, span: Span },
    Array { elements: Vec<Expr>, span: Span },
    Tuple { elements: Vec<Expr>, span: Span },
    StructLit { name: String, fields: Vec<(String, Expr)>, span: Span },
    Binary { left: Box<Expr>, op: BinaryOp, right: Box<Expr>, span: Span },
    Unary { op: UnaryOp, expr: Box<Expr>, span: Span },
    Call { callee: Box<Expr>, args: Vec<Expr>, span: Span },
    MethodCall { receiver: Box<Expr>, method: String, args: Vec<Expr>, span: Span },
    Index { target: Box<Expr>, index: Box<Expr>, span: Span },
    FieldAccess { target: Box<Expr>, field: String, span: Span },
    If { condition: Box<Expr>, then_branch: Block, else_branch: Option<Block>, span: Span },
    Match { scrutinee: Box<Expr>, arms: Vec<MatchArm>, span: Span },
    For { pattern: Box<Pattern>, iterator: Box<Expr>, body: Block, span: Span },
    While { condition: Box<Expr>, body: Block, span: Span },
    Loop { body: Block, span: Span },
    Break { value: Option<Box<Expr>>, span: Span },
    Continue { span: Span },
    Return { value: Option<Box<Expr>>, span: Span },
    Let { name: String, type_ann: Option<TypeExpr>, value: Box<Expr>, span: Span },
    Assign { target: Box<Expr>, op: Option<BinaryOp>, value: Box<Expr>, span: Span },
    Block(Box<Block>),
    Spawn { expr: Box<Expr>, span: Span },
    Try { expr: Box<Expr>, span: Span },
    Closure { params: Vec<Param>, return_type: Option<TypeExpr>, body: Box<Expr>, span: Span },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod,
    And, Or, Xor,
    Eq, Neq, Lt, Gt, Lte, Gte,
    LazyAnd, LazyOr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp { Neg, Not, BitNot, Ref, RefMut, Deref }

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub final_expr: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Expr(Expr),
    Let { name: String, type_ann: Option<TypeExpr>, value: Expr, span: Span },
    Assign { target: Expr, op: Option<BinaryOp>, value: Expr, span: Span },
    Item(Item),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Box<Expr>>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard { span: Span },
    Ident { name: String, span: Span },
    Literal { value: Box<Expr>, span: Span },
    Struct { name: String, fields: Vec<(String, Pattern)>, rest: bool, span: Span },
    Enum { name: String, variant: String, inner: Option<Box<Pattern>>, span: Span },
    Tuple { elements: Vec<Pattern>, span: Span },
    Or { left: Box<Pattern>, right: Box<Pattern>, span: Span },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Int { span, .. } | Expr::Float { span, .. }
            | Expr::String { span, .. } | Expr::Char { span, .. }
            | Expr::Bool { span, .. } | Expr::Nil { span }
            | Expr::Ident { span, .. } | Expr::Array { span, .. }
            | Expr::Tuple { span, .. } | Expr::StructLit { span, .. }
            | Expr::Binary { span, .. } | Expr::Unary { span, .. }
            | Expr::Call { span, .. } | Expr::MethodCall { span, .. }
            | Expr::Index { span, .. } | Expr::FieldAccess { span, .. }
            | Expr::If { span, .. } | Expr::Match { span, .. }
            | Expr::For { span, .. } | Expr::While { span, .. }
            | Expr::Loop { span, .. } | Expr::Break { span, .. }
            | Expr::Continue { span } | Expr::Return { span, .. }
            | Expr::Spawn { span, .. } | Expr::Try { span, .. }
            | Expr::Closure { span, .. }
            | Expr::Let { span, .. } | Expr::Assign { span, .. } => *span,
            Expr::Block(b) => b.span,
        }
    }
}
