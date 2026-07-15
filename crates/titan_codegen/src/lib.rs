//! AST to portable Titan bytecode compiler.

mod artifact;
pub use artifact::{ArtifactError, BytecodeArtifact};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use titan_ast::*;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum CodegenError {
    #[error("unknown function '{0}'")]
    UnknownFunction(String),
    #[error("unknown variable '{0}'")]
    UnknownVariable(String),
    #[error("unsupported construct: {0}")]
    Unsupported(String),
    #[error("break or continue outside a loop")]
    OutsideLoop,
    #[error("invalid interpolation expression '{0}'")]
    InvalidInterpolation(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    PushInt(i64), PushFloat(f64), PushBool(bool), PushChar(char), PushNil,
    PushStr(usize), PushLocal(usize), StoreLocal(usize), Pop, Dup,
    Add, Sub, Mul, Div, Mod, Neg, Not, BitNot,
    Eq, Neq, Lt, Gt, Lte, Gte, BitAnd, BitOr, BitXor,
    Jump(usize), JumpIfFalse(usize),
    Call { function: usize, argc: usize }, CallNative { name: String, argc: usize },
    MakeClosure { function: usize, captures: Vec<usize> }, CallValue(usize), Try,
    ArrayMap, ArrayFilter, ArrayFold,
    Spawn, JoinTask, JoinTaskTimeout, CancelTask, NewChannel, ChannelSend, ChannelRecv, ChannelRecvTimeout, ChannelSelect,
    TcpListen, TcpLocalAddr, TcpAccept, TcpConnect, TcpRead, TcpWrite, TcpSetTimeout, TcpClose,
    HttpServeConnection, HttpRouterNew, HttpRouteAdd, HttpMiddlewareAdd, HttpAfterAdd, HttpErrorHandlerAdd, HttpDispatch,
    TlsConnect, TlsServerConfig, TlsAccept, TlsRead, TlsWrite, TlsClose,
    WsDecoderNew, WsDecoderPush, WsDecoderNext,
    WsConnect, WsAttachTcp, WsAttachTls, WsSendText, WsSendBinary, WsReceive, WsClose,
    ServerControlNew, ServerTryAcquire, ServerRelease, ServerShutdown, ServerStats, ServerHealthResponse, Ret,
    Print(usize), Len, ToString,
    NewArray(usize), NewTuple(usize), Index,
    NewStruct { name: String, fields: Vec<String> }, GetField(String),
    NewEnum { name: String, variant: String, has_payload: bool },
    EnumIs { name: String, variant: String }, EnumPayload,
    Nop, Halt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}
impl From<titan_lexer::Span> for SourceLocation {
    fn from(span: titan_lexer::Span) -> Self { Self { start: span.start, end: span.end, line: span.line, column: span.column } }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BytecodeFunc {
    pub name: String,
    #[serde(default)]
    pub source_file: Option<String>,
    pub arity: usize,
    pub captures: usize,
    pub locals: usize,
    pub max_stack: usize,
    pub code: Vec<Op>,
    #[serde(default)]
    pub debug_locations: Vec<Option<SourceLocation>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledModule {
    pub functions: Vec<BytecodeFunc>,
    pub entry: usize,
    pub string_table: Vec<String>,
}

pub struct AstCompiler {
    module: CompiledModule,
    current: BytecodeFunc,
    locals: Vec<HashMap<String, usize>>,
    next_local: usize,
    strings: HashMap<String, usize>,
    function_ids: HashMap<String, usize>,
    enum_variants: HashMap<String, bool>,
    constants: HashMap<String, Expr>,
    loops: Vec<LoopContext>,
    current_location: Option<SourceLocation>,
}

#[derive(Default)]
struct LoopContext { breaks: Vec<usize>, continues: Vec<usize>, continue_target: usize }

impl AstCompiler {
    pub fn new() -> Self {
        Self {
            module: CompiledModule { functions: Vec::new(), entry: 0, string_table: Vec::new() },
            current: empty_function(), locals: Vec::new(), next_local: 0,
            strings: HashMap::new(), function_ids: HashMap::new(), enum_variants: HashMap::new(), constants: HashMap::new(), loops: Vec::new(), current_location: None,
        }
    }

    pub fn compile_program(&mut self, program: &Program) -> Result<CompiledModule, CodegenError> {
        self.module = CompiledModule { functions: Vec::new(), entry: 0, string_table: Vec::new() };
        self.strings.clear(); self.function_ids.clear(); self.enum_variants.clear(); self.constants.clear();
        self.enum_variants.extend([("Option::None".into(), false), ("Option::Some".into(), true), ("Result::Ok".into(), true), ("Result::Err".into(), true)]);
        let mut functions = Vec::new();
        collect_items(&program.items, &mut functions, &mut self.constants, &mut self.enum_variants);
        for (index, function) in functions.iter().enumerate() {
            if self.function_ids.insert(function.name.clone(), index).is_some() {
                return Err(CodegenError::Unsupported(format!("duplicate function '{}'", function.name)));
            }
        }
        let Some(entry) = self.function_ids.get("main").copied() else { return Err(CodegenError::UnknownFunction("main".into())); };
        self.module.entry = entry;
        self.module.functions = vec![empty_function(); functions.len()];
        for (index, function) in functions.into_iter().enumerate() {
            let compiled = self.compile_function(function)?;
            self.module.functions[index] = compiled;
        }
        Ok(self.module.clone())
    }

    fn compile_function(&mut self, function: &FunctionDecl) -> Result<BytecodeFunc, CodegenError> {
        self.current = BytecodeFunc { name: function.name.clone(), source_file: function.source_file.clone(), arity: function.params.len(), captures: 0, locals: 0, max_stack: 256, code: Vec::new(), debug_locations: Vec::new() };
        self.locals = vec![HashMap::new()]; self.next_local = 0; self.loops.clear();
        for param in &function.params { self.add_local(&param.name); }
        if let Some(body) = &function.body { self.compile_block(body, true)?; } else { self.emit(Op::PushNil); }
        if !matches!(self.current.code.last(), Some(Op::Ret)) { self.emit(Op::Ret); }
        self.current.locals = self.next_local;
        Ok(self.current.clone())
    }

    fn compile_block(&mut self, block: &Block, value_needed: bool) -> Result<(), CodegenError> {
        self.push_scope();
        for stmt in &block.stmts { self.compile_stmt(stmt)?; }
        if let Some(expr) = &block.final_expr {
            self.compile_expr(expr)?;
            if !value_needed { self.emit(Op::Pop); }
        } else if value_needed { self.emit(Op::PushNil); }
        self.pop_scope();
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CodegenError> {
        match stmt {
            Stmt::Expr(expr) => { self.compile_expr(expr)?; if !is_terminal(expr) { self.emit(Op::Pop); } }
            Stmt::Let { name, value, .. } => { self.compile_expr(value)?; let local = self.add_local(name); self.emit(Op::StoreLocal(local)); }
            Stmt::Assign { target, op, value, .. } => { self.compile_assignment(target, *op, value, false)?; }
            Stmt::Item(_) => return Err(CodegenError::Unsupported("nested declarations are not executable yet".into())),
        }
        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<(), CodegenError> {
        let previous = self.current_location.replace(expr.span().into());
        let result = self.compile_expr_inner(expr);
        self.current_location = previous;
        result
    }

    fn compile_expr_inner(&mut self, expr: &Expr) -> Result<(), CodegenError> {
        match expr {
            Expr::Int { value, .. } => self.emit(Op::PushInt(*value)),
            Expr::Float { value, .. } => self.emit(Op::PushFloat(*value)),
            Expr::Bool { value, .. } => self.emit(Op::PushBool(*value)),
            Expr::Char { value, .. } => self.emit(Op::PushChar(*value)),
            Expr::Nil { .. } => self.emit(Op::PushNil),
            Expr::String { value, .. } => { let id = self.intern(value); self.emit(Op::PushStr(id)); }
            Expr::StringTemplate { value, .. } => self.compile_template(value)?,
            Expr::Ident { name, .. } => {
                if let Some(local) = self.find_local(name) { self.emit(Op::PushLocal(local)); }
                else if let Some(value) = self.constants.get(name).cloned() { self.compile_expr(&value)?; }
                else if self.enum_variants.get(name) == Some(&false) {
                    let (enum_name, variant) = split_variant(name)?;
                    self.emit(Op::NewEnum { name: enum_name.into(), variant: variant.into(), has_payload: false });
                }
                else if let Some(function) = self.function_ids.get(name).copied() { self.emit(Op::MakeClosure { function, captures: Vec::new() }); }
                else { return Err(CodegenError::UnknownVariable(name.clone())); }
            }
            Expr::Array { elements, .. } => { for element in elements { self.compile_expr(element)?; } self.emit(Op::NewArray(elements.len())); }
            Expr::Tuple { elements, .. } => { for element in elements { self.compile_expr(element)?; } self.emit(Op::NewTuple(elements.len())); }
            Expr::StructLit { name, fields, .. } => {
                for (_, value) in fields { self.compile_expr(value)?; }
                self.emit(Op::NewStruct { name: name.clone(), fields: fields.iter().map(|(n, _)| n.clone()).collect() });
            }
            Expr::Binary { left, op, right, .. } => self.compile_binary(left, *op, right)?,
            Expr::Range { start, end, inclusive, .. } => self.compile_range(start, end, *inclusive)?,
            Expr::Unary { op, expr, .. } => {
                self.compile_expr(expr)?;
                self.emit(match op { UnaryOp::Neg => Op::Neg, UnaryOp::Not => Op::Not, UnaryOp::BitNot => Op::BitNot, _ => Op::Nop });
            }
            Expr::Call { callee, args, .. } => self.compile_call(callee, args)?,
            Expr::MethodCall { receiver, method, args, .. } => {
                self.compile_expr(receiver)?;
                for arg in args { self.compile_expr(arg)?; }
                match (method.as_str(), args.len()) {
                    ("len", 0) => self.emit(Op::Len), ("map", 1) => self.emit(Op::ArrayMap),
                    ("filter", 1) => self.emit(Op::ArrayFilter), ("fold", 2) => self.emit(Op::ArrayFold),
                    _ => return Err(CodegenError::Unsupported(format!("method call .{method}()"))),
                }
            }
            Expr::Index { target, index, .. } => { self.compile_expr(target)?; self.compile_expr(index)?; self.emit(Op::Index); }
            Expr::FieldAccess { target, field, .. } => { self.compile_expr(target)?; self.emit(Op::GetField(field.clone())); }
            Expr::If { condition, then_branch, else_branch, .. } => {
                self.compile_expr(condition)?;
                let else_jump = self.jump_if_false();
                self.compile_block(then_branch, true)?;
                let end_jump = self.jump();
                self.patch(else_jump, self.position());
                if let Some(other) = else_branch { self.compile_block(other, true)?; } else { self.emit(Op::PushNil); }
                self.patch(end_jump, self.position());
            }
            Expr::While { condition, body, .. } => self.compile_while(condition, body)?,
            Expr::For { pattern, iterator, body, .. } => self.compile_for(pattern, iterator, body)?,
            Expr::Loop { body, .. } => self.compile_loop(body)?,
            Expr::Break { value, .. } => {
                if let Some(value) = value { self.compile_expr(value)?; self.emit(Op::Pop); }
                let jump = self.jump();
                self.loops.last_mut().ok_or(CodegenError::OutsideLoop)?.breaks.push(jump);
            }
            Expr::Continue { .. } => {
                let jump = self.jump();
                self.loops.last_mut().ok_or(CodegenError::OutsideLoop)?.continues.push(jump);
            }
            Expr::Return { value, .. } => { if let Some(value) = value { self.compile_expr(value)?; } else { self.emit(Op::PushNil); } self.emit(Op::Ret); }
            Expr::Let { name, value, .. } => { self.compile_expr(value)?; self.emit(Op::Dup); let local = self.add_local(name); self.emit(Op::StoreLocal(local)); }
            Expr::Assign { target, op, value, .. } => self.compile_assignment(target, *op, value, true)?,
            Expr::Block(block) => self.compile_block(block, true)?,
            Expr::Match { scrutinee, arms, .. } => self.compile_match(scrutinee, arms)?,
            Expr::Spawn { expr, .. } => { self.compile_expr(expr)?; self.emit(Op::Spawn); },
            Expr::Try { expr, .. } => { self.compile_expr(expr)?; self.emit(Op::Try); }
            Expr::Closure { params, body, .. } => self.compile_closure(params, body)?,
        }
        Ok(())
    }

    fn compile_binary(&mut self, left: &Expr, op: BinaryOp, right: &Expr) -> Result<(), CodegenError> {
        if matches!(op, BinaryOp::LazyAnd | BinaryOp::LazyOr) {
            self.compile_expr(left)?;
            self.emit(Op::Dup);
            let jump = self.jump_if_false();
            if op == BinaryOp::LazyOr { let evaluate = self.jump(); self.patch(jump, self.position()); self.emit(Op::Pop); self.compile_expr(right)?; let end = self.jump(); self.patch(evaluate, self.position()); self.patch(end, self.position()); }
            else { self.emit(Op::Pop); self.compile_expr(right)?; self.patch(jump, self.position()); }
            return Ok(());
        }
        self.compile_expr(left)?; self.compile_expr(right)?;
        self.emit(match op {
            BinaryOp::Add => Op::Add, BinaryOp::Sub => Op::Sub, BinaryOp::Mul => Op::Mul,
            BinaryOp::Div => Op::Div, BinaryOp::Mod => Op::Mod, BinaryOp::Eq => Op::Eq,
            BinaryOp::Neq => Op::Neq, BinaryOp::Lt => Op::Lt, BinaryOp::Gt => Op::Gt,
            BinaryOp::Lte => Op::Lte, BinaryOp::Gte => Op::Gte, BinaryOp::And => Op::BitAnd,
            BinaryOp::Or => Op::BitOr, BinaryOp::Xor => Op::BitXor,
            BinaryOp::LazyAnd | BinaryOp::LazyOr => unreachable!(),
        });
        Ok(())
    }

    fn compile_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<(), CodegenError> {
        if let Expr::Ident { name, .. } = callee {
            if let Some(local) = self.find_local(name) {
                self.emit(Op::PushLocal(local));
                for arg in args { self.compile_expr(arg)?; }
                self.emit(Op::CallValue(args.len()));
                return Ok(());
            }
            for arg in args { self.compile_expr(arg)?; }
            match name.as_str() {
                "print" | "println" => self.emit(Op::Print(args.len())),
                "len" if args.len() == 1 => self.emit(Op::Len),
                "map" if args.len() == 2 => self.emit(Op::ArrayMap),
                "filter" if args.len() == 2 => self.emit(Op::ArrayFilter),
                "fold" if args.len() == 3 => self.emit(Op::ArrayFold),
                "join" if args.len() == 1 => self.emit(Op::JoinTask),
                "join_timeout" if args.len() == 2 => self.emit(Op::JoinTaskTimeout),
                "cancel" if args.len() == 1 => self.emit(Op::CancelTask),
                "channel" if args.len() == 1 => self.emit(Op::NewChannel),
                "send" if args.len() == 2 => self.emit(Op::ChannelSend),
                "recv" if args.len() == 1 => self.emit(Op::ChannelRecv),
                "recv_timeout" if args.len() == 2 => self.emit(Op::ChannelRecvTimeout),
                "select" if args.len() == 2 => self.emit(Op::ChannelSelect),
                "std::net::tcp_listen" if args.len() == 1 => self.emit(Op::TcpListen),
                "std::net::tcp_local_addr" if args.len() == 1 => self.emit(Op::TcpLocalAddr),
                "std::net::tcp_accept" if args.len() == 1 => self.emit(Op::TcpAccept),
                "std::net::tcp_connect" if args.len() == 1 => self.emit(Op::TcpConnect),
                "std::net::tcp_read" if args.len() == 2 => self.emit(Op::TcpRead),
                "std::net::tcp_write" if args.len() == 2 => self.emit(Op::TcpWrite),
                "std::net::tcp_set_timeout" if args.len() == 2 => self.emit(Op::TcpSetTimeout),
                "std::net::tcp_close" if args.len() == 1 => self.emit(Op::TcpClose),
                "std::http::serve_connection" if args.len() == 3 => self.emit(Op::HttpServeConnection),
                "std::http::router" if args.is_empty() => self.emit(Op::HttpRouterNew),
                "std::http::route" if args.len() == 4 => self.emit(Op::HttpRouteAdd),
                "std::http::middleware" if args.len() == 2 => self.emit(Op::HttpMiddlewareAdd),
                "std::http::after" if args.len() == 2 => self.emit(Op::HttpAfterAdd),
                "std::http::on_error" if args.len() == 2 => self.emit(Op::HttpErrorHandlerAdd),
                "std::http::dispatch" if args.len() == 2 => self.emit(Op::HttpDispatch),
                "std::tls::connect" if args.len() == 2 => self.emit(Op::TlsConnect),
                "std::tls::server_config" if args.len() == 2 => self.emit(Op::TlsServerConfig),
                "std::tls::accept" if args.len() == 2 => self.emit(Op::TlsAccept),
                "std::tls::read" if args.len() == 2 => self.emit(Op::TlsRead),
                "std::tls::write" if args.len() == 2 => self.emit(Op::TlsWrite),
                "std::tls::close" if args.len() == 1 => self.emit(Op::TlsClose),
                "std::ws::decoder" if args.len() == 1 => self.emit(Op::WsDecoderNew),
                "std::ws::decoder_push" if args.len() == 2 => self.emit(Op::WsDecoderPush),
                "std::ws::decoder_next" if args.len() == 2 => self.emit(Op::WsDecoderNext),
                "std::ws::connect" if args.len() == 3 => self.emit(Op::WsConnect),
                "std::ws::attach_tcp" if args.len() == 3 => self.emit(Op::WsAttachTcp),
                "std::ws::attach_tls" if args.len() == 3 => self.emit(Op::WsAttachTls),
                "std::ws::send_text" if args.len() == 2 => self.emit(Op::WsSendText),
                "std::ws::send_binary" if args.len() == 2 => self.emit(Op::WsSendBinary),
                "std::ws::receive" if args.len() == 1 => self.emit(Op::WsReceive),
                "std::ws::close" if args.len() == 3 => self.emit(Op::WsClose),
                "std::server::control" if args.len() == 1 => self.emit(Op::ServerControlNew),
                "std::server::try_acquire" if args.len() == 1 => self.emit(Op::ServerTryAcquire),
                "std::server::release" if args.len() == 1 => self.emit(Op::ServerRelease),
                "std::server::shutdown" if args.len() == 1 => self.emit(Op::ServerShutdown),
                "std::server::stats" if args.len() == 1 => self.emit(Op::ServerStats),
                "std::server::health_response" if args.len() == 1 => self.emit(Op::ServerHealthResponse),
                _ if titan_stdlib::native::contains(name) => self.emit(Op::CallNative { name: name.clone(), argc: args.len() }),
                _ if self.enum_variants.contains_key(name) => {
                    let has_payload = self.enum_variants[name];
                    if args.len() != usize::from(has_payload) { return Err(CodegenError::Unsupported(format!("wrong payload count for enum variant '{name}'"))); }
                    let (enum_name, variant) = split_variant(name)?;
                    self.emit(Op::NewEnum { name: enum_name.into(), variant: variant.into(), has_payload });
                }
                _ => {
                    let function = self.function_ids.get(name).copied().ok_or_else(|| CodegenError::UnknownFunction(name.clone()))?;
                    self.emit(Op::Call { function, argc: args.len() });
                }
            }
        } else {
            self.compile_expr(callee)?;
            for arg in args { self.compile_expr(arg)?; }
            self.emit(Op::CallValue(args.len()));
        }
        Ok(())
    }

    fn compile_assignment(&mut self, target: &Expr, op: Option<BinaryOp>, value: &Expr, keep: bool) -> Result<(), CodegenError> {
        let Expr::Ident { name, .. } = target else { return Err(CodegenError::Unsupported("assignment target must currently be a variable".into())); };
        let local = self.find_local(name).ok_or_else(|| CodegenError::UnknownVariable(name.clone()))?;
        if let Some(op) = op { self.emit(Op::PushLocal(local)); self.compile_expr(value)?; self.emit(binary_instruction(op)?); }
        else { self.compile_expr(value)?; }
        if keep { self.emit(Op::Dup); }
        self.emit(Op::StoreLocal(local));
        Ok(())
    }

    fn compile_while(&mut self, condition: &Expr, body: &Block) -> Result<(), CodegenError> {
        let start = self.position(); self.compile_expr(condition)?; let exit = self.jump_if_false();
        self.loops.push(LoopContext { continue_target: start, ..Default::default() });
        self.compile_block(body, false)?; self.emit(Op::Jump(start));
        let end = self.position(); self.patch(exit, end); self.finish_loop(end); self.emit(Op::PushNil); Ok(())
    }

    fn compile_loop(&mut self, body: &Block) -> Result<(), CodegenError> {
        let start = self.position(); self.loops.push(LoopContext { continue_target: start, ..Default::default() });
        self.compile_block(body, false)?; self.emit(Op::Jump(start));
        let end = self.position(); self.finish_loop(end); self.emit(Op::PushNil); Ok(())
    }

    fn compile_for(&mut self, pattern: &Pattern, iterator: &Expr, body: &Block) -> Result<(), CodegenError> {
        let Pattern::Ident { name, .. } = pattern else { return Err(CodegenError::Unsupported("for currently requires an identifier pattern".into())); };
        // General arrays use an index and len; ranges are optimized but share the same representation.
        self.compile_expr(iterator)?; let array = self.add_temp("$iter"); self.emit(Op::StoreLocal(array));
        self.emit(Op::PushInt(0)); let index = self.add_temp("$index"); self.emit(Op::StoreLocal(index));
        self.push_scope(); let item = self.add_local(name);
        let start = self.position(); self.emit(Op::PushLocal(index)); self.emit(Op::PushLocal(array)); self.emit(Op::Len); self.emit(Op::Lt); let exit = self.jump_if_false();
        self.emit(Op::PushLocal(array)); self.emit(Op::PushLocal(index)); self.emit(Op::Index); self.emit(Op::StoreLocal(item));
        self.loops.push(LoopContext { continue_target: 0, ..Default::default() });
        self.compile_block(body, false)?;
        let increment = self.position(); if let Some(context) = self.loops.last_mut() { context.continue_target = increment; }
        self.emit(Op::PushLocal(index)); self.emit(Op::PushInt(1)); self.emit(Op::Add); self.emit(Op::StoreLocal(index)); self.emit(Op::Jump(start));
        let end = self.position(); self.patch(exit, end); self.finish_loop(end); self.pop_scope(); self.emit(Op::PushNil); Ok(())
    }

    fn compile_range(&mut self, start: &Expr, end: &Expr, inclusive: bool) -> Result<(), CodegenError> {
        // Runtime helper encoded as an intrinsic function index sentinel.
        self.compile_expr(start)?; self.compile_expr(end)?; self.emit(Op::PushBool(inclusive));
        self.emit(Op::Call { function: usize::MAX, argc: 3 }); Ok(())
    }

    fn compile_match(&mut self, scrutinee: &Expr, arms: &[MatchArm]) -> Result<(), CodegenError> {
        self.compile_expr(scrutinee)?; let subject = self.add_temp("$match"); self.emit(Op::StoreLocal(subject));
        let mut ends = Vec::new();
        for arm in arms {
            self.push_scope();
            let failure = match &arm.pattern {
                Pattern::Wildcard { .. } => None,
                Pattern::Ident { name, .. } => { self.emit(Op::PushLocal(subject)); let local = self.add_local(name); self.emit(Op::StoreLocal(local)); None }
                Pattern::Literal { value, .. } => { self.emit(Op::PushLocal(subject)); self.compile_expr(value)?; self.emit(Op::Eq); Some(self.jump_if_false()) }
                Pattern::Enum { name, variant, inner, .. } => {
                    self.emit(Op::PushLocal(subject));
                    self.emit(Op::EnumIs { name: name.clone(), variant: variant.clone() });
                    let failure = self.jump_if_false();
                    if let Some(inner) = inner {
                        self.emit(Op::PushLocal(subject)); self.emit(Op::EnumPayload);
                        match inner.as_ref() {
                            Pattern::Ident { name, .. } => { let local = self.add_local(name); self.emit(Op::StoreLocal(local)); }
                            Pattern::Wildcard { .. } => self.emit(Op::Pop),
                            _ => return Err(CodegenError::Unsupported("nested enum destructuring pattern".into())),
                        }
                    }
                    Some(failure)
                }
                Pattern::Or { .. } => return Err(CodegenError::Unsupported("or-pattern bytecode".into())),
                Pattern::Tuple { .. } | Pattern::Struct { .. } => return Err(CodegenError::Unsupported("destructuring pattern bytecode".into())),
            };
            let guard_failure = if let Some(guard) = &arm.guard { self.compile_expr(guard)?; Some(self.jump_if_false()) } else { None };
            self.compile_block(&arm.body, true)?; ends.push(self.jump());
            let next = self.position();
            if let Some(jump) = failure { self.patch(jump, next); }
            if let Some(jump) = guard_failure { self.patch(jump, next); }
            self.pop_scope();
        }
        self.emit(Op::PushNil); let end = self.position(); for jump in ends { self.patch(jump, end); } Ok(())
    }

    fn compile_template(&mut self, template: &str) -> Result<(), CodegenError> {
        let mut rest = template;
        let mut has_output = false;
        while let Some(open) = rest.find('{') {
            let literal = &rest[..open];
            if !literal.is_empty() {
                let id = self.intern(literal); self.emit(Op::PushStr(id));
                if has_output { self.emit(Op::Add); } else { has_output = true; }
            }
            let after = &rest[open + 1..];
            let close = after.find('}').ok_or_else(|| CodegenError::InvalidInterpolation(template.into()))?;
            self.compile_interpolation(after[..close].trim())?; self.emit(Op::ToString);
            if has_output { self.emit(Op::Add); } else { has_output = true; }
            rest = &after[close + 1..];
        }
        if !rest.is_empty() {
            let id = self.intern(rest); self.emit(Op::PushStr(id));
            if has_output { self.emit(Op::Add); } else { has_output = true; }
        }
        if !has_output { let id = self.intern(""); self.emit(Op::PushStr(id)); }
        Ok(())
    }

    fn compile_interpolation(&mut self, source: &str) -> Result<(), CodegenError> {
        if let Some(open) = source.find('(') {
            if !source.ends_with(')') { return Err(CodegenError::InvalidInterpolation(source.into())); }
            let name = source[..open].trim(); let args_source = &source[open + 1..source.len() - 1];
            let mut argc = 0;
            for arg in args_source.split(',').map(str::trim).filter(|x| !x.is_empty()) {
                if let Ok(value) = arg.parse::<i64>() { self.emit(Op::PushInt(value)); }
                else { let local = self.find_local(arg).ok_or_else(|| CodegenError::UnknownVariable(arg.into()))?; self.emit(Op::PushLocal(local)); }
                argc += 1;
            }
            let function = self.function_ids.get(name).copied().ok_or_else(|| CodegenError::UnknownFunction(name.into()))?;
            self.emit(Op::Call { function, argc });
        } else {
            let local = self.find_local(source).ok_or_else(|| CodegenError::UnknownVariable(source.into()))?; self.emit(Op::PushLocal(local));
        }
        Ok(())
    }

    fn compile_closure(&mut self, params: &[Param], body: &Expr) -> Result<(), CodegenError> {
        let mut visible = std::collections::BTreeMap::new();
        for scope in &self.locals { for (name, slot) in scope { visible.insert(name.clone(), *slot); } }
        let captures: Vec<_> = visible.into_iter().collect();
        let function = self.module.functions.len();
        self.module.functions.push(empty_function());

        let outer_function = std::mem::replace(&mut self.current, empty_function());
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_next = self.next_local;
        let outer_loops = std::mem::take(&mut self.loops);

        self.current = BytecodeFunc { name: format!("<closure:{function}>"), source_file: outer_function.source_file.clone(), arity: params.len(), captures: captures.len(), locals: 0, max_stack: 256, code: Vec::new(), debug_locations: Vec::new() };
        self.locals = vec![HashMap::new()]; self.next_local = 0;
        for (name, _) in &captures { self.add_local(name); }
        for param in params { self.add_local(&param.name); }
        let result = self.compile_expr(body);
        if result.is_ok() { self.emit(Op::Ret); self.current.locals = self.next_local; self.module.functions[function] = self.current.clone(); }

        self.current = outer_function; self.locals = outer_locals; self.next_local = outer_next; self.loops = outer_loops;
        result?;
        self.emit(Op::MakeClosure { function, captures: captures.into_iter().map(|(_, slot)| slot).collect() });
        Ok(())
    }

    fn finish_loop(&mut self, end: usize) {
        if let Some(context) = self.loops.pop() {
            for jump in context.breaks { self.patch(jump, end); }
            for jump in context.continues { self.patch(jump, context.continue_target); }
        }
    }
    fn emit(&mut self, op: Op) { self.current.code.push(op); self.current.debug_locations.push(self.current_location); }
    fn position(&self) -> usize { self.current.code.len() }
    fn jump(&mut self) -> usize { let p = self.position(); self.emit(Op::Jump(usize::MAX)); p }
    fn jump_if_false(&mut self) -> usize { let p = self.position(); self.emit(Op::JumpIfFalse(usize::MAX)); p }
    fn patch(&mut self, at: usize, target: usize) { match &mut self.current.code[at] { Op::Jump(to) | Op::JumpIfFalse(to) => *to = target, _ => {} } }
    fn intern(&mut self, value: &str) -> usize {
        if let Some(id) = self.strings.get(value) { *id } else { let id = self.module.string_table.len(); self.module.string_table.push(value.into()); self.strings.insert(value.into(), id); id }
    }
    fn push_scope(&mut self) { self.locals.push(HashMap::new()); }
    fn pop_scope(&mut self) { self.locals.pop(); }
    fn add_local(&mut self, name: &str) -> usize { let id = self.next_local; self.next_local += 1; self.locals.last_mut().unwrap().insert(name.into(), id); id }
    fn add_temp(&mut self, prefix: &str) -> usize { let name = format!("{prefix}{}", self.next_local); self.add_local(&name) }
    fn find_local(&self, name: &str) -> Option<usize> { self.locals.iter().rev().find_map(|scope| scope.get(name).copied()) }
}

fn empty_function() -> BytecodeFunc { BytecodeFunc { name: String::new(), source_file: None, arity: 0, captures: 0, locals: 0, max_stack: 0, code: Vec::new(), debug_locations: Vec::new() } }
fn is_terminal(expr: &Expr) -> bool { matches!(expr, Expr::Return { .. } | Expr::Break { .. } | Expr::Continue { .. }) }
fn binary_instruction(op: BinaryOp) -> Result<Op, CodegenError> {
    Ok(match op { BinaryOp::Add => Op::Add, BinaryOp::Sub => Op::Sub, BinaryOp::Mul => Op::Mul, BinaryOp::Div => Op::Div, BinaryOp::Mod => Op::Mod, _ => return Err(CodegenError::Unsupported(format!("compound assignment {op:?}"))) })
}
fn collect_items<'a>(items: &'a [Item], functions: &mut Vec<&'a FunctionDecl>, constants: &mut HashMap<String, Expr>, variants: &mut HashMap<String, bool>) {
    for item in items {
        match item {
            Item::Function(f) => functions.push(f),
            Item::Const(c) => { constants.insert(c.name.clone(), (*c.value).clone()); }
            Item::Enum(e) => for variant in &e.variants { variants.insert(format!("{}::{}", e.name, variant.name), variant.payload.is_some()); },
            Item::Module(m) => collect_items(&m.items, functions, constants, variants),
            Item::Impl(i) => functions.extend(i.methods.iter()), _ => {}
        }
    }
}
fn split_variant(name: &str) -> Result<(&str, &str), CodegenError> {
    name.split_once("::").ok_or_else(|| CodegenError::Unsupported(format!("invalid enum variant '{name}'")))
}

impl Default for AstCompiler { fn default() -> Self { Self::new() } }
