//! Semantic analysis and type checking for Titan.

use std::collections::{HashMap, HashSet};
use thiserror::Error;
use titan_ast::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int, Float, Bool, String, Char, Nil, Unit, Never,
    Array(Box<Type>), Tuple(Vec<Type>), Named(String), Function(Vec<Type>, Box<Type>),
    Unknown,
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum TypeError {
    #[error("type mismatch: expected {expected}, found {found}")]
    Mismatch { expected: Type, found: Type },
    #[error("unknown variable or function '{name}'")]
    UnknownVariable { name: String },
    #[error("'{name}' is not callable")]
    NotCallable { name: String },
    #[error("function expected {expected} arguments, found {found}")]
    Arity { expected: usize, found: usize },
    #[error("invalid operands for {operator}: {left} and {right}")]
    InvalidOperands { operator: String, left: Type, right: Type },
    #[error("missing field '{field}' in struct '{structure}'")]
    MissingField { structure: String, field: String },
    #[error("unknown field '{field}' in struct '{structure}'")]
    UnknownField { structure: String, field: String },
    #[error("non-exhaustive boolean match")]
    NonExhaustiveMatch,
    #[error("break/continue used outside a loop")]
    OutsideLoop,
    #[error("operator ? requires an Option or Result value")]
    InvalidTry,
}

#[derive(Clone)]
struct FunctionSig { params: Vec<Type>, result: Type }

pub struct TypeEnv {
    scopes: Vec<HashMap<String, Type>>,
    functions: HashMap<String, FunctionSig>,
    structs: HashMap<String, HashMap<String, Type>>,
    enum_variants: HashMap<String, Option<Type>>,
    errors: Vec<TypeError>,
    return_type: Type,
    loop_depth: usize,
}

impl TypeEnv {
    pub fn new() -> Self {
        let mut functions = HashMap::new();
        functions.insert("print".into(), FunctionSig { params: vec![Type::Unknown], result: Type::Nil });
        functions.insert("println".into(), FunctionSig { params: vec![Type::Unknown], result: Type::Nil });
        functions.insert("len".into(), FunctionSig { params: vec![Type::Unknown], result: Type::Int });
        functions.insert("map".into(), FunctionSig { params: vec![Type::Unknown, Type::Unknown], result: Type::Array(Box::new(Type::Unknown)) });
        functions.insert("filter".into(), FunctionSig { params: vec![Type::Unknown, Type::Unknown], result: Type::Array(Box::new(Type::Unknown)) });
        functions.insert("fold".into(), FunctionSig { params: vec![Type::Unknown, Type::Unknown, Type::Unknown], result: Type::Unknown });
        functions.insert("join".into(), FunctionSig { params: vec![Type::Named("Task".into())], result: Type::Unknown });
        functions.insert("join_timeout".into(), FunctionSig { params: vec![Type::Named("Task".into()), Type::Int], result: Type::Named("Option".into()) });
        functions.insert("cancel".into(), FunctionSig { params: vec![Type::Named("Task".into())], result: Type::Bool });
        functions.insert("channel".into(), FunctionSig { params: vec![Type::Int], result: Type::Tuple(vec![Type::Named("Sender".into()), Type::Named("Receiver".into())]) });
        functions.insert("send".into(), FunctionSig { params: vec![Type::Named("Sender".into()), Type::Unknown], result: Type::Nil });
        functions.insert("recv".into(), FunctionSig { params: vec![Type::Named("Receiver".into())], result: Type::Unknown });
        functions.insert("recv_timeout".into(), FunctionSig { params: vec![Type::Named("Receiver".into()), Type::Int], result: Type::Named("Option".into()) });
        functions.insert("select".into(), FunctionSig { params: vec![Type::Unknown, Type::Int], result: Type::Named("Option".into()) });
        functions.insert("std::net::tcp_listen".into(), FunctionSig { params: vec![Type::String], result: Type::Named("TcpListener".into()) });
        functions.insert("std::net::tcp_local_addr".into(), FunctionSig { params: vec![Type::Named("TcpListener".into())], result: Type::String });
        functions.insert("std::net::tcp_accept".into(), FunctionSig { params: vec![Type::Named("TcpListener".into())], result: Type::Tuple(vec![Type::Named("TcpStream".into()), Type::String]) });
        functions.insert("std::net::tcp_connect".into(), FunctionSig { params: vec![Type::String], result: Type::Named("TcpStream".into()) });
        functions.insert("std::net::tcp_read".into(), FunctionSig { params: vec![Type::Named("TcpStream".into()), Type::Int], result: Type::Named("bytes".into()) });
        functions.insert("std::net::tcp_write".into(), FunctionSig { params: vec![Type::Named("TcpStream".into()), Type::Named("bytes".into())], result: Type::Int });
        functions.insert("std::net::tcp_set_timeout".into(), FunctionSig { params: vec![Type::Named("TcpStream".into()), Type::Int], result: Type::Nil });
        functions.insert("std::net::tcp_close".into(), FunctionSig { params: vec![Type::Unknown], result: Type::Bool });
        functions.insert("std::http::serve_connection".into(), FunctionSig { params: vec![Type::Named("TcpListener".into()), Type::Unknown, Type::Int], result: Type::String });
        functions.insert("std::http::router".into(), FunctionSig { params: vec![], result: Type::Named("HttpRouter".into()) });
        functions.insert("std::http::route".into(), FunctionSig { params: vec![Type::Named("HttpRouter".into()), Type::String, Type::String, Type::Unknown], result: Type::Nil });
        functions.insert("std::http::middleware".into(), FunctionSig { params: vec![Type::Named("HttpRouter".into()), Type::Unknown], result: Type::Nil });
        functions.insert("std::http::after".into(), FunctionSig { params: vec![Type::Named("HttpRouter".into()), Type::Unknown], result: Type::Nil });
        functions.insert("std::http::dispatch".into(), FunctionSig { params: vec![Type::Named("HttpRouter".into()), Type::Unknown], result: Type::Unknown });
        let enum_variants = HashMap::from([
            ("Option::None".into(), None), ("Option::Some".into(), Some(Type::Unknown)),
            ("Result::Ok".into(), Some(Type::Unknown)), ("Result::Err".into(), Some(Type::Unknown)),
        ]);
        Self { scopes: vec![HashMap::new()], functions, structs: HashMap::new(), enum_variants, errors: Vec::new(), return_type: Type::Unknown, loop_depth: 0 }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), Vec<TypeError>> {
        self.errors.clear();
        self.collect_declarations(&program.items);
        for item in &program.items { self.check_item(item); }
        if self.errors.is_empty() { Ok(()) } else { Err(std::mem::take(&mut self.errors)) }
    }

    fn collect_declarations(&mut self, items: &[Item]) {
        for item in items {
            match item {
                Item::Function(function) => {
                    self.functions.insert(function.name.clone(), FunctionSig {
                        params: function.params.iter().map(|p| p.type_ann.as_ref().map(type_from_ast).unwrap_or(Type::Unknown)).collect(),
                        result: function.return_type.as_ref().map(type_from_ast).unwrap_or(Type::Unit),
                    });
                }
                Item::Struct(structure) => {
                    self.structs.insert(structure.name.clone(), structure.fields.iter().map(|f| (f.name.clone(), type_from_ast(&f.type_ann))).collect());
                }
                Item::Enum(enumeration) => for variant in &enumeration.variants {
                    self.enum_variants.insert(format!("{}::{}", enumeration.name, variant.name), variant.payload.as_ref().map(type_from_ast));
                },
                Item::Module(module) => self.collect_declarations(&module.items),
                Item::Impl(block) => for method in &block.methods {
                    self.functions.insert(method.name.clone(), FunctionSig {
                        params: method.params.iter().map(|p| p.type_ann.as_ref().map(type_from_ast).unwrap_or(Type::Unknown)).collect(),
                        result: method.return_type.as_ref().map(type_from_ast).unwrap_or(Type::Unit),
                    });
                },
                _ => {}
            }
        }
    }

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Function(function) => self.check_function(function),
            Item::Impl(block) => for method in &block.methods { self.check_function(method); },
            Item::Module(module) => for item in &module.items { self.check_item(item); },
            Item::Const(constant) => {
                let found = self.check_expr(&constant.value);
                if let Some(expected_ast) = &constant.type_ann { self.require_compatible(&type_from_ast(expected_ast), &found); }
                self.scopes[0].insert(constant.name.clone(), found);
            }
            _ => {}
        }
    }

    fn check_function(&mut self, function: &FunctionDecl) {
        self.push_scope();
        for param in &function.params {
            self.define(param.name.clone(), param.type_ann.as_ref().map(type_from_ast).unwrap_or(Type::Unknown));
            if let Some(default) = &param.default { self.check_expr(default); }
        }
        let old_return = std::mem::replace(&mut self.return_type, function.return_type.as_ref().map(type_from_ast).unwrap_or(Type::Unit));
        if let Some(body) = &function.body {
            let body_type = self.check_block(body);
            if body.final_expr.is_some() && self.return_type != Type::Unit { self.require_compatible(&self.return_type.clone(), &body_type); }
        }
        self.return_type = old_return;
        self.pop_scope();
    }

    fn check_block(&mut self, block: &Block) -> Type {
        self.push_scope();
        for stmt in &block.stmts {
            match stmt {
                Stmt::Let { name, type_ann, value, .. } => {
                    let found = self.check_expr(value);
                    let ty = type_ann.as_ref().map(type_from_ast).unwrap_or_else(|| found.clone());
                    self.require_compatible(&ty, &found);
                    self.define(name.clone(), ty);
                }
                Stmt::Assign { target, value, .. } => { let a = self.check_expr(target); let b = self.check_expr(value); self.require_compatible(&a, &b); }
                Stmt::Expr(expr) => { self.check_expr(expr); }
                Stmt::Item(item) => self.check_item(item),
            }
        }
        let result = block.final_expr.as_ref().map(|e| self.check_expr(e)).unwrap_or(Type::Unit);
        self.pop_scope();
        result
    }

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Int { .. } => Type::Int, Expr::Float { .. } => Type::Float,
            Expr::String { .. } | Expr::StringTemplate { .. } => Type::String,
            Expr::Char { .. } => Type::Char, Expr::Bool { .. } => Type::Bool,
            Expr::Nil { .. } => Type::Nil,
            Expr::Ident { name, .. } => self.lookup(name)
                .or_else(|| self.functions.get(name).map(|s| Type::Function(s.params.clone(), Box::new(s.result.clone()))))
                .or_else(|| self.enum_variants.get(name).and_then(|payload| if payload.is_none() { name.split_once("::").map(|(e, _)| Type::Named(e.into())) } else { None }))
                .unwrap_or_else(|| { self.errors.push(TypeError::UnknownVariable { name: name.clone() }); Type::Unknown }),
            Expr::Array { elements, .. } => {
                let ty = elements.first().map(|e| self.check_expr(e)).unwrap_or(Type::Unknown);
                for element in elements.iter().skip(1) { let found = self.check_expr(element); self.require_compatible(&ty, &found); }
                Type::Array(Box::new(ty))
            }
            Expr::Tuple { elements, .. } => Type::Tuple(elements.iter().map(|e| self.check_expr(e)).collect()),
            Expr::StructLit { name, fields, .. } => {
                if let Some(expected_fields) = self.structs.get(name).cloned() {
                    let supplied: HashSet<_> = fields.iter().map(|(n, _)| n.as_str()).collect();
                    for (field, expected) in &expected_fields {
                        if !supplied.contains(field.as_str()) { self.errors.push(TypeError::MissingField { structure: name.clone(), field: field.clone() }); }
                        if let Some((_, value)) = fields.iter().find(|(n, _)| n == field) { let found = self.check_expr(value); self.require_compatible(expected, &found); }
                    }
                    for (field, value) in fields { if !expected_fields.contains_key(field) { self.errors.push(TypeError::UnknownField { structure: name.clone(), field: field.clone() }); } self.check_expr(value); }
                } else { self.errors.push(TypeError::UnknownVariable { name: name.clone() }); }
                Type::Named(name.clone())
            }
            Expr::Binary { left, op, right, .. } => {
                let left = self.check_expr(left); let right = self.check_expr(right);
                self.check_binary(*op, left, right)
            }
            Expr::Range { start, end, .. } => {
                let a = self.check_expr(start); let b = self.check_expr(end);
                self.require_compatible(&Type::Int, &a); self.require_compatible(&Type::Int, &b);
                Type::Array(Box::new(Type::Int))
            }
            Expr::Unary { op, expr, .. } => {
                let ty = self.check_expr(expr);
                match op { UnaryOp::Not => { self.require_compatible(&Type::Bool, &ty); Type::Bool }, UnaryOp::Neg | UnaryOp::BitNot => ty, _ => ty }
            }
            Expr::Call { callee, args, .. } => self.check_call(callee, args),
            Expr::MethodCall { receiver, method, args, .. } => {
                let receiver_type = self.check_expr(receiver); for arg in args { self.check_expr(arg); }
                match (method.as_str(), args.len(), receiver_type) {
                    ("len", 0, _) => Type::Int,
                    ("map", 1, Type::Array(_)) | ("filter", 1, Type::Array(_)) => Type::Array(Box::new(Type::Unknown)),
                    ("fold", 2, Type::Array(_)) => Type::Unknown,
                    _ => Type::Unknown,
                }
            }
            Expr::Index { target, index, .. } => {
                let target = self.check_expr(target); let index = self.check_expr(index);
                match target {
                    Type::Array(inner) => { self.require_compatible(&Type::Int, &index); *inner }
                    Type::String => { self.require_compatible(&Type::Int, &index); Type::Char }
                    Type::Named(name) if name == "bytes" => { self.require_compatible(&Type::Int, &index); Type::Int }
                    Type::Named(name) if name == "map" => { self.require_compatible(&Type::String, &index); Type::Unknown }
                    Type::Unknown => Type::Unknown,
                    _ => Type::Unknown,
                }
            }
            Expr::FieldAccess { target, field, .. } => {
                match self.check_expr(target) {
                    Type::Named(name) if name == "map" => Type::Unknown,
                    Type::Named(name) => self.structs.get(&name).and_then(|s| s.get(field)).cloned().unwrap_or_else(|| { self.errors.push(TypeError::UnknownField { structure: name, field: field.clone() }); Type::Unknown }),
                    _ => Type::Unknown,
                }
            }
            Expr::If { condition, then_branch, else_branch, .. } => {
                let condition = self.check_expr(condition); self.require_compatible(&Type::Bool, &condition);
                let a = self.check_block(then_branch);
                if let Some(other) = else_branch { let b = self.check_block(other); self.require_compatible(&a, &b); a } else { Type::Unit }
            }
            Expr::Match { scrutinee, arms, .. } => {
                let subject = self.check_expr(scrutinee); let mut result = Type::Unknown; let mut wildcard = false; let mut bools = HashSet::new();
                for arm in arms {
                    self.push_scope(); self.bind_pattern(&arm.pattern, &subject, &mut wildcard, &mut bools);
                    if let Some(guard) = &arm.guard { let g = self.check_expr(guard); self.require_compatible(&Type::Bool, &g); }
                    let found = self.check_block(&arm.body); if result == Type::Unknown { result = found; } else { self.require_compatible(&result, &found); }
                    self.pop_scope();
                }
                if subject == Type::Bool && !wildcard && bools.len() < 2 { self.errors.push(TypeError::NonExhaustiveMatch); }
                result
            }
            Expr::For { pattern, iterator, body, .. } => {
                let item = match self.check_expr(iterator) { Type::Array(inner) => *inner, _ => Type::Unknown };
                self.push_scope(); let mut w = false; let mut b = HashSet::new(); self.bind_pattern(pattern, &item, &mut w, &mut b);
                self.loop_depth += 1; self.check_block(body); self.loop_depth -= 1; self.pop_scope(); Type::Unit
            }
            Expr::While { condition, body, .. } => { let c = self.check_expr(condition); self.require_compatible(&Type::Bool, &c); self.loop_depth += 1; self.check_block(body); self.loop_depth -= 1; Type::Unit }
            Expr::Loop { body, .. } => { self.loop_depth += 1; self.check_block(body); self.loop_depth -= 1; Type::Unit }
            Expr::Break { value, .. } => {
                if self.loop_depth == 0 { self.errors.push(TypeError::OutsideLoop); }
                if let Some(value) = value { self.check_expr(value); }
                Type::Never
            }
            Expr::Continue { .. } => { if self.loop_depth == 0 { self.errors.push(TypeError::OutsideLoop); } Type::Never }
            Expr::Return { value, .. } => { let found = value.as_ref().map(|v| self.check_expr(v)).unwrap_or(Type::Unit); self.require_compatible(&self.return_type.clone(), &found); Type::Never }
            Expr::Let { name, type_ann, value, .. } => { let found = self.check_expr(value); let ty = type_ann.as_ref().map(type_from_ast).unwrap_or(found); self.define(name.clone(), ty.clone()); ty }
            Expr::Assign { target, value, .. } => { let a = self.check_expr(target); let b = self.check_expr(value); self.require_compatible(&a, &b); a }
            Expr::Block(block) => self.check_block(block),
            Expr::Spawn { expr, .. } => { let ty = self.check_expr(expr); if !matches!(ty, Type::Function(_, _)) { self.errors.push(TypeError::NotCallable { name: "spawn expression".into() }); } Type::Named("Task".into()) },
            Expr::Try { expr, .. } => match self.check_expr(expr) {
                Type::Named(name) if name == "Option" || name == "Result" => Type::Unknown,
                Type::Unknown => Type::Unknown,
                _ => { self.errors.push(TypeError::InvalidTry); Type::Unknown }
            },
            Expr::Closure { params, return_type, body, .. } => {
                self.push_scope(); let p: Vec<Type> = params.iter().map(|x| x.type_ann.as_ref().map(type_from_ast).unwrap_or(Type::Unknown)).collect();
                for (param, ty) in params.iter().zip(&p) { self.define(param.name.clone(), ty.clone()); }
                let actual = self.check_expr(body); let result = return_type.as_ref().map(type_from_ast).unwrap_or_else(|| actual.clone()); self.require_compatible(&result, &actual); self.pop_scope(); Type::Function(p, Box::new(result))
            }
        }
    }

    fn check_call(&mut self, callee: &Expr, args: &[Expr]) -> Type {
        let name = if let Expr::Ident { name, .. } = callee { Some(name.clone()) } else { None };
        if let Some(name) = &name {
            if let Some(signature) = titan_stdlib::native::lookup(name) {
                if args.len() != signature.params.len() { self.errors.push(TypeError::Arity { expected: signature.params.len(), found: args.len() }); }
                for (argument, expected) in args.iter().zip(signature.params) {
                    let found = self.check_expr(argument);
                    let expected = native_type(*expected);
                    if !native_compatible(&expected, &found) { self.errors.push(TypeError::Mismatch { expected, found }); }
                }
                for argument in args.iter().skip(signature.params.len()) { self.check_expr(argument); }
                return native_type(signature.result);
            }
            if let Some(payload) = self.enum_variants.get(name).cloned() {
                let expected = usize::from(payload.is_some());
                if args.len() != expected { self.errors.push(TypeError::Arity { expected, found: args.len() }); }
                if let (Some(expected), Some(argument)) = (payload, args.first()) { let found = self.check_expr(argument); self.require_compatible(&expected, &found); }
                return Type::Named(name.split_once("::").map_or(name.as_str(), |(e, _)| e).into());
            }
        }
        let ty = self.check_expr(callee);
        match ty {
            Type::Function(params, result) => {
                if params.len() != args.len() && name.as_deref() != Some("print") && name.as_deref() != Some("println") { self.errors.push(TypeError::Arity { expected: params.len(), found: args.len() }); }
                for (arg, expected) in args.iter().zip(&params) { let found = self.check_expr(arg); self.require_compatible(expected, &found); }
                for arg in args.iter().skip(params.len()) { self.check_expr(arg); }
                *result
            }
            Type::Unknown => { for arg in args { self.check_expr(arg); } Type::Unknown }
            _ => { self.errors.push(TypeError::NotCallable { name: name.unwrap_or_else(|| "expression".into()) }); Type::Unknown }
        }
    }

    fn check_binary(&mut self, op: BinaryOp, left: Type, right: Type) -> Type {
        use BinaryOp::*;
        match op {
            Eq | Neq => { self.require_compatible(&left, &right); Type::Bool }
            Lt | Gt | Lte | Gte => { if !is_numeric(&left) || !compatible(&left, &right) { self.invalid(op, left, right); } Type::Bool }
            LazyAnd | LazyOr => { self.require_compatible(&Type::Bool, &left); self.require_compatible(&Type::Bool, &right); Type::Bool }
            Add if left == Type::String && right == Type::String => Type::String,
            Add | Sub | Mul | Div | Mod if is_numeric(&left) && compatible(&left, &right) => left,
            And | Or | Xor if left == Type::Int && right == Type::Int => Type::Int,
            _ => { self.invalid(op, left, right); Type::Unknown }
        }
    }

    fn invalid(&mut self, op: BinaryOp, left: Type, right: Type) { self.errors.push(TypeError::InvalidOperands { operator: format!("{op:?}"), left, right }); }
    fn require_compatible(&mut self, expected: &Type, found: &Type) { if !compatible(expected, found) { self.errors.push(TypeError::Mismatch { expected: expected.clone(), found: found.clone() }); } }
    fn bind_pattern(&mut self, pattern: &Pattern, subject: &Type, wildcard: &mut bool, bools: &mut HashSet<bool>) {
        match pattern {
            Pattern::Wildcard { .. } => *wildcard = true,
            Pattern::Ident { name, .. } => { self.define(name.clone(), subject.clone()); *wildcard = true; }
            Pattern::Literal { value, .. } => { if let Expr::Bool { value, .. } = value.as_ref() { bools.insert(*value); } let found = self.check_expr(value); self.require_compatible(subject, &found); }
            Pattern::Or { left, right, .. } => { self.bind_pattern(left, subject, wildcard, bools); self.bind_pattern(right, subject, wildcard, bools); }
            Pattern::Enum { inner, .. } => if let Some(inner) = inner { self.bind_pattern(inner, &Type::Unknown, wildcard, bools); },
            Pattern::Tuple { elements, .. } => for element in elements { self.bind_pattern(element, &Type::Unknown, wildcard, bools); },
            Pattern::Struct { fields, .. } => for (_, pattern) in fields { self.bind_pattern(pattern, &Type::Unknown, wildcard, bools); },
        }
    }
    fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop_scope(&mut self) { self.scopes.pop(); }
    fn define(&mut self, name: String, ty: Type) { self.scopes.last_mut().unwrap().insert(name, ty); }
    fn lookup(&self, name: &str) -> Option<Type> { self.scopes.iter().rev().find_map(|s| s.get(name).cloned()) }
}

fn type_from_ast(ty: &TypeExpr) -> Type {
    match ty {
        TypeExpr::Named { name, generics } => match name.as_str() {
            "int" | "i32" | "i64" | "u64" | "usize" => Type::Int,
            "float" | "f32" | "f64" => Type::Float, "bool" => Type::Bool,
            "string" | "str" => Type::String, "char" => Type::Char,
            "Array" | "Vec" if !generics.is_empty() => Type::Array(Box::new(type_from_ast(&generics[0]))),
            _ => Type::Named(name.clone()),
        },
        TypeExpr::Slice { inner } | TypeExpr::Array { inner, .. } => Type::Array(Box::new(type_from_ast(inner))),
        TypeExpr::Tuple { elements } => Type::Tuple(elements.iter().map(type_from_ast).collect()),
        TypeExpr::Function { params, return_type } => Type::Function(params.iter().map(type_from_ast).collect(), Box::new(type_from_ast(return_type))),
        TypeExpr::Unit => Type::Unit, TypeExpr::Never => Type::Never, TypeExpr::Infer(_) => Type::Unknown,
        TypeExpr::Reference { inner, .. } => type_from_ast(inner),
    }
}
fn native_type(ty: titan_stdlib::native::NativeType) -> Type {
    use titan_stdlib::native::NativeType;
    match ty {
        NativeType::Any => Type::Unknown, NativeType::Int => Type::Int,
        NativeType::Float => Type::Float, NativeType::Bool => Type::Bool,
        NativeType::String => Type::String, NativeType::Bytes => Type::Named("bytes".into()),
        NativeType::Array => Type::Array(Box::new(Type::Unknown)),
        NativeType::Map => Type::Named("map".into()), NativeType::Nil => Type::Nil,
    }
}
fn native_compatible(expected: &Type, found: &Type) -> bool {
    if compatible(expected, found) || (expected == &Type::Float && found == &Type::Int) { return true; }
    match (expected, found) {
        (Type::Array(expected), Type::Array(found)) => native_compatible(expected, found),
        (Type::Array(expected), Type::Tuple(found)) => found.iter().all(|item| native_compatible(expected, item)),
        _ => false,
    }
}
fn compatible(a: &Type, b: &Type) -> bool { a == b || matches!(a, Type::Unknown | Type::Never) || matches!(b, Type::Unknown | Type::Never) || (matches!(a, Type::Unit) && matches!(b, Type::Nil)) }
fn is_numeric(ty: &Type) -> bool { matches!(ty, Type::Int | Type::Float | Type::Unknown) }

impl Default for TypeEnv { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    use titan_lexer::Lexer;
    use titan_parser::Parser;

    fn check(source: &str) -> Result<(), Vec<TypeError>> {
        let mut lexer = Lexer::new(source); let tokens = lexer.tokenize().0.to_vec();
        let program = Parser::new(tokens).parse_program().unwrap(); TypeEnv::new().check_program(&program)
    }
    #[test] fn accepts_recursive_typed_function() { assert!(check("fn fib(n: int) -> int { if n <= 1 { return n } fib(n-1) + fib(n-2) }").is_ok()); }
    #[test] fn rejects_unknown_names() { assert!(check("fn main() { missing + 1 }").is_err()); }
    #[test] fn rejects_wrong_return() { assert!(check("fn bad() -> int { return true }").is_err()); }
    #[test] fn checks_registered_native_signatures() { assert!(check("fn main() { std::text::reverse(\"Titan\") }").is_ok()); assert!(check("fn main() { std::text::reverse(42) }").is_err()); }
    #[test] fn generic_native_arrays_accept_concrete_elements() { assert!(check("fn main() { std::stats::mean([10, 20, 30, 40]) }").is_ok()); }
    #[test] fn checks_tasks_and_channels() { assert!(check("fn main() { let endpoints = channel(1) let task = spawn || 42 join(task) endpoints }").is_ok()); assert!(check("fn main() { spawn 42 }").is_err()); }
    #[test] fn checks_tcp_handle_and_byte_signatures() { assert!(check("fn main() { let listener = std::net::tcp_listen(\"127.0.0.1:0\") let address = std::net::tcp_local_addr(listener) let stream = std::net::tcp_connect(address) let bytes = std::encoding::utf8_encode(\"ping\") std::net::tcp_write(stream, bytes) }").is_ok()); }
}
