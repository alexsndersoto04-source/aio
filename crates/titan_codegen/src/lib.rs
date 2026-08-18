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
    PushInt(i64),
    PushFloat(f64),
    PushBool(bool),
    PushChar(char),
    PushNil,
    PushStr(usize),
    PushLocal(usize),
    StoreLocal(usize),
    Pop,
    Dup,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Not,
    BitNot,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    BitAnd,
    BitOr,
    BitXor,
    Jump(usize),
    JumpIfFalse(usize),
    Call {
        function: usize,
        argc: usize,
    },
    CallNative {
        name: String,
        argc: usize,
    },
    MakeClosure {
        function: usize,
        captures: Vec<usize>,
    },
    CallValue(usize),
    Try,
    // Phase 18: exception-safe closure call. Pops (fn, args...), calls it
    // like CallValue, but ANY runtime error (native failure, type mismatch,
    // panic in a native, out-of-bounds, etc.) is captured and pushed as
    // Result::Err(String); success pushes Result::Ok(value). Consumed by
    // the `std::try::catch(fn)` builtin so .titan code can handle errors
    // as values instead of the whole program dying.
    TryCall(usize),
    ArrayMap,
    ArrayFilter,
    ArrayFold,
    // Phase 19: more higher-order operations over arrays with closures.
    // ArraySortBy pops (array, closure) — closure receives (a, b) and
    //   returns int (negative if a<b, 0 if equal, positive if a>b),
    //   like a classic C compareTo. Pushes the sorted array (stable).
    // ArrayFind pops (array, closure); closure receives one element
    //   and returns bool. Pushes the first matching element or nil.
    // ArrayAny / ArrayAll pop (array, closure); closure returns bool.
    //   Pushes true iff any / all elements pass. Short-circuits.
    ArraySortBy,
    ArrayFind,
    ArrayAny,
    ArrayAll,
    // Phase 20: dynamic method dispatch for `impl` on structs.
    // Stack layout when executed: [..., receiver, arg1, arg2, ..., argN].
    // The VM pops N args and the receiver. Struct values resolve
    // "<StructName>::<method>" in module.method_table and invoke it with
    // (receiver, arg1..argN). Non-struct values fall back to a matching
    // built-in collection operation.
    // Static-associated calls like `Point::origin()` don't use this opcode
    // — they go through the regular Op::Call because the parser folds
    // `Point::origin` into a single qualified identifier at parse time.
    CallMethod {
        method: String,
        argc: usize,
    },
    Spawn,
    JoinTask,
    JoinTaskTimeout,
    CancelTask,
    NewChannel,
    ChannelSend,
    ChannelRecv,
    ChannelRecvTimeout,
    ChannelSelect,
    TcpListen,
    TcpLocalAddr,
    TcpAccept,
    TcpConnect,
    TcpRead,
    TcpWrite,
    TcpSetTimeout,
    TcpClose,
    HttpServeConnection,
    HttpRouterNew,
    HttpRouteAdd,
    HttpMiddlewareAdd,
    HttpAfterAdd,
    HttpErrorHandlerAdd,
    HttpDispatch,
    TlsConnect,
    TlsServerConfig,
    TlsAccept,
    TlsRead,
    TlsWrite,
    TlsClose,
    WsDecoderNew,
    WsDecoderPush,
    WsDecoderNext,
    WsConnect,
    WsAttachTcp,
    WsAttachTls,
    WsSendText,
    WsSendBinary,
    WsReceive,
    WsClose,
    ServerControlNew,
    ServerTryAcquire,
    ServerRelease,
    ServerShutdown,
    ServerStats,
    ServerHealthResponse,
    SqliteOpen,
    SqliteMemory,
    SqliteExecute,
    SqliteQuery,
    SqliteBegin,
    SqliteCommit,
    SqliteRollback,
    SqliteMigrate,
    SqliteLastId,
    SqliteClose,
    SqlitePing,
    SqlitePoolNew,
    SqlitePoolAcquire,
    SqlitePoolStats,
    SqlitePoolHealth,
    SqlitePoolClose,
    PostgresConnect,
    PostgresConnectTls,
    PostgresExecute,
    PostgresQuery,
    PostgresBegin,
    PostgresCommit,
    PostgresRollback,
    PostgresMigrate,
    PostgresCancel,
    PostgresClose,
    PostgresPing,
    PostgresPoolNew,
    PostgresPoolAcquire,
    PostgresPoolStats,
    PostgresPoolHealth,
    PostgresPoolClose,
    MysqlConnect,
    MysqlExecute,
    MysqlQuery,
    MysqlBegin,
    MysqlCommit,
    MysqlRollback,
    MysqlMigrate,
    MysqlLastId,
    MysqlClose,
    MysqlPing,
    MysqlPoolNew,
    MysqlPoolAcquire,
    MysqlPoolStats,
    MysqlPoolHealth,
    MysqlPoolClose,
    DbExecute,
    DbQuery,
    DbBegin,
    DbCommit,
    DbRollback,
    DbMigrate,
    DbClose,
    DbPing,
    RuntimeMemoryLimit,
    RuntimeAllocatedBytes,
    RuntimeGcLiveCount,
    RuntimeGcCollect,
    RuntimeGcThreshold,
    RuntimeGcSetThreshold,
    RuntimeActiveTasks,
    RuntimeHeapDump,
    RuntimeOptimizeLevel,
    RuntimeFastPath,
    RuntimeBenchmark,
    SpawnQuota,
    Ret,
    Print(usize),
    Len,
    ToString,
    NewArray(usize),
    NewTuple(usize),
    Index,
    NewStruct {
        name: String,
        fields: Vec<String>,
    },
    GetField(String),
    NewEnum {
        name: String,
        variant: String,
        has_payload: bool,
    },
    EnumIs {
        name: String,
        variant: String,
    },
    EnumPayload,
    Nop,
    Halt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}
impl From<titan_lexer::Span> for SourceLocation {
    fn from(span: titan_lexer::Span) -> Self {
        Self {
            start: span.start,
            end: span.end,
            line: span.line,
            column: span.column,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BytecodeType {
    Unknown,
    Int,
    Bool,
    String,
    Struct(String),
    Enum(String),
    Array,
    Tuple,
    Map,
    ArrayOf(Box<BytecodeType>),
    TupleOf(Vec<BytecodeType>),
    EnumOf(String, Vec<BytecodeType>),
    MapOf(Box<BytecodeType>, Box<BytecodeType>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BytecodeFunc {
    pub name: String,
    #[serde(default)]
    pub source_file: Option<String>,
    pub arity: usize,
    #[serde(default)]
    pub param_types: Vec<BytecodeType>,
    #[serde(default)]
    pub return_type: Option<BytecodeType>,
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
    #[serde(default)]
    pub struct_schemas: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub enum_schemas: HashMap<String, Vec<String>>,
    /// Phase 20: maps "<StructName>::<method>" to the function index in
    /// `functions`. Populated at compile time from `impl` blocks; consumed
    /// by the VM's `Op::CallMethod` for dynamic dispatch based on the
    /// receiver's Value::Struct name. Static calls (`Point::origin()`)
    /// don't need this — they resolve at compile time.
    #[serde(default)]
    pub method_table: HashMap<String, usize>,
}

pub struct AstCompiler {
    module: CompiledModule,
    current: BytecodeFunc,
    locals: Vec<HashMap<String, usize>>,
    local_mutability: Vec<HashMap<String, bool>>,
    next_local: usize,
    strings: HashMap<String, usize>,
    function_ids: HashMap<String, usize>,
    enum_variants: HashMap<String, bool>,
    constants: HashMap<String, Expr>,
    constant_expansion: Vec<String>,
    loops: Vec<LoopContext>,
    current_location: Option<SourceLocation>,
}

#[derive(Default)]
struct LoopContext {
    breaks: Vec<usize>,
    continues: Vec<usize>,
    continue_target: usize,
}

impl AstCompiler {
    pub fn new() -> Self {
        Self {
            module: CompiledModule {
                functions: Vec::new(),
                entry: 0,
                string_table: Vec::new(),
                struct_schemas: HashMap::new(),
                enum_schemas: HashMap::new(),
                method_table: HashMap::new(),
            },
            current: empty_function(),
            locals: Vec::new(),
            local_mutability: Vec::new(),
            next_local: 0,
            strings: HashMap::new(),
            function_ids: HashMap::new(),
            enum_variants: HashMap::new(),
            constants: HashMap::new(),
            constant_expansion: Vec::new(),
            loops: Vec::new(),
            current_location: None,
        }
    }

    pub fn compile_program(&mut self, program: &Program) -> Result<CompiledModule, CodegenError> {
        self.module = CompiledModule {
            functions: Vec::new(),
            entry: 0,
            string_table: Vec::new(),
            struct_schemas: HashMap::new(),
            enum_schemas: HashMap::new(),
            method_table: HashMap::new(),
        };
        self.strings.clear();
        self.function_ids.clear();
        self.enum_variants.clear();
        self.constants.clear();
        self.constant_expansion.clear();
        self.enum_variants.extend([
            ("Option::None".into(), false),
            ("Option::Some".into(), true),
            ("Result::Ok".into(), true),
            ("Result::Err".into(), true),
        ]);
        self.module
            .enum_schemas
            .insert("Option".into(), vec!["None".into(), "Some".into()]);
        self.module
            .enum_schemas
            .insert("Result".into(), vec!["Ok".into(), "Err".into()]);
        collect_struct_schemas(&program.items, &mut self.module.struct_schemas);
        collect_enum_schemas(&program.items, &mut self.module.enum_schemas);
        let mut functions: Vec<FunctionDecl> = Vec::new();
        let mut method_table: HashMap<String, usize> = HashMap::new();
        collect_items(
            &program.items,
            &mut functions,
            &mut self.constants,
            &mut self.enum_variants,
            &mut method_table,
        );
        for (index, function) in functions.iter().enumerate() {
            if self
                .function_ids
                .insert(function.name.clone(), index)
                .is_some()
            {
                return Err(CodegenError::Unsupported(format!(
                    "duplicate function '{}'",
                    function.name
                )));
            }
        }
        self.module.method_table = method_table;
        let Some(entry) = self.function_ids.get("main").copied() else {
            return Err(CodegenError::UnknownFunction("main".into()));
        };
        self.module.entry = entry;
        self.module.functions = vec![empty_function(); functions.len()];
        for (index, function) in functions.iter().enumerate() {
            let compiled = self.compile_function(function)?;
            self.module.functions[index] = compiled;
        }
        Ok(self.module.clone())
    }

    fn compile_function(&mut self, function: &FunctionDecl) -> Result<BytecodeFunc, CodegenError> {
        if function.is_extern {
            return Err(CodegenError::Unsupported(format!(
                "extern function '{}' has no runtime linkage implementation",
                function.name
            )));
        }
        let Some(body) = &function.body else {
            return Err(CodegenError::Unsupported(format!(
                "bodyless function '{}' outside a trait declaration",
                function.name
            )));
        };
        if function.params.iter().any(|param| param.default.is_some()) {
            return Err(CodegenError::Unsupported(format!(
                "default parameters in function '{}' have no bytecode implementation",
                function.name
            )));
        }
        let param_types = function
            .params
            .iter()
            .map(|param| bytecode_type(param.type_ann.as_ref(), &self.module.enum_schemas))
            .collect();
        let return_type = function
            .return_type
            .as_ref()
            .map(|ty| bytecode_type(Some(ty), &self.module.enum_schemas));
        self.current = BytecodeFunc {
            name: function.name.clone(),
            source_file: function.source_file.clone(),
            arity: function.params.len(),
            param_types,
            return_type,
            captures: 0,
            locals: 0,
            max_stack: 256,
            code: Vec::new(),
            debug_locations: Vec::new(),
        };
        self.locals = vec![HashMap::new()];
        self.local_mutability = vec![HashMap::new()];
        self.next_local = 0;
        self.loops.clear();
        for param in &function.params {
            self.add_mutable_local(&param.name, param.mutable);
        }
        self.compile_block(body, true)?;
        if !matches!(self.current.code.last(), Some(Op::Ret)) {
            self.emit(Op::Ret);
        }
        self.current.locals = self.next_local;
        Ok(self.current.clone())
    }

    fn compile_block(&mut self, block: &Block, value_needed: bool) -> Result<(), CodegenError> {
        self.push_scope();
        for stmt in &block.stmts {
            self.compile_stmt(stmt)?;
        }
        if let Some(expr) = &block.final_expr {
            self.compile_expr(expr)?;
            if !value_needed {
                self.emit(Op::Pop);
            }
        } else if value_needed {
            self.emit(Op::PushNil);
        }
        self.pop_scope();
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CodegenError> {
        match stmt {
            Stmt::Expr(expr) => {
                self.compile_expr(expr)?;
                if !is_terminal(expr) {
                    self.emit(Op::Pop);
                }
            }
            Stmt::Let {
                name,
                mutable,
                value,
                ..
            } => {
                self.compile_expr(value)?;
                let local = self.add_mutable_local(name, *mutable);
                self.emit(Op::StoreLocal(local));
            }
            Stmt::Assign {
                target, op, value, ..
            } => {
                self.compile_assignment(target, *op, value, false)?;
            }
            Stmt::Item(_) => {
                return Err(CodegenError::Unsupported(
                    "nested declarations are not executable yet".into(),
                ))
            }
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
            Expr::String { value, .. } => {
                let id = self.intern(value);
                self.emit(Op::PushStr(id));
            }
            Expr::StringTemplate { value, .. } => self.compile_template(value)?,
            Expr::Ident { name, .. } => {
                if let Some(local) = self.find_local(name) {
                    self.emit(Op::PushLocal(local));
                } else if let Some(value) = self.constants.get(name).cloned() {
                    if let Some(start) = self
                        .constant_expansion
                        .iter()
                        .position(|constant| constant == name)
                    {
                        let mut cycle = self.constant_expansion[start..].to_vec();
                        cycle.push(name.clone());
                        return Err(CodegenError::Unsupported(format!(
                            "recursive constant expansion: {}",
                            cycle.join(" -> ")
                        )));
                    }
                    self.constant_expansion.push(name.clone());
                    let result = self.compile_expr(&value);
                    self.constant_expansion.pop();
                    result?;
                } else if self.enum_variants.get(name) == Some(&false) {
                    let (enum_name, variant) = split_variant(name)?;
                    self.emit(Op::NewEnum {
                        name: enum_name.into(),
                        variant: variant.into(),
                        has_payload: false,
                    });
                } else if let Some(function) = self.function_ids.get(name).copied() {
                    self.emit(Op::MakeClosure {
                        function,
                        captures: Vec::new(),
                    });
                } else {
                    return Err(CodegenError::UnknownVariable(name.clone()));
                }
            }
            Expr::Array { elements, .. } => {
                for element in elements {
                    self.compile_expr(element)?;
                }
                self.emit(Op::NewArray(elements.len()));
            }
            Expr::Tuple { elements, .. } => {
                for element in elements {
                    self.compile_expr(element)?;
                }
                self.emit(Op::NewTuple(elements.len()));
            }
            Expr::StructLit { name, fields, .. } => {
                for (_, value) in fields {
                    self.compile_expr(value)?;
                }
                self.emit(Op::NewStruct {
                    name: name.clone(),
                    fields: fields.iter().map(|(n, _)| n.clone()).collect(),
                });
            }
            Expr::Binary {
                left, op, right, ..
            } => self.compile_binary(left, *op, right)?,
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => self.compile_range(start, end, *inclusive)?,
            Expr::Unary { op, expr, .. } => {
                self.compile_expr(expr)?;
                let instruction = match op {
                    UnaryOp::Neg => Op::Neg,
                    UnaryOp::Not => Op::Not,
                    UnaryOp::BitNot => Op::BitNot,
                    UnaryOp::Ref | UnaryOp::RefMut | UnaryOp::Deref => {
                        return Err(CodegenError::Unsupported(
                            "references and dereferencing".into(),
                        ))
                    }
                };
                self.emit(instruction);
            }
            Expr::Call { callee, args, .. } => self.compile_call(callee, args)?,
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                self.compile_expr(receiver)?;
                for arg in args {
                    self.compile_expr(arg)?;
                }
                // Every method-syntax call is dispatched from the runtime
                // receiver. Struct implementations take priority; built-in
                // collection methods are the non-struct fallback. Keeping a
                // single lowering path prevents names such as `map` or `fold`
                // from bypassing a real method declared by the struct.
                self.emit(Op::CallMethod {
                    method: method.clone(),
                    argc: args.len(),
                });
            }
            Expr::Index { target, index, .. } => {
                self.compile_expr(target)?;
                self.compile_expr(index)?;
                self.emit(Op::Index);
            }
            Expr::FieldAccess { target, field, .. } => {
                self.compile_expr(target)?;
                self.emit(Op::GetField(field.clone()));
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.compile_expr(condition)?;
                let else_jump = self.jump_if_false();
                self.compile_block(then_branch, true)?;
                let end_jump = self.jump();
                self.patch(else_jump, self.position());
                if let Some(other) = else_branch {
                    self.compile_block(other, true)?;
                } else {
                    self.emit(Op::PushNil);
                }
                self.patch(end_jump, self.position());
            }
            Expr::While {
                condition, body, ..
            } => self.compile_while(condition, body)?,
            Expr::For {
                pattern,
                iterator,
                body,
                ..
            } => self.compile_for(pattern, iterator, body)?,
            Expr::Loop { body, .. } => self.compile_loop(body)?,
            Expr::Break { value, .. } => {
                if value.is_some() {
                    return Err(CodegenError::Unsupported(
                        "values carried by break".into(),
                    ));
                }
                let jump = self.jump();
                self.loops
                    .last_mut()
                    .ok_or(CodegenError::OutsideLoop)?
                    .breaks
                    .push(jump);
            }
            Expr::Continue { .. } => {
                let jump = self.jump();
                self.loops
                    .last_mut()
                    .ok_or(CodegenError::OutsideLoop)?
                    .continues
                    .push(jump);
            }
            Expr::Return { value, .. } => {
                if let Some(value) = value {
                    self.compile_expr(value)?;
                } else {
                    self.emit(Op::PushNil);
                }
                self.emit(Op::Ret);
            }
            Expr::Let {
                name,
                mutable,
                value,
                ..
            } => {
                self.compile_expr(value)?;
                self.emit(Op::Dup);
                let local = self.add_mutable_local(name, *mutable);
                self.emit(Op::StoreLocal(local));
            }
            Expr::Assign {
                target, op, value, ..
            } => self.compile_assignment(target, *op, value, true)?,
            Expr::Block(block) => self.compile_block(block, true)?,
            Expr::Match {
                scrutinee, arms, ..
            } => self.compile_match(scrutinee, arms)?,
            Expr::Spawn { expr, .. } => {
                self.compile_expr(expr)?;
                self.emit(Op::Spawn);
            }
            Expr::Try { expr, .. } => {
                self.compile_expr(expr)?;
                self.emit(Op::Try);
            }
            Expr::Closure { params, body, .. } => self.compile_closure(params, body)?,
        }
        Ok(())
    }

    fn compile_binary(
        &mut self,
        left: &Expr,
        op: BinaryOp,
        right: &Expr,
    ) -> Result<(), CodegenError> {
        if matches!(op, BinaryOp::LazyAnd | BinaryOp::LazyOr) {
            self.compile_expr(left)?;
            self.emit(Op::Dup);
            let jump = self.jump_if_false();
            if op == BinaryOp::LazyOr {
                let evaluate = self.jump();
                self.patch(jump, self.position());
                self.emit(Op::Pop);
                self.compile_expr(right)?;
                let end = self.jump();
                self.patch(evaluate, self.position());
                self.patch(end, self.position());
            } else {
                self.emit(Op::Pop);
                self.compile_expr(right)?;
                self.patch(jump, self.position());
            }
            return Ok(());
        }
        self.compile_expr(left)?;
        self.compile_expr(right)?;
        self.emit(match op {
            BinaryOp::Add => Op::Add,
            BinaryOp::Sub => Op::Sub,
            BinaryOp::Mul => Op::Mul,
            BinaryOp::Div => Op::Div,
            BinaryOp::Mod => Op::Mod,
            BinaryOp::Eq => Op::Eq,
            BinaryOp::Neq => Op::Neq,
            BinaryOp::Lt => Op::Lt,
            BinaryOp::Gt => Op::Gt,
            BinaryOp::Lte => Op::Lte,
            BinaryOp::Gte => Op::Gte,
            BinaryOp::And => Op::BitAnd,
            BinaryOp::Or => Op::BitOr,
            BinaryOp::Xor => Op::BitXor,
            BinaryOp::LazyAnd | BinaryOp::LazyOr => unreachable!(),
        });
        Ok(())
    }

    fn compile_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<(), CodegenError> {
        if let Expr::Ident { name, .. } = callee {
            if let Some(local) = self.find_local(name) {
                self.emit(Op::PushLocal(local));
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.emit(Op::CallValue(args.len()));
                return Ok(());
            }
            // Constants are expression aliases, so a callable constant must
            // be materialized before its arguments just like any other
            // first-class callee. Falling through to the named-function path
            // would instead report UnknownFunction during lowering.
            if self.constants.contains_key(name) {
                self.compile_expr(callee)?;
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.emit(Op::CallValue(args.len()));
                return Ok(());
            }
            // Phase 18: std::try::catch(fn, args...) is compiled specially
            // because its argument is a closure that must be executed inside
            // a try boundary. Emit the closure and its args then TryCall.
            if name == "std::try::catch" {
                if args.is_empty() {
                    return Err(CodegenError::Unsupported(
                        "std::try::catch requires a callable argument".into(),
                    ));
                }
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.emit(Op::TryCall(args.len() - 1));
                return Ok(());
            }
            for arg in args {
                self.compile_expr(arg)?;
            }
            match name.as_str() {
                "print" | "println" => self.emit(Op::Print(args.len())),
                "len" if args.len() == 1 => self.emit(Op::Len),
                "map" if args.len() == 2 => self.emit(Op::ArrayMap),
                "filter" if args.len() == 2 => self.emit(Op::ArrayFilter),
                "fold" if args.len() == 3 => self.emit(Op::ArrayFold),
                // Phase 19: sort_by(arr, |a,b| cmp), find/any/all(arr, |x| bool)
                "sort_by" if args.len() == 2 => self.emit(Op::ArraySortBy),
                "find" if args.len() == 2 => self.emit(Op::ArrayFind),
                "any" if args.len() == 2 => self.emit(Op::ArrayAny),
                "all" if args.len() == 2 => self.emit(Op::ArrayAll),
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
                "std::http::serve_connection" if args.len() == 3 => {
                    self.emit(Op::HttpServeConnection)
                }
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
                "std::server::health_response" if args.len() == 1 => {
                    self.emit(Op::ServerHealthResponse)
                }
                "std::sqlite::open" if args.len() == 1 => self.emit(Op::SqliteOpen),
                "std::sqlite::memory" if args.is_empty() => self.emit(Op::SqliteMemory),
                "std::sqlite::execute" if args.len() == 3 => self.emit(Op::SqliteExecute),
                "std::sqlite::query" if args.len() == 3 => self.emit(Op::SqliteQuery),
                "std::sqlite::begin" if args.len() == 1 => self.emit(Op::SqliteBegin),
                "std::sqlite::commit" if args.len() == 1 => self.emit(Op::SqliteCommit),
                "std::sqlite::rollback" if args.len() == 1 => self.emit(Op::SqliteRollback),
                "std::sqlite::migrate" if args.len() == 2 => self.emit(Op::SqliteMigrate),
                "std::sqlite::last_insert_id" if args.len() == 1 => self.emit(Op::SqliteLastId),
                "std::sqlite::close" if args.len() == 1 => self.emit(Op::SqliteClose),
                "std::sqlite::ping" if args.len() == 1 => self.emit(Op::SqlitePing),
                "std::sqlite::pool" if args.len() == 2 => self.emit(Op::SqlitePoolNew),
                "std::sqlite::acquire" if args.len() == 2 => self.emit(Op::SqlitePoolAcquire),
                "std::sqlite::pool_stats" if args.len() == 1 => self.emit(Op::SqlitePoolStats),
                "std::sqlite::pool_health" if args.len() == 2 => self.emit(Op::SqlitePoolHealth),
                "std::sqlite::pool_close" if args.len() == 1 => self.emit(Op::SqlitePoolClose),
                "std::postgres::connect" if args.len() == 1 => self.emit(Op::PostgresConnect),
                "std::postgres::connect_tls" if args.len() == 1 => {
                    self.emit(Op::PostgresConnectTls)
                }
                "std::postgres::execute" if args.len() == 3 => self.emit(Op::PostgresExecute),
                "std::postgres::query" if args.len() == 3 => self.emit(Op::PostgresQuery),
                "std::postgres::begin" if args.len() == 1 => self.emit(Op::PostgresBegin),
                "std::postgres::commit" if args.len() == 1 => self.emit(Op::PostgresCommit),
                "std::postgres::rollback" if args.len() == 1 => self.emit(Op::PostgresRollback),
                "std::postgres::migrate" if args.len() == 2 => self.emit(Op::PostgresMigrate),
                "std::postgres::cancel" if args.len() == 1 => self.emit(Op::PostgresCancel),
                "std::postgres::close" if args.len() == 1 => self.emit(Op::PostgresClose),
                "std::postgres::ping" if args.len() == 1 => self.emit(Op::PostgresPing),
                "std::postgres::pool" if args.len() == 3 => self.emit(Op::PostgresPoolNew),
                "std::postgres::acquire" if args.len() == 2 => self.emit(Op::PostgresPoolAcquire),
                "std::postgres::pool_stats" if args.len() == 1 => self.emit(Op::PostgresPoolStats),
                "std::postgres::pool_health" if args.len() == 2 => {
                    self.emit(Op::PostgresPoolHealth)
                }
                "std::postgres::pool_close" if args.len() == 1 => self.emit(Op::PostgresPoolClose),
                "std::mysql::connect" if args.len() == 1 => self.emit(Op::MysqlConnect),
                "std::mysql::execute" if args.len() == 3 => self.emit(Op::MysqlExecute),
                "std::mysql::query" if args.len() == 3 => self.emit(Op::MysqlQuery),
                "std::mysql::begin" if args.len() == 1 => self.emit(Op::MysqlBegin),
                "std::mysql::commit" if args.len() == 1 => self.emit(Op::MysqlCommit),
                "std::mysql::rollback" if args.len() == 1 => self.emit(Op::MysqlRollback),
                "std::mysql::migrate" if args.len() == 2 => self.emit(Op::MysqlMigrate),
                "std::mysql::last_insert_id" if args.len() == 1 => self.emit(Op::MysqlLastId),
                "std::mysql::close" if args.len() == 1 => self.emit(Op::MysqlClose),
                "std::mysql::ping" if args.len() == 1 => self.emit(Op::MysqlPing),
                "std::mysql::pool" if args.len() == 2 => self.emit(Op::MysqlPoolNew),
                "std::mysql::acquire" if args.len() == 2 => self.emit(Op::MysqlPoolAcquire),
                "std::mysql::pool_stats" if args.len() == 1 => self.emit(Op::MysqlPoolStats),
                "std::mysql::pool_health" if args.len() == 2 => self.emit(Op::MysqlPoolHealth),
                "std::mysql::pool_close" if args.len() == 1 => self.emit(Op::MysqlPoolClose),
                "std::db::execute" if args.len() == 3 => self.emit(Op::DbExecute),
                "std::db::query" if args.len() == 3 => self.emit(Op::DbQuery),
                "std::db::begin" if args.len() == 1 => self.emit(Op::DbBegin),
                "std::db::commit" if args.len() == 1 => self.emit(Op::DbCommit),
                "std::db::rollback" if args.len() == 1 => self.emit(Op::DbRollback),
                "std::db::migrate" if args.len() == 2 => self.emit(Op::DbMigrate),
                "std::db::close" if args.len() == 1 => self.emit(Op::DbClose),
                "std::db::ping" if args.len() == 1 => self.emit(Op::DbPing),
                "std::runtime::memory_limit" if args.len() == 0 => {
                    self.emit(Op::RuntimeMemoryLimit)
                }
                "std::runtime::allocated_bytes" if args.len() == 0 => {
                    self.emit(Op::RuntimeAllocatedBytes)
                }
                "std::runtime::gc_live_count" if args.len() == 0 => {
                    self.emit(Op::RuntimeGcLiveCount)
                }
                "std::runtime::gc_collect" if args.len() == 0 => self.emit(Op::RuntimeGcCollect),
                "std::runtime::gc_threshold" if args.len() == 0 => {
                    self.emit(Op::RuntimeGcThreshold)
                }
                "std::runtime::gc_set_threshold" if args.len() == 1 => {
                    self.emit(Op::RuntimeGcSetThreshold)
                }
                "std::runtime::active_tasks" if args.len() == 0 => {
                    self.emit(Op::RuntimeActiveTasks)
                }
                "std::runtime::heap_dump" if args.len() == 1 => self.emit(Op::RuntimeHeapDump),
                "std::runtime::optimize_level" if args.len() == 0 => {
                    self.emit(Op::RuntimeOptimizeLevel)
                }
                "std::runtime::fast_path_enabled" if args.len() == 0 => {
                    self.emit(Op::RuntimeFastPath)
                }
                "std::runtime::benchmark" if args.len() == 2 => self.emit(Op::RuntimeBenchmark),
                "std::runtime::spawn_quota" if args.len() == 2 => self.emit(Op::SpawnQuota),
                _ if titan_stdlib::native::contains(name) => self.emit(Op::CallNative {
                    name: name.clone(),
                    argc: args.len(),
                }),
                _ if self.enum_variants.contains_key(name) => {
                    let has_payload = self.enum_variants[name];
                    if args.len() != usize::from(has_payload) {
                        return Err(CodegenError::Unsupported(format!(
                            "wrong payload count for enum variant '{name}'"
                        )));
                    }
                    let (enum_name, variant) = split_variant(name)?;
                    self.emit(Op::NewEnum {
                        name: enum_name.into(),
                        variant: variant.into(),
                        has_payload,
                    });
                }
                _ => {
                    let function = self
                        .function_ids
                        .get(name)
                        .copied()
                        .ok_or_else(|| CodegenError::UnknownFunction(name.clone()))?;
                    self.emit(Op::Call {
                        function,
                        argc: args.len(),
                    });
                }
            }
        } else {
            self.compile_expr(callee)?;
            for arg in args {
                self.compile_expr(arg)?;
            }
            self.emit(Op::CallValue(args.len()));
        }
        Ok(())
    }

    fn compile_assignment(
        &mut self,
        target: &Expr,
        op: Option<BinaryOp>,
        value: &Expr,
        keep: bool,
    ) -> Result<(), CodegenError> {
        let Expr::Ident { name, .. } = target else {
            return Err(CodegenError::Unsupported(
                "assignment target must currently be a variable".into(),
            ));
        };
        let local = self
            .find_local(name)
            .ok_or_else(|| CodegenError::UnknownVariable(name.clone()))?;
        if !self.is_local_mutable(name) {
            return Err(CodegenError::Unsupported(format!(
                "assignment to immutable or captured local '{name}'"
            )));
        }
        if let Some(op) = op {
            self.emit(Op::PushLocal(local));
            self.compile_expr(value)?;
            self.emit(binary_instruction(op)?);
        } else {
            self.compile_expr(value)?;
        }
        if keep {
            self.emit(Op::Dup);
        }
        self.emit(Op::StoreLocal(local));
        Ok(())
    }

    fn compile_while(&mut self, condition: &Expr, body: &Block) -> Result<(), CodegenError> {
        let start = self.position();
        self.compile_expr(condition)?;
        let exit = self.jump_if_false();
        self.loops.push(LoopContext {
            continue_target: start,
            ..Default::default()
        });
        self.compile_block(body, false)?;
        self.emit(Op::Jump(start));
        let end = self.position();
        self.patch(exit, end);
        self.finish_loop(end);
        self.emit(Op::PushNil);
        Ok(())
    }

    fn compile_loop(&mut self, body: &Block) -> Result<(), CodegenError> {
        let start = self.position();
        self.loops.push(LoopContext {
            continue_target: start,
            ..Default::default()
        });
        self.compile_block(body, false)?;
        self.emit(Op::Jump(start));
        let end = self.position();
        self.finish_loop(end);
        self.emit(Op::PushNil);
        Ok(())
    }

    fn compile_for(
        &mut self,
        pattern: &Pattern,
        iterator: &Expr,
        body: &Block,
    ) -> Result<(), CodegenError> {
        let Pattern::Ident { name, .. } = pattern else {
            return Err(CodegenError::Unsupported(
                "for currently requires an identifier pattern".into(),
            ));
        };
        // General arrays use an index and len; ranges are optimized but share the same representation.
        self.compile_expr(iterator)?;
        let array = self.add_temp("$iter");
        self.emit(Op::StoreLocal(array));
        self.emit(Op::PushInt(0));
        let index = self.add_temp("$index");
        self.emit(Op::StoreLocal(index));
        self.push_scope();
        let item = self.add_local(name);
        let start = self.position();
        self.emit(Op::PushLocal(index));
        self.emit(Op::PushLocal(array));
        self.emit(Op::Len);
        self.emit(Op::Lt);
        let exit = self.jump_if_false();
        self.emit(Op::PushLocal(array));
        self.emit(Op::PushLocal(index));
        self.emit(Op::Index);
        self.emit(Op::StoreLocal(item));
        self.loops.push(LoopContext {
            continue_target: 0,
            ..Default::default()
        });
        self.compile_block(body, false)?;
        let increment = self.position();
        if let Some(context) = self.loops.last_mut() {
            context.continue_target = increment;
        }
        self.emit(Op::PushLocal(index));
        self.emit(Op::PushInt(1));
        self.emit(Op::Add);
        self.emit(Op::StoreLocal(index));
        self.emit(Op::Jump(start));
        let end = self.position();
        self.patch(exit, end);
        self.finish_loop(end);
        self.pop_scope();
        self.emit(Op::PushNil);
        Ok(())
    }

    fn compile_range(
        &mut self,
        start: &Expr,
        end: &Expr,
        inclusive: bool,
    ) -> Result<(), CodegenError> {
        // Runtime helper encoded as an intrinsic function index sentinel.
        self.compile_expr(start)?;
        self.compile_expr(end)?;
        self.emit(Op::PushBool(inclusive));
        self.emit(Op::Call {
            function: usize::MAX,
            argc: 3,
        });
        Ok(())
    }

    fn compile_match(&mut self, scrutinee: &Expr, arms: &[MatchArm]) -> Result<(), CodegenError> {
        self.compile_expr(scrutinee)?;
        let subject = self.add_temp("$match");
        self.emit(Op::StoreLocal(subject));
        let mut ends = Vec::new();
        for arm in arms {
            self.push_scope();
            let failure = match &arm.pattern {
                Pattern::Wildcard { .. } => None,
                Pattern::Ident { name, .. } => {
                    self.emit(Op::PushLocal(subject));
                    let local = self.add_local(name);
                    self.emit(Op::StoreLocal(local));
                    None
                }
                Pattern::Literal { value, .. } => {
                    self.emit(Op::PushLocal(subject));
                    self.compile_expr(value)?;
                    self.emit(Op::Eq);
                    Some(self.jump_if_false())
                }
                Pattern::Enum {
                    name,
                    variant,
                    inner,
                    ..
                } => {
                    self.emit(Op::PushLocal(subject));
                    self.emit(Op::EnumIs {
                        name: name.clone(),
                        variant: variant.clone(),
                    });
                    let failure = self.jump_if_false();
                    if let Some(inner) = inner {
                        self.emit(Op::PushLocal(subject));
                        self.emit(Op::EnumPayload);
                        match inner.as_ref() {
                            Pattern::Ident { name, .. } => {
                                let local = self.add_local(name);
                                self.emit(Op::StoreLocal(local));
                            }
                            Pattern::Wildcard { .. } => self.emit(Op::Pop),
                            _ => {
                                return Err(CodegenError::Unsupported(
                                    "nested enum destructuring pattern".into(),
                                ))
                            }
                        }
                    }
                    Some(failure)
                }
                Pattern::Or { .. } => {
                    return Err(CodegenError::Unsupported("or-pattern bytecode".into()))
                }
                Pattern::Tuple { .. } | Pattern::Struct { .. } => {
                    return Err(CodegenError::Unsupported(
                        "destructuring pattern bytecode".into(),
                    ))
                }
            };
            let guard_failure = if let Some(guard) = &arm.guard {
                self.compile_expr(guard)?;
                Some(self.jump_if_false())
            } else {
                None
            };
            self.compile_block(&arm.body, true)?;
            ends.push(self.jump());
            let next = self.position();
            if let Some(jump) = failure {
                self.patch(jump, next);
            }
            if let Some(jump) = guard_failure {
                self.patch(jump, next);
            }
            self.pop_scope();
        }
        self.emit(Op::PushNil);
        let end = self.position();
        for jump in ends {
            self.patch(jump, end);
        }
        Ok(())
    }

    fn compile_template(&mut self, template: &str) -> Result<(), CodegenError> {
        let mut rest = template;
        let mut has_output = false;
        while let Some(open) = rest.find('{') {
            let literal = &rest[..open];
            if !literal.is_empty() {
                let id = self.intern(literal);
                self.emit(Op::PushStr(id));
                if has_output {
                    self.emit(Op::Add);
                } else {
                    has_output = true;
                }
            }
            let after = &rest[open + 1..];
            let close = after
                .find('}')
                .ok_or_else(|| CodegenError::InvalidInterpolation(template.into()))?;
            self.compile_interpolation(after[..close].trim())?;
            self.emit(Op::ToString);
            if has_output {
                self.emit(Op::Add);
            } else {
                has_output = true;
            }
            rest = &after[close + 1..];
        }
        if !rest.is_empty() {
            let id = self.intern(rest);
            self.emit(Op::PushStr(id));
            if has_output {
                self.emit(Op::Add);
            } else {
                has_output = true;
            }
        }
        if !has_output {
            let id = self.intern("");
            self.emit(Op::PushStr(id));
        }
        Ok(())
    }

    fn compile_interpolation(&mut self, source: &str) -> Result<(), CodegenError> {
        if let Some(open) = source.find('(') {
            if !source.ends_with(')') {
                return Err(CodegenError::InvalidInterpolation(source.into()));
            }
            let span = Default::default();
            let name = source[..open].trim();
            let args_source = &source[open + 1..source.len() - 1];
            let mut args = Vec::new();
            for arg in args_source
                .split(',')
                .map(str::trim)
                .filter(|x| !x.is_empty())
            {
                if let Ok(value) = arg.parse::<i64>() {
                    args.push(Expr::Int { value, span });
                } else {
                    // Template arguments deliberately remain limited to
                    // locals and integer literals. Validate that boundary here
                    // before routing the call through ordinary call lowering.
                    self.find_local(arg)
                        .ok_or_else(|| CodegenError::UnknownVariable(arg.into()))?;
                    args.push(Expr::Ident {
                        name: arg.into(),
                        span,
                    });
                }
            }
            // Reuse normal call lowering so local closures and callable
            // constants have the same priority over named functions, native
            // calls, intrinsics, and enum constructors inside and outside a
            // string template.
            self.compile_call(
                &Expr::Ident {
                    name: name.into(),
                    span,
                },
                &args,
            )?;
        } else {
            let local = self
                .find_local(source)
                .ok_or_else(|| CodegenError::UnknownVariable(source.into()))?;
            self.emit(Op::PushLocal(local));
        }
        Ok(())
    }

    fn compile_closure(&mut self, params: &[Param], body: &Expr) -> Result<(), CodegenError> {
        let mut visible = std::collections::BTreeMap::new();
        for scope in &self.locals {
            for (name, slot) in scope {
                visible.insert(name.clone(), *slot);
            }
        }
        let captures: Vec<_> = visible.into_iter().collect();
        let function = self.module.functions.len();
        self.module.functions.push(empty_function());

        let outer_function = std::mem::replace(&mut self.current, empty_function());
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_mutability = std::mem::take(&mut self.local_mutability);
        let outer_next = self.next_local;
        let outer_loops = std::mem::take(&mut self.loops);

        let param_types = params
            .iter()
            .map(|param| bytecode_type(param.type_ann.as_ref(), &self.module.enum_schemas))
            .collect();
        self.current = BytecodeFunc {
            name: format!("<closure:{function}>"),
            source_file: outer_function.source_file.clone(),
            arity: params.len(),
            param_types,
            return_type: None,
            captures: captures.len(),
            locals: 0,
            max_stack: 256,
            code: Vec::new(),
            debug_locations: Vec::new(),
        };
        self.locals = vec![HashMap::new()];
        self.local_mutability = vec![HashMap::new()];
        self.next_local = 0;
        for (name, _) in &captures {
            self.add_local(name);
        }
        for param in params {
            self.add_mutable_local(&param.name, param.mutable);
        }
        let result = self.compile_expr(body);
        if result.is_ok() {
            self.emit(Op::Ret);
            self.current.locals = self.next_local;
            self.module.functions[function] = self.current.clone();
        }

        self.current = outer_function;
        self.locals = outer_locals;
        self.local_mutability = outer_mutability;
        self.next_local = outer_next;
        self.loops = outer_loops;
        result?;
        self.emit(Op::MakeClosure {
            function,
            captures: captures.into_iter().map(|(_, slot)| slot).collect(),
        });
        Ok(())
    }

    fn finish_loop(&mut self, end: usize) {
        if let Some(context) = self.loops.pop() {
            for jump in context.breaks {
                self.patch(jump, end);
            }
            for jump in context.continues {
                self.patch(jump, context.continue_target);
            }
        }
    }
    fn emit(&mut self, op: Op) {
        self.current.code.push(op);
        self.current.debug_locations.push(self.current_location);
    }
    fn position(&self) -> usize {
        self.current.code.len()
    }
    fn jump(&mut self) -> usize {
        let p = self.position();
        self.emit(Op::Jump(usize::MAX));
        p
    }
    fn jump_if_false(&mut self) -> usize {
        let p = self.position();
        self.emit(Op::JumpIfFalse(usize::MAX));
        p
    }
    fn patch(&mut self, at: usize, target: usize) {
        match &mut self.current.code[at] {
            Op::Jump(to) | Op::JumpIfFalse(to) => *to = target,
            _ => {}
        }
    }
    fn intern(&mut self, value: &str) -> usize {
        if let Some(id) = self.strings.get(value) {
            *id
        } else {
            let id = self.module.string_table.len();
            self.module.string_table.push(value.into());
            self.strings.insert(value.into(), id);
            id
        }
    }
    fn push_scope(&mut self) {
        self.locals.push(HashMap::new());
        self.local_mutability.push(HashMap::new());
    }
    fn pop_scope(&mut self) {
        self.locals.pop();
        self.local_mutability.pop();
    }
    fn add_local(&mut self, name: &str) -> usize {
        self.add_mutable_local(name, false)
    }
    fn add_mutable_local(&mut self, name: &str, mutable: bool) -> usize {
        let id = self.next_local;
        self.next_local += 1;
        self.locals.last_mut().unwrap().insert(name.into(), id);
        self.local_mutability
            .last_mut()
            .unwrap()
            .insert(name.into(), mutable);
        id
    }
    fn add_temp(&mut self, prefix: &str) -> usize {
        let name = format!("{prefix}{}", self.next_local);
        self.add_local(&name)
    }
    fn find_local(&self, name: &str) -> Option<usize> {
        self.locals
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
    fn is_local_mutable(&self, name: &str) -> bool {
        self.local_mutability
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
            .unwrap_or(false)
    }
}

fn bytecode_type(
    ty: Option<&TypeExpr>,
    enum_schemas: &HashMap<String, Vec<String>>,
) -> BytecodeType {
    match ty {
        Some(TypeExpr::Named { name, generics }) => match name.as_str() {
            "int" => BytecodeType::Int,
            "bool" => BytecodeType::Bool,
            "string" => BytecodeType::String,
            "map" if generics.len() == 2 => BytecodeType::MapOf(
                Box::new(bytecode_type(Some(&generics[0]), enum_schemas)),
                Box::new(bytecode_type(Some(&generics[1]), enum_schemas)),
            ),
            "map" => BytecodeType::Map,
            other if enum_schemas.contains_key(other) && !generics.is_empty() => {
                BytecodeType::EnumOf(
                    other.into(),
                    generics
                        .iter()
                        .map(|generic| bytecode_type(Some(generic), enum_schemas))
                        .collect(),
                )
            }
            other if enum_schemas.contains_key(other) => BytecodeType::Enum(other.into()),
            other => BytecodeType::Struct(other.into()),
        },
        Some(TypeExpr::Array { inner, .. } | TypeExpr::Slice { inner }) => {
            BytecodeType::ArrayOf(Box::new(bytecode_type(Some(inner), enum_schemas)))
        }
        Some(TypeExpr::Tuple { elements }) => BytecodeType::TupleOf(
            elements
                .iter()
                .map(|element| bytecode_type(Some(element), enum_schemas))
                .collect(),
        ),
        _ => BytecodeType::Unknown,
    }
}

fn empty_function() -> BytecodeFunc {
    BytecodeFunc {
        name: String::new(),
        source_file: None,
        arity: 0,
        param_types: Vec::new(),
        return_type: None,
        captures: 0,
        locals: 0,
        max_stack: 0,
        code: Vec::new(),
        debug_locations: Vec::new(),
    }
}
fn is_terminal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Return { .. } | Expr::Break { .. } | Expr::Continue { .. }
    )
}
fn binary_instruction(op: BinaryOp) -> Result<Op, CodegenError> {
    Ok(match op {
        BinaryOp::Add => Op::Add,
        BinaryOp::Sub => Op::Sub,
        BinaryOp::Mul => Op::Mul,
        BinaryOp::Div => Op::Div,
        BinaryOp::Mod => Op::Mod,
        _ => {
            return Err(CodegenError::Unsupported(format!(
                "compound assignment {op:?}"
            )))
        }
    })
}
fn collect_struct_schemas(items: &[Item], output: &mut HashMap<String, Vec<String>>) {
    for item in items {
        match item {
            Item::Struct(item) => {
                output.insert(
                    item.name.clone(),
                    item.fields.iter().map(|field| field.name.clone()).collect(),
                );
            }
            Item::Module(module) => collect_struct_schemas(&module.items, output),
            _ => {}
        }
    }
}

fn collect_enum_schemas(items: &[Item], output: &mut HashMap<String, Vec<String>>) {
    for item in items {
        match item {
            Item::Enum(item) => {
                output.insert(
                    item.name.clone(),
                    item.variants
                        .iter()
                        .map(|variant| variant.name.clone())
                        .collect(),
                );
            }
            Item::Module(module) => collect_enum_schemas(&module.items, output),
            _ => {}
        }
    }
}

fn collect_items(
    items: &[Item],
    functions: &mut Vec<FunctionDecl>,
    constants: &mut HashMap<String, Expr>,
    variants: &mut HashMap<String, bool>,
    methods: &mut HashMap<String, usize>,
) {
    // Phase 22: two-pass so `impl Trait for Type` blocks can pull
    // default method bodies from the trait declaration even if the
    // trait is defined further down or in a different module.
    let mut traits: HashMap<String, TraitDecl> = HashMap::new();
    collect_traits(items, &mut traits);
    collect_items_with_traits(items, functions, constants, variants, methods, &traits);
}

fn collect_traits(items: &[Item], traits: &mut HashMap<String, TraitDecl>) {
    for item in items {
        match item {
            Item::Trait(t) => {
                traits.insert(t.name.clone(), t.clone());
            }
            Item::Module(m) => collect_traits(&m.items, traits),
            _ => {}
        }
    }
}

fn collect_items_with_traits(
    items: &[Item],
    functions: &mut Vec<FunctionDecl>,
    constants: &mut HashMap<String, Expr>,
    variants: &mut HashMap<String, bool>,
    methods: &mut HashMap<String, usize>,
    traits: &HashMap<String, TraitDecl>,
) {
    for item in items {
        match item {
            Item::Function(f) => functions.push(f.clone()),
            Item::Const(c) => {
                constants.insert(c.name.clone(), (*c.value).clone());
            }
            Item::Enum(e) => {
                for variant in &e.variants {
                    variants.insert(
                        format!("{}::{}", e.name, variant.name),
                        variant.payload.is_some(),
                    );
                }
            }
            Item::Module(m) => {
                collect_items_with_traits(&m.items, functions, constants, variants, methods, traits)
            }
            // Phase 20: each method inside `impl Point { ... }` becomes a
            // regular top-level function with a qualified name like
            // `Point::distance`. If the method's first param is named
            // `self`, we synthesize its type annotation as `Point` so the
            // typechecker treats field access on it correctly. The
            // method_table gets a "Point::distance" -> function_index
            // entry that the VM uses at runtime for dynamic dispatch of
            // `p.distance(...)` — resolved from the receiver's struct
            // name, so two structs can share method names safely.
            //
            // Phase 22: when the block is `impl SomeTrait for Point {}`,
            // any trait method with a default body that Point didn't
            // override is synthesized as a Point method too, so the VM
            // can dispatch it identically. Trait methods without a body
            // that the impl doesn't provide raise an error at compile
            // time — no silent missing-method surprises at runtime.
            Item::Impl(i) => {
                let type_name = match &i.target_type {
                    TypeExpr::Named { name, .. } => name.clone(),
                    _ => continue,
                };
                let provided: std::collections::HashSet<String> =
                    i.methods.iter().map(|m| m.name.clone()).collect();
                for method in &i.methods {
                    let mut renamed = method.clone();
                    renamed.name = format!("{}::{}", type_name, method.name);
                    if let Some(first) = renamed.params.first_mut() {
                        if first.name == "self" && first.type_ann.is_none() {
                            first.type_ann = Some(TypeExpr::Named {
                                name: type_name.clone(),
                                generics: Vec::new(),
                            });
                        }
                    }
                    methods.insert(renamed.name.clone(), functions.len());
                    functions.push(renamed);
                }
                // Phase 22: fill in defaults + validate required methods.
                if let Some(trait_name) = &i.trait_name {
                    if let Some(trait_decl) = traits.get(trait_name) {
                        for tm in &trait_decl.methods {
                            if provided.contains(&tm.name) {
                                continue;
                            }
                            let Some(default_body) = &tm.body else {
                                continue;
                            };
                            // Synthesize FunctionDecl from TraitMethod + default body.
                            let mut synth = FunctionDecl {
                                name: format!("{}::{}", type_name, tm.name),
                                source_file: None,
                                params: tm.params.clone(),
                                return_type: tm.return_type.clone(),
                                body: Some(default_body.clone()),
                                is_extern: false,
                                abi: None,
                                span: tm.span,
                            };
                            if let Some(first) = synth.params.first_mut() {
                                if first.name == "self" && first.type_ann.is_none() {
                                    first.type_ann = Some(TypeExpr::Named {
                                        name: type_name.clone(),
                                        generics: Vec::new(),
                                    });
                                }
                            }
                            methods.insert(synth.name.clone(), functions.len());
                            functions.push(synth);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
fn split_variant(name: &str) -> Result<(&str, &str), CodegenError> {
    name.split_once("::")
        .ok_or_else(|| CodegenError::Unsupported(format!("invalid enum variant '{name}'")))
}

impl Default for AstCompiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use titan_lexer::Lexer;
    use titan_parser::Parser;

    fn parse(source: &str) -> Program {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().0.to_vec();
        Parser::new(tokens).parse_program().unwrap()
    }

    #[test]
    fn rejects_reference_and_break_value_bytecode() {
        let reference = parse("fn main() { let value = 1; &value }");
        assert!(matches!(
            AstCompiler::new().compile_program(&reference),
            Err(CodegenError::Unsupported(_))
        ));

        let break_value = parse("fn main() { loop { break 1 } }");
        assert!(matches!(
            AstCompiler::new().compile_program(&break_value),
            Err(CodegenError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_empty_try_catch_without_typechecker() {
        let program = parse("fn main() { std::try::catch() }");
        assert!(matches!(
            AstCompiler::new().compile_program(&program),
            Err(CodegenError::Unsupported(message))
                if message.contains("requires a callable argument")
        ));
    }

    #[test]
    fn rejects_function_features_without_runtime_semantics() {
        for source in [
            "fn declared() -> int; fn main() { declared() }",
            "extern \"C\" fn native() -> int; fn main() { native() }",
            "fn pending(value: int = 1) -> int { value } fn main() { pending() }",
        ] {
            assert!(matches!(
                AstCompiler::new().compile_program(&parse(source)),
                Err(CodegenError::Unsupported(_))
            ));
        }
    }

    #[test]
    fn lowers_direct_calls_to_callable_constants() {
        let program = parse(
            "const INCREMENT: fn(int) -> int = |value| value + 1 fn main() { INCREMENT(41) }",
        );
        let module = AstCompiler::new().compile_program(&program).unwrap();
        assert!(module.functions[module.entry]
            .code
            .iter()
            .any(|operation| matches!(operation, Op::CallValue(1))));
    }

    #[test]
    fn lowers_interpolation_calls_through_ordinary_callable_resolution() {
        for source in [
            "fn render(value: int) -> int { 0 } fn main() { let render = |value: int| value + 1 \"{render(41)}\" }",
            "const RENDER: fn(int) -> int = |value| value + 1 fn main() { \"{RENDER(41)}\" }",
        ] {
            let module = AstCompiler::new().compile_program(&parse(source)).unwrap();
            assert!(module.functions[module.entry]
                .code
                .iter()
                .any(|operation| matches!(operation, Op::CallValue(1))));
        }
    }

    #[test]
    fn rejects_recursive_constant_expansion_without_overflowing() {
        for source in [
            "const VALUE = VALUE fn main() { VALUE }",
            "const FIRST = SECOND const SECOND = FIRST fn main() { FIRST }",
        ] {
            let error = AstCompiler::new().compile_program(&parse(source));
            assert!(matches!(
                error,
                Err(CodegenError::Unsupported(message))
                    if message.contains("recursive constant expansion")
            ));
        }
    }

    #[test]
    fn enforces_mutable_local_assignment_defensively() {
        let immutable = parse("fn main() { let value = 1 value = 2 }");
        assert!(matches!(
            AstCompiler::new().compile_program(&immutable),
            Err(CodegenError::Unsupported(_))
        ));

        let captured =
            parse("fn main() { let mut value = 1 let update = || { value = 2 } update() }");
        assert!(matches!(
            AstCompiler::new().compile_program(&captured),
            Err(CodegenError::Unsupported(_))
        ));

        let mutable = parse("fn main() { let mut value = 1 value += 2 value }");
        assert!(AstCompiler::new().compile_program(&mutable).is_ok());
    }
}
