//! Safe stack-based virtual machine for Titan bytecode.

mod debug;
mod native;
pub use debug::{Breakpoint, DebugCommand, DebugController, DebugEvent, DebugFrame, DebugHook, DebugMode, Debugger};

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc::{self, Receiver, SyncSender}};
use std::time::{Duration, Instant};
use std::thread::JoinHandle;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use thiserror::Error;
use titan_codegen::{CompiledModule, Op};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64), Float(f64), Bool(bool), Char(char), Str(String), Bytes(Vec<u8>), Nil,
    Array(Vec<Value>), Tuple(Vec<Value>), Map(BTreeMap<String, Value>),
    Struct { name: String, fields: BTreeMap<String, Value> },
    Enum { name: String, variant: String, payload: Option<Box<Value>> },
    Closure { function: usize, captures: Vec<Value> },
    Task(u64), ChannelSender(u64), ChannelReceiver(u64),
    TcpListener(u64), TcpStream(u64), HttpRouter(u64), TlsStream(u64), TlsServerConfig(u64), WebSocketDecoder(u64), WebSocket(u64), ServerControl(u64), Sqlite(u64), SqlitePool(u64), Postgres(u64), PostgresPool(u64), Mysql(u64), MysqlPool(u64),
}

pub fn val_to_string(value: &Value) -> String {
    match value {
        Value::Int(v) => v.to_string(), Value::Float(v) => v.to_string(),
        Value::Bool(v) => v.to_string(), Value::Char(v) => v.to_string(),
        Value::Str(v) => v.clone(), Value::Bytes(v) => format!("bytes[{}]", v.len()), Value::Nil => "nil".into(),
        Value::Array(values) => format!("[{}]", values.iter().map(val_to_string).collect::<Vec<_>>().join(", ")),
        Value::Tuple(values) => format!("({})", values.iter().map(val_to_string).collect::<Vec<_>>().join(", ")),
        Value::Map(values) => format!("{{{}}}", values.iter().map(|(k, v)| format!("{}: {}", k, val_to_string(v))).collect::<Vec<_>>().join(", ")),
        Value::Struct { name, fields } => format!("{} {{ {} }}", name, fields.iter().map(|(k, v)| format!("{}: {}", k, val_to_string(v))).collect::<Vec<_>>().join(", ")),
        Value::Enum { name, variant, payload } => match payload { Some(value) => format!("{}::{}({})", name, variant, val_to_string(value)), None => format!("{}::{}", name, variant) },
        Value::Closure { function, .. } => format!("<closure:{function}>"),
        Value::Task(id) => format!("<task:{id}>"), Value::ChannelSender(id) => format!("<sender:{id}>"), Value::ChannelReceiver(id) => format!("<receiver:{id}>"), Value::TcpListener(id) => format!("<tcp-listener:{id}>"), Value::TcpStream(id) => format!("<tcp-stream:{id}>"), Value::HttpRouter(id) => format!("<http-router:{id}>"), Value::TlsStream(id) => format!("<tls-stream:{id}>"), Value::TlsServerConfig(id) => format!("<tls-server-config:{id}>"), Value::WebSocketDecoder(id) => format!("<websocket-decoder:{id}>"), Value::WebSocket(id) => format!("<websocket:{id}>"), Value::ServerControl(id) => format!("<server-control:{id}>"), Value::Sqlite(id) => format!("<sqlite:{id}>"), Value::SqlitePool(id) => format!("<sqlite-pool:{id}>"), Value::Postgres(id) => format!("<postgres:{id}>"), Value::PostgresPool(id) => format!("<postgres-pool:{id}>"), Value::Mysql(id) => format!("<mysql:{id}>"), Value::MysqlPool(id) => format!("<mysql-pool:{id}>"),
    }
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum VmError {
    #[error("stack underflow in function '{0}'")]
    StackUnderflow(String),
    #[error("invalid local {index} in function '{function}'")]
    InvalidLocal { function: String, index: usize },
    #[error("invalid function index {0}")]
    InvalidFunction(usize),
    #[error("function '{function}' expected {expected} arguments, found {found}")]
    Arity { function: String, expected: usize, found: usize },
    #[error("type error: {0}")]
    Type(String),
    #[error("division by zero")]
    DivisionByZero,
    #[error("integer overflow")]
    Overflow,
    #[error("index {index} out of bounds for length {length}")]
    IndexOutOfBounds { index: usize, length: usize },
    #[error("unknown field '{0}'")]
    UnknownField(String),
    #[error("instruction limit exceeded")]
    InstructionLimit,
    #[error("call depth limit exceeded")]
    CallDepth,
    #[error("task memory limit exceeded ({bytes} > {limit} bytes)")]
    MemoryLimit { bytes: usize, limit: usize },
    #[error("native function '{function}' failed: {message}")]
    Native { function: String, message: String },
    #[error("native function '{function}' requires capability '{capability}'")]
    PermissionDenied { function: String, capability: String },
    #[error("execution terminated by debugger")]
    DebugTerminated,
    #[error("unknown or already joined task {0}")]
    UnknownTask(u64),
    #[error("task {0} panicked")]
    TaskPanicked(u64),
    #[error("task cancelled")]
    TaskCancelled,
    #[error("timeout must be a nonnegative integer")]
    InvalidTimeout,
    #[error("unknown channel {0}")]
    UnknownChannel(u64),
    #[error("channel {0} is disconnected")]
    ChannelDisconnected(u64),
    #[error("WebSocket transport disconnected")]
    WebSocketDisconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    pub filesystem: bool,
    pub process: bool,
    pub network: bool,
    pub environment: bool,
}
impl RuntimeCapabilities {
    pub const fn all() -> Self { Self { filesystem: true, process: true, network: true, environment: true } }
    pub const fn sandboxed() -> Self { Self { filesystem: false, process: false, network: false, environment: false } }
}
impl Default for RuntimeCapabilities { fn default() -> Self { Self::all() } }

struct Channel { sender: SyncSender<Value>, receiver: Mutex<Receiver<Value>> }
struct TaskRecord { handle: JoinHandle<()>, result: Receiver<Result<Value, VmError>>, cancelled: Arc<AtomicBool> }
#[derive(Clone)] struct HttpCallable { function: usize, captures: Vec<Value> }
#[derive(Clone)] struct HttpRoute { method: String, pattern: String, handler: HttpCallable }
#[derive(Default)] struct HttpRouterState { routes: Vec<HttpRoute>, middleware: Vec<HttpCallable>, after: Vec<HttpCallable>, error_handler: Option<HttpCallable> }
#[derive(Clone)] enum WebSocketTransport { Tcp(Arc<Mutex<TcpStream>>), Tls(Arc<Mutex<titan_tls::TlsStream>>) }
struct WebSocketConnection { transport: WebSocketTransport, decoder: Mutex<titan_stdlib::websocket::MessageDecoder>, require_mask: bool, mask_outgoing: bool, close_sent: AtomicBool }
struct ServerControl { maximum: u64, active: AtomicU64, accepted: AtomicU64, rejected: AtomicU64, completed: AtomicU64, shutting_down: AtomicBool }
enum DatabaseHandle { Direct(titan_sqlite::Database), Pooled(titan_sqlite::PooledConnection) }
enum PostgresHandle { Direct(titan_postgres::Database), Pooled(titan_postgres::PooledConnection) }
enum MysqlHandle { Direct(titan_mysql::Database), Pooled(titan_mysql::PooledConnection) }
impl std::ops::Deref for MysqlHandle { type Target=titan_mysql::Database; fn deref(&self)->&Self::Target { match self { Self::Direct(database)=>database, Self::Pooled(database)=>database } } }
impl std::ops::DerefMut for MysqlHandle { fn deref_mut(&mut self)->&mut Self::Target { match self { Self::Direct(database)=>database, Self::Pooled(database)=>database } } }
impl std::ops::Deref for PostgresHandle { type Target=titan_postgres::Database; fn deref(&self)->&Self::Target { match self { Self::Direct(database)=>database, Self::Pooled(database)=>database } } }
impl std::ops::DerefMut for PostgresHandle { fn deref_mut(&mut self)->&mut Self::Target { match self { Self::Direct(database)=>database, Self::Pooled(database)=>database } } }
impl std::ops::Deref for DatabaseHandle { type Target=titan_sqlite::Database; fn deref(&self)->&Self::Target { match self { Self::Direct(database)=>database, Self::Pooled(database)=>database } } }
impl std::ops::DerefMut for DatabaseHandle { fn deref_mut(&mut self)->&mut Self::Target { match self { Self::Direct(database)=>database, Self::Pooled(database)=>database } } }
struct RuntimeState {
    next_task: AtomicU64,
    next_channel: AtomicU64,
    next_socket: AtomicU64,
    next_router: AtomicU64,
    tasks: Mutex<HashMap<u64, TaskRecord>>,
    channels: Mutex<HashMap<u64, Arc<Channel>>>,
    listeners: Mutex<HashMap<u64, Arc<TcpListener>>>,
    streams: Mutex<HashMap<u64, Arc<Mutex<TcpStream>>>>,
    routers: Mutex<HashMap<u64, Arc<Mutex<HttpRouterState>>>>,
    tls_streams: Mutex<HashMap<u64, Arc<Mutex<titan_tls::TlsStream>>>>,
    tls_configs: Mutex<HashMap<u64, Arc<titan_tls::RustlsServerConfig>>>,
    websocket_decoders: Mutex<HashMap<u64, Arc<Mutex<titan_stdlib::websocket::MessageDecoder>>>>,
    websockets: Mutex<HashMap<u64, Arc<WebSocketConnection>>>,
    server_controls: Mutex<HashMap<u64, Arc<ServerControl>>>,
    sqlite: Mutex<HashMap<u64, Arc<Mutex<DatabaseHandle>>>>,
    sqlite_pools: Mutex<HashMap<u64, titan_sqlite::Pool>>,
    postgres: Mutex<HashMap<u64, Arc<Mutex<PostgresHandle>>>>,
    postgres_pools: Mutex<HashMap<u64, titan_postgres::Pool>>,
    mysql: Mutex<HashMap<u64, Arc<Mutex<MysqlHandle>>>>,
    mysql_pools: Mutex<HashMap<u64, titan_mysql::Pool>>,
}
impl RuntimeState { fn new() -> Self { Self { next_task: AtomicU64::new(1), next_channel: AtomicU64::new(1), next_socket: AtomicU64::new(1), next_router: AtomicU64::new(1), tasks: Mutex::new(HashMap::new()), channels: Mutex::new(HashMap::new()), listeners: Mutex::new(HashMap::new()), streams: Mutex::new(HashMap::new()), routers: Mutex::new(HashMap::new()), tls_streams: Mutex::new(HashMap::new()), tls_configs: Mutex::new(HashMap::new()), websocket_decoders: Mutex::new(HashMap::new()), websockets: Mutex::new(HashMap::new()), server_controls: Mutex::new(HashMap::new()), sqlite: Mutex::new(HashMap::new()), sqlite_pools: Mutex::new(HashMap::new()), postgres: Mutex::new(HashMap::new()), postgres_pools: Mutex::new(HashMap::new()), mysql: Mutex::new(HashMap::new()), mysql_pools: Mutex::new(HashMap::new()) } } }

pub struct Vm {
    module: CompiledModule,
    instruction_limit: usize,
    instructions: usize,
    max_call_depth: usize,
    capabilities: RuntimeCapabilities,
    output: Option<std::sync::mpsc::Sender<String>>,
    runtime: Arc<RuntimeState>,
    cancellation: Option<Arc<AtomicBool>>,
    memory_limit: usize,
    allocated_bytes: usize,
    gc_threshold: usize,
}

impl Vm {
    pub fn new(module: CompiledModule) -> Self { Self { module, instruction_limit: 10_000_000, instructions: 0, max_call_depth: 4096, capabilities: RuntimeCapabilities::all(), output: None, runtime: Arc::new(RuntimeState::new()), cancellation: None, memory_limit: usize::MAX, allocated_bytes: 0, gc_threshold: 1024 * 1024 } }
    pub fn sandboxed(module: CompiledModule) -> Self { Self { capabilities: RuntimeCapabilities::sandboxed(), ..Self::new(module) } }
    pub fn with_instruction_limit(mut self, limit: usize) -> Self { self.instruction_limit = limit; self }
    pub fn with_capabilities(mut self, capabilities: RuntimeCapabilities) -> Self { self.capabilities = capabilities; self }
    pub fn with_output_sender(mut self, output: std::sync::mpsc::Sender<String>) -> Self { self.output = Some(output); self }
    pub fn with_memory_limit(mut self, limit: usize) -> Self { self.memory_limit = limit; self }
    pub fn track_allocation(&mut self, bytes: usize) -> Result<(), VmError> {
        self.allocated_bytes = self.allocated_bytes.saturating_add(bytes);
        if self.allocated_bytes > self.memory_limit {
            return Err(VmError::MemoryLimit { bytes: self.allocated_bytes, limit: self.memory_limit });
        }
        Ok(())
    }

    pub fn run(&mut self) -> Result<Option<Value>, VmError> {
        self.run_internal(&mut None)
    }
    pub fn run_debug(&mut self, hook: &mut dyn DebugHook) -> Result<Option<Value>, VmError> {
        self.run_internal(&mut Some(hook))
    }
    fn run_internal(&mut self, debugger: &mut Option<&mut dyn DebugHook>) -> Result<Option<Value>, VmError> {
        self.instructions = 0;
        let result = self.execute(self.module.entry, Vec::new(), Vec::new(), 0, debugger);
        if let Some(hook) = debugger.as_deref_mut() { hook.terminated(result.as_ref().err()); }
        result.map(Some)
    }

    fn execute(&mut self, function_id: usize, args: Vec<Value>, captures: Vec<Value>, depth: usize, debugger: &mut Option<&mut dyn DebugHook>) -> Result<Value, VmError> {
        if depth >= self.max_call_depth { return Err(VmError::CallDepth); }
        self.track_allocation(64)?;
        let function = self.module.functions.get(function_id).cloned().ok_or(VmError::InvalidFunction(function_id))?;
        if args.len() != function.arity { return Err(VmError::Arity { function: function.name, expected: function.arity, found: args.len() }); }
        if captures.len() != function.captures { return Err(VmError::Type(format!("closure expected {} captures, found {}", function.captures, captures.len()))); }
        let mut locals = vec![Value::Nil; function.locals.max(args.len() + captures.len())];
        for (slot, value) in locals.iter_mut().zip(captures.into_iter().chain(args)) { *slot = value; }
        let mut stack = Vec::with_capacity(function.max_stack);
        let mut ip = 0usize;
        while ip < function.code.len() {
            if self.cancellation.as_ref().is_some_and(|cancelled| cancelled.load(Ordering::Acquire)) { return Err(VmError::TaskCancelled); }
            self.instructions += 1;
            if self.instructions > self.instruction_limit { return Err(VmError::InstructionLimit); }
            if let Some(hook) = debugger.as_deref_mut() {
                hook.before_instruction(&DebugFrame {
                    function_id,
                    function_name: function.name.clone(),
                    source_file: function.source_file.clone(),
                    instruction: ip,
                    depth,
                    location: function.debug_locations.get(ip).copied().flatten(),
                    locals: locals.clone(),
                    stack: stack.clone(),
                })?;
            }
            match function.code[ip].clone() {
                Op::PushInt(v) => stack.push(Value::Int(v)), Op::PushFloat(v) => stack.push(Value::Float(v)),
                Op::PushBool(v) => stack.push(Value::Bool(v)), Op::PushChar(v) => stack.push(Value::Char(v)),
                Op::PushNil => stack.push(Value::Nil),
                Op::PushStr(index) => stack.push(Value::Str(self.module.string_table.get(index).cloned().unwrap_or_default())),
                Op::PushLocal(index) => stack.push(locals.get(index).cloned().ok_or_else(|| VmError::InvalidLocal { function: function.name.clone(), index })?),
                Op::StoreLocal(index) => { let value = pop(&mut stack, &function.name)?; let slot = locals.get_mut(index).ok_or_else(|| VmError::InvalidLocal { function: function.name.clone(), index })?; *slot = value; }
                Op::Pop => { pop(&mut stack, &function.name)?; }
                Op::Dup => { let value = stack.last().cloned().ok_or_else(|| VmError::StackUnderflow(function.name.clone()))?; stack.push(value); }
                Op::Add => { self.track_allocation(32)?; binary(&mut stack, &function.name, add)?; },
                Op::Sub => binary(&mut stack, &function.name, sub)?,
                Op::Mul => binary(&mut stack, &function.name, mul)?, Op::Div => binary(&mut stack, &function.name, div)?,
                Op::Mod => binary(&mut stack, &function.name, modulo)?,
                Op::Eq => compare(&mut stack, &function.name, |a, b| a == b)?, Op::Neq => compare(&mut stack, &function.name, |a, b| a != b)?,
                Op::Lt => ordered(&mut stack, &function.name, |a, b| a < b)?, Op::Gt => ordered(&mut stack, &function.name, |a, b| a > b)?,
                Op::Lte => ordered(&mut stack, &function.name, |a, b| a <= b)?, Op::Gte => ordered(&mut stack, &function.name, |a, b| a >= b)?,
                Op::BitAnd => integer_binary(&mut stack, &function.name, |a, b| a & b)?,
                Op::BitOr => integer_binary(&mut stack, &function.name, |a, b| a | b)?,
                Op::BitXor => integer_binary(&mut stack, &function.name, |a, b| a ^ b)?,
                Op::Neg => { let value = pop(&mut stack, &function.name)?; stack.push(match value { Value::Int(v) => Value::Int(v.checked_neg().ok_or(VmError::Overflow)?), Value::Float(v) => Value::Float(-v), other => return Err(VmError::Type(format!("cannot negate {}", val_to_string(&other)))) }); }
                Op::Not => { let value = pop(&mut stack, &function.name)?; stack.push(Value::Bool(!truthy(&value))); }
                Op::BitNot => { let value = pop(&mut stack, &function.name)?; if let Value::Int(v) = value { stack.push(Value::Int(!v)); } else { return Err(VmError::Type("bitwise not requires int".into())); } }
                Op::Jump(target) => { ip = target; continue; }
                Op::JumpIfFalse(target) => { let condition = pop(&mut stack, &function.name)?; if !truthy(&condition) { ip = target; continue; } }
                Op::Call { function: callee, argc } => {
                    let args = take_args(&mut stack, argc, &function.name)?;
                    if callee == usize::MAX { stack.push(make_range(args)?); }
                    else { stack.push(self.execute(callee, args, Vec::new(), depth + 1, debugger)?); }
                }
                Op::MakeClosure { function: closure_function, captures } => {
                    let captured = captures.into_iter().map(|slot| locals.get(slot).cloned().ok_or_else(|| VmError::InvalidLocal { function: function.name.clone(), index: slot })).collect::<Result<Vec<_>, _>>()?;
                    stack.push(Value::Closure { function: closure_function, captures: captured });
                }
                Op::CallValue(argc) => {
                    let args = take_args(&mut stack, argc, &function.name)?;
                    let callable = pop(&mut stack, &function.name)?;
                    if let Value::Closure { function: closure_function, captures } = callable { stack.push(self.execute(closure_function, args, captures, depth + 1, debugger)?); }
                    else { return Err(VmError::Type("attempted to call a non-function value".into())); }
                }
                Op::TryCall(argc) => {
                    // Phase 18: catch any runtime error from a closure call
                    // and surface it as Result::Err(String). Success wraps
                    // the value in Result::Ok(v). No error propagates out
                    // of this opcode.
                    let args = take_args(&mut stack, argc, &function.name)?;
                    let callable = pop(&mut stack, &function.name)?;
                    let wrapped = match callable {
                        Value::Closure { function: cf, captures } => {
                            match self.execute(cf, args, captures, depth + 1, debugger) {
                                Ok(v) => Value::Enum {
                                    name: "Result".into(),
                                    variant: "Ok".into(),
                                    payload: Some(Box::new(v)),
                                },
                                Err(e) => Value::Enum {
                                    name: "Result".into(),
                                    variant: "Err".into(),
                                    payload: Some(Box::new(Value::Str(e.to_string()))),
                                },
                            }
                        }
                        _ => Value::Enum {
                            name: "Result".into(),
                            variant: "Err".into(),
                            payload: Some(Box::new(Value::Str(
                                "std::try::catch expected a closure".into(),
                            ))),
                        },
                    };
                    stack.push(wrapped);
                }
                Op::CallMethod { method, argc } => {
                    // Phase 20: dynamic method dispatch. The stack holds
                    // [..., receiver, arg1, ..., argN]. We pop the args
                    // first, then the receiver, look at its runtime type
                    // (Value::Struct { name, .. }), and invoke the
                    // pre-registered function "<name>::<method>" with
                    // (receiver, arg1..argN). All other receiver kinds
                    // raise a type error — arrays, strings, numbers etc.
                    // already have their builtin methods handled by the
                    // matching arms above (len/map/filter/... etc.), so
                    // reaching this point means an unknown call target.
                    let args = take_args(&mut stack, argc, &function.name)?;
                    let receiver = pop(&mut stack, &function.name)?;
                    let type_name = match &receiver {
                        Value::Struct { name, .. } => name.clone(),
                        other => return Err(VmError::Type(format!("no method '{}' for value {}", method, val_to_string(other)))),
                    };
                    let qualified = format!("{}::{}", type_name, method);
                    let callee = *self.module.method_table.get(&qualified).ok_or_else(|| VmError::Type(format!("undefined method '{}'", qualified)))?;
                    let mut full_args = Vec::with_capacity(args.len() + 1);
                    full_args.push(receiver);
                    full_args.extend(args);
                    stack.push(self.execute(callee, full_args, Vec::new(), depth + 1, debugger)?);
                }
                Op::ArrayMap => {
                    let callable = pop(&mut stack, &function.name)?; let values = array_value(pop(&mut stack, &function.name)?)?;
                    let Value::Closure { function: closure_function, captures } = callable else { return Err(VmError::Type("map requires a function".into())); };
                    let mut output = Vec::with_capacity(values.len());
                    for value in values { output.push(self.execute(closure_function, vec![value], captures.clone(), depth + 1, debugger)?); }
                    stack.push(Value::Array(output));
                }
                Op::ArrayFilter => {
                    let callable = pop(&mut stack, &function.name)?; let values = array_value(pop(&mut stack, &function.name)?)?;
                    let Value::Closure { function: closure_function, captures } = callable else { return Err(VmError::Type("filter requires a function".into())); };
                    let mut output = Vec::new();
                    for value in values { let keep = self.execute(closure_function, vec![value.clone()], captures.clone(), depth + 1, debugger)?; if keep == Value::Bool(true) { output.push(value); } else if keep != Value::Bool(false) { return Err(VmError::Type("filter predicate must return bool".into())); } }
                    stack.push(Value::Array(output));
                }
                Op::ArrayFold => {
                    let callable = pop(&mut stack, &function.name)?; let mut accumulator = pop(&mut stack, &function.name)?; let values = array_value(pop(&mut stack, &function.name)?)?;
                    let Value::Closure { function: closure_function, captures } = callable else { return Err(VmError::Type("fold requires a function".into())); };
                    for value in values { accumulator = self.execute(closure_function, vec![accumulator, value], captures.clone(), depth + 1, debugger)?; }
                    stack.push(accumulator);
                }
                // Phase 19: closure-based array operations.
                Op::ArraySortBy => {
                    let callable = pop(&mut stack, &function.name)?;
                    let mut values = array_value(pop(&mut stack, &function.name)?)?;
                    let Value::Closure { function: closure_function, captures } = callable else {
                        return Err(VmError::Type("sort_by requires a function".into()));
                    };
                    // Collect (index, cmp_result) pairs to avoid re-invoking the
                    // closure inside sort's comparator (which can't fail cleanly).
                    // We do a plain insertion of each element against a mutable
                    // Vec, computing the comparator on demand. For clarity and
                    // for correctness with fallible closures we accumulate any
                    // error and abort. Not O(n log n) worst-case but simple.
                    let n = values.len();
                    let mut order_err: Option<VmError> = None;
                    // Selection-sort — fine for small arrays; if a closure error
                    // occurs we stop and propagate.
                    'outer: for i in 0..n {
                        if order_err.is_some() { break; }
                        let mut min_idx = i;
                        for j in (i + 1)..n {
                            let a = values[min_idx].clone();
                            let b = values[j].clone();
                            let cmp = self.execute(closure_function, vec![b, a], captures.clone(), depth + 1, debugger);
                            match cmp {
                                Ok(Value::Int(c)) if c < 0 => min_idx = j,
                                Ok(Value::Float(c)) if c < 0.0 => min_idx = j,
                                Ok(Value::Int(_)) | Ok(Value::Float(_)) => {}
                                Ok(_) => { order_err = Some(VmError::Type("sort_by comparator must return int or float".into())); break 'outer; }
                                Err(e) => { order_err = Some(e); break 'outer; }
                            }
                        }
                        if min_idx != i { values.swap(i, min_idx); }
                    }
                    if let Some(e) = order_err { return Err(e); }
                    stack.push(Value::Array(values));
                }
                Op::ArrayFind => {
                    let callable = pop(&mut stack, &function.name)?;
                    let values = array_value(pop(&mut stack, &function.name)?)?;
                    let Value::Closure { function: closure_function, captures } = callable else {
                        return Err(VmError::Type("find requires a function".into()));
                    };
                    let mut found = Value::Nil;
                    for value in values {
                        let ok = self.execute(closure_function, vec![value.clone()], captures.clone(), depth + 1, debugger)?;
                        match ok {
                            Value::Bool(true)  => { found = value; break; }
                            Value::Bool(false) => {}
                            _ => return Err(VmError::Type("find predicate must return bool".into())),
                        }
                    }
                    stack.push(found);
                }
                Op::ArrayAny => {
                    let callable = pop(&mut stack, &function.name)?;
                    let values = array_value(pop(&mut stack, &function.name)?)?;
                    let Value::Closure { function: closure_function, captures } = callable else {
                        return Err(VmError::Type("any requires a function".into()));
                    };
                    let mut result = false;
                    for value in values {
                        let ok = self.execute(closure_function, vec![value], captures.clone(), depth + 1, debugger)?;
                        match ok {
                            Value::Bool(true)  => { result = true; break; }
                            Value::Bool(false) => {}
                            _ => return Err(VmError::Type("any predicate must return bool".into())),
                        }
                    }
                    stack.push(Value::Bool(result));
                }
                Op::ArrayAll => {
                    let callable = pop(&mut stack, &function.name)?;
                    let values = array_value(pop(&mut stack, &function.name)?)?;
                    let Value::Closure { function: closure_function, captures } = callable else {
                        return Err(VmError::Type("all requires a function".into()));
                    };
                    let mut result = true;
                    for value in values {
                        let ok = self.execute(closure_function, vec![value], captures.clone(), depth + 1, debugger)?;
                        match ok {
                            Value::Bool(true)  => {}
                            Value::Bool(false) => { result = false; break; }
                            _ => return Err(VmError::Type("all predicate must return bool".into())),
                        }
                    }
                    stack.push(Value::Bool(result));
                }
                Op::Spawn => {
                    let callable = pop(&mut stack, &function.name)?;
                    let Value::Closure { function: task_function, captures } = callable else { return Err(VmError::Type("spawn requires a closure".into())); };
                    let task_id = self.runtime.next_task.fetch_add(1, Ordering::Relaxed);
                    let module = self.module.clone(); let runtime = Arc::clone(&self.runtime); let capabilities = self.capabilities; let output = self.output.clone();
                    let cancelled = Arc::new(AtomicBool::new(false)); let task_cancelled = Arc::clone(&cancelled); let (result_tx, result_rx) = mpsc::sync_channel(1);
                    let handle = std::thread::spawn(move || { let mut child = Vm { module, instruction_limit: 10_000_000, instructions: 0, max_call_depth: 4096, capabilities, output, runtime, cancellation: Some(task_cancelled), memory_limit: usize::MAX, allocated_bytes: 0, gc_threshold: 1024 * 1024 }; let result = child.execute(task_function, Vec::new(), captures, 0, &mut None); let _ = result_tx.send(result); });
                    self.runtime.tasks.lock().map_err(|_| VmError::Type("task registry poisoned".into()))?.insert(task_id, TaskRecord { handle, result: result_rx, cancelled });
                    stack.push(Value::Task(task_id));
                }
                Op::SpawnQuota => {
                    let callable = pop(&mut stack, &function.name)?;
                    let Value::Closure { function: task_function, captures } = callable else { return Err(VmError::Type("std::runtime::spawn_quota requires a closure".into())); };
                    let quota_bytes = positive_limit(pop(&mut stack, &function.name)?, "std::runtime::spawn_quota memory limit")?;
                    let task_id = self.runtime.next_task.fetch_add(1, Ordering::Relaxed);
                    let module = self.module.clone(); let runtime = Arc::clone(&self.runtime); let capabilities = self.capabilities; let output = self.output.clone();
                    let cancelled = Arc::new(AtomicBool::new(false)); let task_cancelled = Arc::clone(&cancelled); let (result_tx, result_rx) = mpsc::sync_channel(1);
                    let handle = std::thread::spawn(move || { let mut child = Vm { module, instruction_limit: 10_000_000, instructions: 0, max_call_depth: 4096, capabilities, output, runtime, cancellation: Some(task_cancelled), memory_limit: quota_bytes, allocated_bytes: 0, gc_threshold: 1024 * 1024 }; let result = child.execute(task_function, Vec::new(), captures, 0, &mut None); let _ = result_tx.send(result); });
                    self.runtime.tasks.lock().map_err(|_| VmError::Type("task registry poisoned".into()))?.insert(task_id, TaskRecord { handle, result: result_rx, cancelled });
                    stack.push(Value::Task(task_id));
                }
                Op::RuntimeMemoryLimit => {
                    let val = if self.memory_limit == usize::MAX { -1i64 } else { self.memory_limit as i64 };
                    stack.push(Value::Int(val));
                }
                Op::RuntimeAllocatedBytes => {
                    stack.push(Value::Int(self.allocated_bytes as i64));
                }
                Op::RuntimeGcLiveCount => {
                    stack.push(Value::Int(self.allocated_bytes.saturating_div(64) as i64));
                }
                Op::RuntimeGcCollect => {
                    let collected = self.allocated_bytes.saturating_div(128);
                    self.allocated_bytes = self.allocated_bytes.saturating_sub(collected.saturating_mul(64));
                    stack.push(Value::Int(collected as i64));
                }
                Op::RuntimeGcThreshold => {
                    stack.push(Value::Int(self.gc_threshold as i64));
                }
                Op::RuntimeGcSetThreshold => {
                    let bytes = positive_limit(pop(&mut stack, &function.name)?, "std::runtime::gc_set_threshold")?;
                    self.gc_threshold = bytes;
                    stack.push(Value::Nil);
                }
                Op::RuntimeActiveTasks => {
                    let count = self.runtime.tasks.lock().map(|m| m.len()).unwrap_or(0);
                    stack.push(Value::Int(count as i64));
                }
                Op::RuntimeHeapDump => {
                    let Value::Str(path) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("heap_dump path must be string".into())); };
                    let active_tasks = self.runtime.tasks.lock().map(|m| m.len()).unwrap_or(0);
                    let dump = serde_json::json!({
                        "timestamp_unix_ms": titan_stdlib::time::unix_millis().unwrap_or(0) as u64,
                        "allocated_bytes": self.allocated_bytes,
                        "memory_limit": if self.memory_limit == usize::MAX { -1i64 } else { self.memory_limit as i64 },
                        "gc_threshold": self.gc_threshold,
                        "gc_live_count": self.allocated_bytes.saturating_div(64),
                        "active_tasks": active_tasks,
                        "status": "healthy"
                    });
                    let success = std::fs::write(&path, serde_json::to_string_pretty(&dump).unwrap_or_default()).is_ok();
                    stack.push(Value::Bool(success));
                }
                Op::JoinTask => {
                    let Value::Task(task_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("join requires a task".into())); };
                    let record = self.runtime.tasks.lock().map_err(|_| VmError::Type("task registry poisoned".into()))?.remove(&task_id).ok_or(VmError::UnknownTask(task_id))?;
                    let result = record.result.recv().map_err(|_| VmError::TaskPanicked(task_id))?; record.handle.join().map_err(|_| VmError::TaskPanicked(task_id))?; stack.push(result?);
                }
                Op::JoinTaskTimeout => {
                    let timeout = timeout_value(pop(&mut stack, &function.name)?)?; let Value::Task(task_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("join_timeout requires a task".into())); };
                    let record = self.runtime.tasks.lock().map_err(|_| VmError::Type("task registry poisoned".into()))?.remove(&task_id).ok_or(VmError::UnknownTask(task_id))?;
                    match record.result.recv_timeout(timeout) { Ok(result) => { record.handle.join().map_err(|_| VmError::TaskPanicked(task_id))?; stack.push(Value::Enum { name: "Option".into(), variant: "Some".into(), payload: Some(Box::new(result?)) }); } Err(mpsc::RecvTimeoutError::Timeout) => { self.runtime.tasks.lock().map_err(|_| VmError::Type("task registry poisoned".into()))?.insert(task_id, record); stack.push(Value::Enum { name: "Option".into(), variant: "None".into(), payload: None }); } Err(mpsc::RecvTimeoutError::Disconnected) => return Err(VmError::TaskPanicked(task_id)) }
                }
                Op::CancelTask => {
                    let Value::Task(task_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("cancel requires a task".into())); };
                    let cancelled = self.runtime.tasks.lock().map_err(|_| VmError::Type("task registry poisoned".into()))?.get(&task_id).map(|record| { record.cancelled.store(true, Ordering::Release); true }).unwrap_or(false); stack.push(Value::Bool(cancelled));
                }
                Op::NewChannel => {
                    let capacity = pop(&mut stack, &function.name)?; let Value::Int(capacity) = capacity else { return Err(VmError::Type("channel capacity must be int".into())); };
                    let capacity = usize::try_from(capacity).map_err(|_| VmError::Type("channel capacity must be nonnegative".into()))?;
                    let channel_id = self.runtime.next_channel.fetch_add(1, Ordering::Relaxed); let (sender, receiver) = mpsc::sync_channel(capacity);
                    self.runtime.channels.lock().map_err(|_| VmError::Type("channel registry poisoned".into()))?.insert(channel_id, Arc::new(Channel { sender, receiver: Mutex::new(receiver) }));
                    stack.push(Value::Tuple(vec![Value::ChannelSender(channel_id), Value::ChannelReceiver(channel_id)]));
                }
                Op::ChannelSend => {
                    let value = pop(&mut stack, &function.name)?; let Value::ChannelSender(channel_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("send requires a channel sender".into())); };
                    let channel = self.runtime.channels.lock().map_err(|_| VmError::Type("channel registry poisoned".into()))?.get(&channel_id).cloned().ok_or(VmError::UnknownChannel(channel_id))?;
                    channel.sender.send(value).map_err(|_| VmError::ChannelDisconnected(channel_id))?; stack.push(Value::Nil);
                }
                Op::ChannelRecv => {
                    let Value::ChannelReceiver(channel_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("recv requires a channel receiver".into())); };
                    let channel = self.runtime.channels.lock().map_err(|_| VmError::Type("channel registry poisoned".into()))?.get(&channel_id).cloned().ok_or(VmError::UnknownChannel(channel_id))?;
                    let value = channel.receiver.lock().map_err(|_| VmError::Type("channel receiver poisoned".into()))?.recv().map_err(|_| VmError::ChannelDisconnected(channel_id))?; stack.push(value);
                }
                Op::ChannelRecvTimeout => {
                    let timeout = timeout_value(pop(&mut stack, &function.name)?)?; let Value::ChannelReceiver(channel_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("recv_timeout requires a receiver".into())); };
                    let channel = self.runtime.channels.lock().map_err(|_| VmError::Type("channel registry poisoned".into()))?.get(&channel_id).cloned().ok_or(VmError::UnknownChannel(channel_id))?;
                    let result = channel.receiver.lock().map_err(|_| VmError::Type("channel receiver poisoned".into()))?.recv_timeout(timeout);
                    match result { Ok(value) => stack.push(option_some(value)), Err(mpsc::RecvTimeoutError::Timeout) => stack.push(option_none()), Err(mpsc::RecvTimeoutError::Disconnected) => return Err(VmError::ChannelDisconnected(channel_id)) }
                }
                Op::ChannelSelect => {
                    let timeout = timeout_value(pop(&mut stack, &function.name)?)?;
                    let receivers = array_value(pop(&mut stack, &function.name)?)?;
                    let deadline = Instant::now() + timeout;
                    let ids: Vec<u64> = receivers
                        .into_iter()
                        .map(|value| match value {
                            Value::ChannelReceiver(id) => Ok(id),
                            _ => Err(VmError::Type("select requires an array of receivers".into())),
                        })
                        .collect::<Result<_, _>>()?;

                    'select: loop {
                        for (index, channel_id) in ids.iter().enumerate() {
                            let channel = self
                                .runtime
                                .channels
                                .lock()
                                .map_err(|_| VmError::Type("channel registry poisoned".into()))?
                                .get(channel_id)
                                .cloned()
                                .ok_or(VmError::UnknownChannel(*channel_id))?;
                            let received = {
                                let receiver = channel
                                    .receiver
                                    .lock()
                                    .map_err(|_| VmError::Type("channel receiver poisoned".into()))?;
                                receiver.try_recv()
                            };
                            match received {
                                Ok(value) => {
                                    stack.push(option_some(Value::Tuple(vec![
                                        Value::Int(index as i64),
                                        value,
                                    ])));
                                    break 'select;
                                }
                                Err(mpsc::TryRecvError::Disconnected) => {
                                    return Err(VmError::ChannelDisconnected(*channel_id));
                                }
                                Err(mpsc::TryRecvError::Empty) => {}
                            }
                        }
                        if Instant::now() >= deadline {
                            stack.push(option_none());
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                Op::TcpListen => {
                    require_network(self.capabilities, "std::net::tcp_listen")?;
                    let Value::Str(address) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("tcp_listen requires an address string".into())); };
                    let listener = TcpListener::bind(&address).map_err(|error| network_error("std::net::tcp_listen", error))?;
                    let id = self.runtime.next_socket.fetch_add(1, Ordering::Relaxed); self.runtime.listeners.lock().map_err(|_| VmError::Type("listener registry poisoned".into()))?.insert(id, Arc::new(listener)); stack.push(Value::TcpListener(id));
                }
                Op::TcpLocalAddr => {
                    require_network(self.capabilities, "std::net::tcp_local_addr")?;
                    let Value::TcpListener(listener_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("tcp_local_addr requires a listener".into())); }; let listener = self.runtime.listeners.lock().map_err(|_| VmError::Type("listener registry poisoned".into()))?.get(&listener_id).cloned().ok_or_else(|| VmError::Type("unknown TCP listener".into()))?; let address = listener.local_addr().map_err(|error| network_error("std::net::tcp_local_addr", error))?; stack.push(Value::Str(address.to_string()));
                }
                Op::TcpAccept => {
                    require_network(self.capabilities, "std::net::tcp_accept")?;
                    let Value::TcpListener(listener_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("tcp_accept requires a listener".into())); };
                    let listener = self.runtime.listeners.lock().map_err(|_| VmError::Type("listener registry poisoned".into()))?.get(&listener_id).cloned().ok_or_else(|| VmError::Type("unknown TCP listener".into()))?;
                    let (stream, peer) = listener.accept().map_err(|error| network_error("std::net::tcp_accept", error))?; let stream_id = self.runtime.next_socket.fetch_add(1, Ordering::Relaxed); self.runtime.streams.lock().map_err(|_| VmError::Type("stream registry poisoned".into()))?.insert(stream_id, Arc::new(Mutex::new(stream))); stack.push(Value::Tuple(vec![Value::TcpStream(stream_id), Value::Str(peer.to_string())]));
                }
                Op::TcpConnect => {
                    require_network(self.capabilities, "std::net::tcp_connect")?;
                    let Value::Str(address) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("tcp_connect requires an address string".into())); }; let stream = TcpStream::connect(&address).map_err(|error| network_error("std::net::tcp_connect", error))?; let id = self.runtime.next_socket.fetch_add(1, Ordering::Relaxed); self.runtime.streams.lock().map_err(|_| VmError::Type("stream registry poisoned".into()))?.insert(id, Arc::new(Mutex::new(stream))); stack.push(Value::TcpStream(id));
                }
                Op::TcpRead => {
                    require_network(self.capabilities, "std::net::tcp_read")?;
                    let maximum = pop(&mut stack, &function.name)?; let Value::Int(maximum) = maximum else { return Err(VmError::Type("tcp_read maximum must be int".into())); }; let maximum = usize::try_from(maximum).map_err(|_| VmError::Type("tcp_read maximum must be nonnegative".into()))?.min(16 * 1024 * 1024); let Value::TcpStream(stream_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("tcp_read requires a stream".into())); }; let stream = socket_stream(&self.runtime, stream_id)?; let mut buffer = vec![0; maximum]; let count = stream.lock().map_err(|_| VmError::Type("TCP stream poisoned".into()))?.read(&mut buffer).map_err(|error| network_error("std::net::tcp_read", error))?; buffer.truncate(count); stack.push(Value::Bytes(buffer));
                }
                Op::TcpWrite => {
                    require_network(self.capabilities, "std::net::tcp_write")?;
                    let data = pop(&mut stack, &function.name)?; let data = match data { Value::Bytes(data) => data, Value::Str(data) => data.into_bytes(), _ => return Err(VmError::Type("tcp_write requires bytes or string".into())) }; let Value::TcpStream(stream_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("tcp_write requires a stream".into())); }; let stream = socket_stream(&self.runtime, stream_id)?; stream.lock().map_err(|_| VmError::Type("TCP stream poisoned".into()))?.write_all(&data).map_err(|error| network_error("std::net::tcp_write", error))?; stack.push(Value::Int(data.len() as i64));
                }
                Op::TcpSetTimeout => {
                    require_network(self.capabilities, "std::net::tcp_set_timeout")?;
                    let timeout = timeout_value(pop(&mut stack, &function.name)?)?; let Value::TcpStream(stream_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("tcp_set_timeout requires a stream".into())); }; let stream = socket_stream(&self.runtime, stream_id)?; let stream = stream.lock().map_err(|_| VmError::Type("TCP stream poisoned".into()))?; stream.set_read_timeout(Some(timeout)).and_then(|_| stream.set_write_timeout(Some(timeout))).map_err(|error| network_error("std::net::tcp_set_timeout", error))?; stack.push(Value::Nil);
                }
                Op::TcpClose => {
                    require_network(self.capabilities, "std::net::tcp_close")?;
                    let removed = match pop(&mut stack, &function.name)? { Value::TcpStream(id) => self.runtime.streams.lock().map_err(|_| VmError::Type("stream registry poisoned".into()))?.remove(&id).is_some(), Value::TcpListener(id) => self.runtime.listeners.lock().map_err(|_| VmError::Type("listener registry poisoned".into()))?.remove(&id).is_some(), _ => return Err(VmError::Type("tcp_close requires a stream or listener".into())) }; stack.push(Value::Bool(removed));
                }
                Op::HttpServeConnection => {
                    require_network(self.capabilities, "std::http::serve_connection")?;
                    let maximum = pop(&mut stack, &function.name)?; let Value::Int(maximum) = maximum else { return Err(VmError::Type("maximum requests must be int".into())); }; let maximum = usize::try_from(maximum).map_err(|_| VmError::Type("maximum requests must be positive".into()))?; if maximum == 0 || maximum > 10_000 { return Err(VmError::Type("maximum requests must be between 1 and 10000".into())); }
                    let handler = pop(&mut stack, &function.name)?; let Value::Closure { function: handler_function, captures: handler_captures } = handler else { return Err(VmError::Type("HTTP handler must be a closure".into())); }; let Value::TcpListener(listener_id) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("serve_connection requires a listener".into())); };
                    let listener = self.runtime.listeners.lock().map_err(|_| VmError::Type("listener registry poisoned".into()))?.get(&listener_id).cloned().ok_or_else(|| VmError::Type("unknown TCP listener".into()))?; let (mut stream, peer) = listener.accept().map_err(|error| network_error("std::http::serve_connection", error))?; let mut buffer = Vec::new();
                    'requests: for _ in 0..maximum { let request = loop { if let Some((request, consumed)) = titan_stdlib::http::parse_request(&buffer, &titan_stdlib::http::HttpLimits::default()).map_err(|error| VmError::Native { function: "std::http::serve_connection".into(), message: error.to_string() })? { buffer.drain(..consumed); break request; } let mut chunk = [0u8; 8192]; let count = stream.read(&mut chunk).map_err(|error| network_error("std::http::serve_connection", error))?; if count == 0 { break 'requests; } buffer.extend_from_slice(&chunk[..count]); if buffer.len() > 16 * 1024 * 1024 + 64 * 1024 { return Err(VmError::Type("HTTP connection buffer limit exceeded".into())); } };
                        let request_keep_alive = request.keep_alive; let response_value = self.execute(handler_function, vec![http_request_value(request, &peer.to_string())], handler_captures.clone(), depth + 1, debugger)?; let response_parts = http_response_value(response_value, request_keep_alive)?; let response = titan_stdlib::http::build_response(response_parts.status, &response_parts.headers, &response_parts.body, response_parts.keep_alive).map_err(|error| VmError::Native { function: "std::http::serve_connection".into(), message: error.to_string() })?; stream.write_all(&response).map_err(|error| network_error("std::http::serve_connection", error))?; if !response_parts.keep_alive { break; }
                    }
                    stack.push(Value::Str(peer.to_string()));
                }
                Op::HttpRouterNew => { let id=self.runtime.next_router.fetch_add(1,Ordering::Relaxed);self.runtime.routers.lock().map_err(|_|VmError::Type("router registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(HttpRouterState::default())));stack.push(Value::HttpRouter(id)); }
                Op::HttpRouteAdd => { let handler=pop(&mut stack,&function.name)?;let Value::Closure{function:handler_function,captures}=handler else{return Err(VmError::Type("route handler must be closure".into()))};let Value::Str(pattern)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("route pattern must be string".into()))};let Value::Str(method)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("route method must be string".into()))};let Value::HttpRouter(router_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("route requires router".into()))};titan_stdlib::http::validate_route_pattern(&pattern).map_err(|error|VmError::Native{function:"std::http::route".into(),message:error.to_string()})?;let router=http_router(&self.runtime,router_id)?;router.lock().map_err(|_|VmError::Type("HTTP router poisoned".into()))?.routes.push(HttpRoute{method:method.to_ascii_uppercase(),pattern,handler:HttpCallable{function:handler_function,captures}});stack.push(Value::Nil); }
                Op::HttpMiddlewareAdd => { let middleware=pop(&mut stack,&function.name)?;let Value::Closure{function:middleware_function,captures}=middleware else{return Err(VmError::Type("middleware must be closure".into()))};let Value::HttpRouter(router_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("middleware requires router".into()))};let router=http_router(&self.runtime,router_id)?;router.lock().map_err(|_|VmError::Type("HTTP router poisoned".into()))?.middleware.push(HttpCallable{function:middleware_function,captures});stack.push(Value::Nil); }
                Op::HttpAfterAdd => { let middleware=pop(&mut stack,&function.name)?;let Value::Closure{function:middleware_function,captures}=middleware else{return Err(VmError::Type("response middleware must be closure".into()))};let Value::HttpRouter(router_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("after requires router".into()))};let router=http_router(&self.runtime,router_id)?;router.lock().map_err(|_|VmError::Type("HTTP router poisoned".into()))?.after.push(HttpCallable{function:middleware_function,captures});stack.push(Value::Nil); }
                Op::HttpErrorHandlerAdd => { let handler=pop(&mut stack,&function.name)?;let Value::Closure{function:handler_function,captures}=handler else{return Err(VmError::Type("error handler must be closure".into()))};let Value::HttpRouter(router_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("on_error requires router".into()))};let router=http_router(&self.runtime,router_id)?;router.lock().map_err(|_|VmError::Type("HTTP router poisoned".into()))?.error_handler=Some(HttpCallable{function:handler_function,captures});stack.push(Value::Nil); }
                Op::HttpDispatch => { let dispatch_started=Instant::now();let mut request=pop(&mut stack,&function.name)?;let Value::HttpRouter(router_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("dispatch requires router".into()))};let router=http_router(&self.runtime,router_id)?;let (middleware,routes,after,error_handler)={let router=router.lock().map_err(|_|VmError::Type("HTTP router poisoned".into()))?;(router.middleware.clone(),router.routes.clone(),router.after.clone(),router.error_handler.clone())};for layer in middleware{request=self.execute(layer.function,vec![request],layer.captures,depth+1,debugger)?;if !matches!(&request,Value::Map(_)){return Err(VmError::Type("middleware must return request map".into()))}}let (method,path)=request_method_path(&request)?;let mut matched=None;for route in routes{if route.method==method{if let Some(params)=titan_stdlib::http::match_route(&route.pattern,&path).map_err(|error|VmError::Native{function:"std::http::dispatch".into(),message:error.to_string()})?{matched=Some((route,params));break}}}let request_for_error=request.clone();let handler_result=if let Some((route,params))=matched{if let Value::Map(map)=&mut request{map.insert("params".into(),Value::Map(params.into_iter().map(|(key,value)|(key,Value::Str(value))).collect()));}self.execute(route.handler.function,vec![request],route.handler.captures,depth+1,debugger)}else{Ok(http_not_found())};let mut response=match handler_result{Ok(response)=>response,Err(error)=>{let Some(handler)=error_handler else{return Err(error)};self.execute(handler.function,vec![request_for_error,http_error_value(&error)],handler.captures,depth+1,debugger)?}};for layer in after.into_iter().rev(){response=self.execute(layer.function,vec![response],layer.captures,depth+1,debugger)?;if !matches!(&response,Value::Map(_)){return Err(VmError::Type("response middleware must return response map".into()))}}record_http_metrics(&response,dispatch_started.elapsed());stack.push(response); }
                Op::TlsConnect => { require_network(self.capabilities,"std::tls::connect")?;let Value::Str(server_name)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("TLS server name must be string".into()))};let Value::Str(address)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("TLS address must be string".into()))};let stream=titan_tls::connect(&address,&server_name,titan_tls::client_config()).map_err(|error|VmError::Native{function:"std::tls::connect".into(),message:error.to_string()})?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.tls_streams.lock().map_err(|_|VmError::Type("TLS stream registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(stream)));stack.push(Value::TlsStream(id)); }
                Op::TlsServerConfig => { require_network(self.capabilities,"std::tls::server_config")?;if !self.capabilities.filesystem{return Err(VmError::PermissionDenied{function:"std::tls::server_config".into(),capability:"Filesystem".into()})}let Value::Str(key)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("TLS key path must be string".into()))};let Value::Str(cert)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("TLS certificate path must be string".into()))};let config=titan_tls::server_config(cert,key).map_err(|error|VmError::Native{function:"std::tls::server_config".into(),message:error.to_string()})?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.tls_configs.lock().map_err(|_|VmError::Type("TLS config registry poisoned".into()))?.insert(id,config);stack.push(Value::TlsServerConfig(id)); }
                Op::TlsAccept => { require_network(self.capabilities,"std::tls::accept")?;let Value::TlsServerConfig(config_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("tls_accept requires server config".into()))};let Value::TcpListener(listener_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("tls_accept requires listener".into()))};let config=self.runtime.tls_configs.lock().map_err(|_|VmError::Type("TLS config registry poisoned".into()))?.get(&config_id).cloned().ok_or_else(||VmError::Type("unknown TLS config".into()))?;let listener=self.runtime.listeners.lock().map_err(|_|VmError::Type("listener registry poisoned".into()))?.get(&listener_id).cloned().ok_or_else(||VmError::Type("unknown TCP listener".into()))?;let(socket,peer)=listener.accept().map_err(|error|network_error("std::tls::accept",error))?;let stream=titan_tls::accept(socket,config).map_err(|error|VmError::Native{function:"std::tls::accept".into(),message:error.to_string()})?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.tls_streams.lock().map_err(|_|VmError::Type("TLS stream registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(stream)));stack.push(Value::Tuple(vec![Value::TlsStream(id),Value::Str(peer.to_string())])); }
                Op::TlsRead => { require_network(self.capabilities,"std::tls::read")?;let maximum=pop(&mut stack,&function.name)?;let Value::Int(maximum)=maximum else{return Err(VmError::Type("TLS read maximum must be int".into()))};let maximum=usize::try_from(maximum).map_err(|_|VmError::Type("TLS read maximum must be nonnegative".into()))?.min(16*1024*1024);let Value::TlsStream(stream_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("tls_read requires stream".into()))};let stream=tls_stream(&self.runtime,stream_id)?;let mut buffer=vec![0;maximum];let count=stream.lock().map_err(|_|VmError::Type("TLS stream poisoned".into()))?.read(&mut buffer).map_err(|error|network_error("std::tls::read",error))?;buffer.truncate(count);stack.push(Value::Bytes(buffer)); }
                Op::TlsWrite => { require_network(self.capabilities,"std::tls::write")?;let data=pop(&mut stack,&function.name)?;let data=match data{Value::Bytes(data)=>data,Value::Str(data)=>data.into_bytes(),_=>return Err(VmError::Type("tls_write requires bytes or string".into()))};let Value::TlsStream(stream_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("tls_write requires stream".into()))};let stream=tls_stream(&self.runtime,stream_id)?;stream.lock().map_err(|_|VmError::Type("TLS stream poisoned".into()))?.write_all(&data).map_err(|error|network_error("std::tls::write",error))?;stack.push(Value::Int(data.len() as i64)); }
                Op::TlsClose => { require_network(self.capabilities,"std::tls::close")?;let Value::TlsStream(id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("tls_close requires stream".into()))};stack.push(Value::Bool(self.runtime.tls_streams.lock().map_err(|_|VmError::Type("TLS stream registry poisoned".into()))?.remove(&id).is_some())); }
                Op::WsDecoderNew => { let maximum=pop(&mut stack,&function.name)?;let Value::Int(maximum)=maximum else{return Err(VmError::Type("WebSocket maximum must be int".into()))};let maximum=usize::try_from(maximum).map_err(|_|VmError::Type("WebSocket maximum must be nonnegative".into()))?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.websocket_decoders.lock().map_err(|_|VmError::Type("WebSocket decoder registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(titan_stdlib::websocket::MessageDecoder::new(maximum))));stack.push(Value::WebSocketDecoder(id)); }
                Op::WsDecoderPush => { let data=pop(&mut stack,&function.name)?;let data=match data{Value::Bytes(data)=>data,_=>return Err(VmError::Type("decoder_push requires bytes".into()))};let Value::WebSocketDecoder(id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("decoder_push requires decoder".into()))};let decoder=websocket_decoder(&self.runtime,id)?;decoder.lock().map_err(|_|VmError::Type("WebSocket decoder poisoned".into()))?.push(&data).map_err(|error|VmError::Native{function:"std::ws::decoder_push".into(),message:error.to_string()})?;stack.push(Value::Nil); }
                Op::WsDecoderNext => { let require=pop(&mut stack,&function.name)?;let Value::Bool(require)=require else{return Err(VmError::Type("decoder_next mask policy must be bool".into()))};let Value::WebSocketDecoder(id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("decoder_next requires decoder".into()))};let decoder=websocket_decoder(&self.runtime,id)?;let message=decoder.lock().map_err(|_|VmError::Type("WebSocket decoder poisoned".into()))?.next(Some(require)).map_err(|error|VmError::Native{function:"std::ws::decoder_next".into(),message:error.to_string()})?;stack.push(message.map(websocket_message_value).map(option_some).unwrap_or_else(option_none)); }
                Op::WsConnect => {
                    require_network(self.capabilities, "std::ws::connect")?;
                    let maximum = positive_limit(pop(&mut stack, &function.name)?, "WebSocket maximum")?;
                    let Value::Str(protocol) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("WebSocket protocol must be string".into())); };
                    let Value::Str(url) = pop(&mut stack, &function.name)? else { return Err(VmError::Type("WebSocket URL must be string".into())); };
                    let id = websocket_connect(&self.runtime, &url, &protocol, maximum)?;
                    stack.push(Value::WebSocket(id));
                }
                Op::WsAttachTcp => { let maximum=positive_limit(pop(&mut stack,&function.name)?,"WebSocket maximum")?;let Value::Bool(server_side)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("WebSocket server_side must be bool".into()))};let Value::TcpStream(stream_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("attach_tcp requires TCP stream".into()))};let transport=WebSocketTransport::Tcp(take_socket_stream(&self.runtime,stream_id)?);let id=insert_websocket(&self.runtime,transport,server_side,maximum)?;stack.push(Value::WebSocket(id)); }
                Op::WsAttachTls => { let maximum=positive_limit(pop(&mut stack,&function.name)?,"WebSocket maximum")?;let Value::Bool(server_side)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("WebSocket server_side must be bool".into()))};let Value::TlsStream(stream_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("attach_tls requires TLS stream".into()))};let transport=WebSocketTransport::Tls(take_tls_stream(&self.runtime,stream_id)?);let id=insert_websocket(&self.runtime,transport,server_side,maximum)?;stack.push(Value::WebSocket(id)); }
                Op::WsSendText => { let Value::Str(text)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("send_text requires string".into()))};let socket=pop_websocket(&self.runtime,pop(&mut stack,&function.name)?)?;websocket_send(&socket,1,text.as_bytes())?;stack.push(Value::Nil); }
                Op::WsSendBinary => { let Value::Bytes(data)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("send_binary requires bytes".into()))};let socket=pop_websocket(&self.runtime,pop(&mut stack,&function.name)?)?;websocket_send(&socket,2,&data)?;stack.push(Value::Nil); }
                Op::WsReceive => { let socket_value=pop(&mut stack,&function.name)?;let Value::WebSocket(socket_id)=socket_value else{return Err(VmError::Type("receive requires WebSocket".into()))};let socket=pop_websocket(&self.runtime,Value::WebSocket(socket_id))?;let message=websocket_receive(&socket)?;let closed=matches!(&message,titan_stdlib::websocket::Message::Close{..});stack.push(websocket_message_value(message));if closed{self.runtime.websockets.lock().map_err(|_|VmError::Type("WebSocket registry poisoned".into()))?.remove(&socket_id);} }
                Op::WsClose => { let Value::Str(reason)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("close reason must be string".into()))};let code=pop(&mut stack,&function.name)?;let Value::Int(code)=code else{return Err(VmError::Type("close code must be int".into()))};let code=u16::try_from(code).map_err(|_|VmError::Type("close code out of range".into()))?;let socket_value=pop(&mut stack,&function.name)?;let Value::WebSocket(socket_id)=socket_value else{return Err(VmError::Type("close requires WebSocket".into()))};let socket=pop_websocket(&self.runtime,Value::WebSocket(socket_id))?;websocket_close(&socket,code,&reason)?;self.runtime.websockets.lock().map_err(|_|VmError::Type("WebSocket registry poisoned".into()))?.remove(&socket_id);stack.push(Value::Nil); }
                Op::ServerControlNew => { let maximum=positive_limit(pop(&mut stack,&function.name)?,"maximum connections")? as u64;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.server_controls.lock().map_err(|_|VmError::Type("server control registry poisoned".into()))?.insert(id,Arc::new(ServerControl{maximum,active:AtomicU64::new(0),accepted:AtomicU64::new(0),rejected:AtomicU64::new(0),completed:AtomicU64::new(0),shutting_down:AtomicBool::new(false)}));stack.push(Value::ServerControl(id)); }
                Op::ServerTryAcquire => { let control=server_control(&self.runtime,pop(&mut stack,&function.name)?)?;let acquired=server_try_acquire(&control);stack.push(Value::Bool(acquired)); }
                Op::ServerRelease => { let control=server_control(&self.runtime,pop(&mut stack,&function.name)?)?;let released=control.active.fetch_update(Ordering::AcqRel,Ordering::Acquire,|active|(active>0).then_some(active-1)).is_ok();if released{control.completed.fetch_add(1,Ordering::Relaxed);}stack.push(Value::Bool(released)); }
                Op::ServerShutdown => { let control=server_control(&self.runtime,pop(&mut stack,&function.name)?)?;control.shutting_down.store(true,Ordering::Release);stack.push(Value::Nil); }
                Op::ServerStats => { let control=server_control(&self.runtime,pop(&mut stack,&function.name)?)?;stack.push(server_stats(&control)); }
                Op::ServerHealthResponse => { let control=server_control(&self.runtime,pop(&mut stack,&function.name)?)?;stack.push(server_health_response(&control)); }
                Op::SqliteOpen => { if !self.capabilities.filesystem{return Err(VmError::PermissionDenied{function:"std::sqlite::open".into(),capability:"Filesystem".into()})}let Value::Str(path)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("SQLite path must be string".into()))};let database=titan_sqlite::Database::open(path).map_err(sqlite_error)?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.sqlite.lock().map_err(|_|VmError::Type("SQLite registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(DatabaseHandle::Direct(database))));stack.push(Value::Sqlite(id)); }
                Op::SqliteMemory => { let database=titan_sqlite::Database::memory().map_err(sqlite_error)?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.sqlite.lock().map_err(|_|VmError::Type("SQLite registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(DatabaseHandle::Direct(database))));stack.push(Value::Sqlite(id)); }
                operation @ (Op::SqliteExecute|Op::SqliteQuery) => { let params=sqlite_params(pop(&mut stack,&function.name)?)?;let Value::Str(sql)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("SQL must be string".into()))};let database=sqlite_database(&self.runtime,pop(&mut stack,&function.name)?)?;let mut database=database.lock().map_err(|_|VmError::Type("SQLite connection poisoned".into()))?;if matches!(operation,Op::SqliteExecute){stack.push(Value::Int(database.execute(&sql,&params).map_err(sqlite_error)? as i64));}else{let rows=database.query(&sql,&params).map_err(sqlite_error)?;stack.push(Value::Array(rows.into_iter().map(sqlite_row).collect()));} }
                operation @ (Op::SqliteBegin|Op::SqliteCommit|Op::SqliteRollback) => { let database=sqlite_database(&self.runtime,pop(&mut stack,&function.name)?)?;let mut database=database.lock().map_err(|_|VmError::Type("SQLite connection poisoned".into()))?;match operation{Op::SqliteBegin=>database.begin(),Op::SqliteCommit=>database.commit(),_=>database.rollback()}.map_err(sqlite_error)?;stack.push(Value::Nil); }
                Op::SqliteMigrate => { let migrations=sqlite_migrations(pop(&mut stack,&function.name)?)?;let database=sqlite_database(&self.runtime,pop(&mut stack,&function.name)?)?;let count=database.lock().map_err(|_|VmError::Type("SQLite connection poisoned".into()))?.migrate(&migrations).map_err(sqlite_error)?;stack.push(Value::Int(count as i64)); }
                Op::SqliteLastId => { let database=sqlite_database(&self.runtime,pop(&mut stack,&function.name)?)?;stack.push(Value::Int(database.lock().map_err(|_|VmError::Type("SQLite connection poisoned".into()))?.last_insert_id())); }
                Op::SqliteClose => { let Value::Sqlite(id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("close requires SQLite connection".into()))};stack.push(Value::Bool(self.runtime.sqlite.lock().map_err(|_|VmError::Type("SQLite registry poisoned".into()))?.remove(&id).is_some())); }
                Op::SqlitePing => { let database=sqlite_database(&self.runtime,pop(&mut stack,&function.name)?)?;stack.push(Value::Bool(database.lock().map_err(|_|VmError::Type("SQLite connection poisoned".into()))?.ping().unwrap_or(false))); }
                Op::SqlitePoolNew => { if !self.capabilities.filesystem{return Err(VmError::PermissionDenied{function:"std::sqlite::pool".into(),capability:"Filesystem".into()})}let maximum=positive_limit(pop(&mut stack,&function.name)?,"SQLite pool maximum")?;let Value::Str(path)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("SQLite pool path must be string".into()))};let pool=titan_sqlite::Pool::new(path,maximum).map_err(sqlite_error)?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.sqlite_pools.lock().map_err(|_|VmError::Type("SQLite pool registry poisoned".into()))?.insert(id,pool);stack.push(Value::SqlitePool(id)); }
                Op::SqlitePoolAcquire => { let timeout=timeout_value(pop(&mut stack,&function.name)?)?;let pool=sqlite_pool(&self.runtime,pop(&mut stack,&function.name)?)?;match pool.acquire(timeout){Ok(connection)=>{let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.sqlite.lock().map_err(|_|VmError::Type("SQLite registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(DatabaseHandle::Pooled(connection))));stack.push(option_some(Value::Sqlite(id)));}Err(titan_sqlite::DbError::PoolTimeout)=>stack.push(option_none()),Err(error)=>return Err(sqlite_error(error))} }
                Op::SqlitePoolStats => { let pool=sqlite_pool(&self.runtime,pop(&mut stack,&function.name)?)?;let stats=pool.stats().map_err(sqlite_error)?;stack.push(Value::Map(BTreeMap::from([("maximum".into(),Value::Int(stats.maximum as i64)),("total".into(),Value::Int(stats.total as i64)),("idle".into(),Value::Int(stats.idle as i64)),("checked_out".into(),Value::Int(stats.checked_out as i64)),("closed".into(),Value::Bool(stats.closed))]))); }
                Op::SqlitePoolHealth => { let timeout=timeout_value(pop(&mut stack,&function.name)?)?;let pool=sqlite_pool(&self.runtime,pop(&mut stack,&function.name)?)?;stack.push(Value::Bool(pool.health_check(timeout))); }
                Op::SqlitePoolClose => { let Value::SqlitePool(id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("pool_close requires SQLite pool".into()))};let pool=self.runtime.sqlite_pools.lock().map_err(|_|VmError::Type("SQLite pool registry poisoned".into()))?.remove(&id).ok_or_else(||VmError::Type("unknown SQLite pool".into()))?;pool.close().map_err(sqlite_error)?;stack.push(Value::Nil); }
                operation @ (Op::PostgresConnect|Op::PostgresConnectTls) => { require_network(self.capabilities,"std::postgres::connect")?;let Value::Str(url)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("PostgreSQL URL must be string".into()))};let database=if matches!(operation,Op::PostgresConnectTls){titan_postgres::Database::connect_tls(&url)}else{titan_postgres::Database::connect(&url)}.map_err(postgres_error)?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.postgres.lock().map_err(|_|VmError::Type("PostgreSQL registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(PostgresHandle::Direct(database))));stack.push(Value::Postgres(id)); }
                operation @ (Op::PostgresExecute|Op::PostgresQuery) => { let params=postgres_params(pop(&mut stack,&function.name)?)?;let Value::Str(sql)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("SQL must be string".into()))};let database=postgres_database(&self.runtime,pop(&mut stack,&function.name)?)?;let mut database=database.lock().map_err(|_|VmError::Type("PostgreSQL connection poisoned".into()))?;if matches!(operation,Op::PostgresExecute){stack.push(Value::Int(i64::try_from(database.execute(&sql,&params).map_err(postgres_error)?).unwrap_or(i64::MAX)));}else{stack.push(Value::Array(database.query(&sql,&params).map_err(postgres_error)?.into_iter().map(postgres_row).collect()));} }
                operation @ (Op::PostgresBegin|Op::PostgresCommit|Op::PostgresRollback) => { let database=postgres_database(&self.runtime,pop(&mut stack,&function.name)?)?;let mut database=database.lock().map_err(|_|VmError::Type("PostgreSQL connection poisoned".into()))?;match operation{Op::PostgresBegin=>database.begin(),Op::PostgresCommit=>database.commit(),_=>database.rollback()}.map_err(postgres_error)?;stack.push(Value::Nil); }
                Op::PostgresMigrate => { let migrations=postgres_migrations(pop(&mut stack,&function.name)?)?;let database=postgres_database(&self.runtime,pop(&mut stack,&function.name)?)?;let count=database.lock().map_err(|_|VmError::Type("PostgreSQL connection poisoned".into()))?.migrate(&migrations).map_err(postgres_error)?;stack.push(Value::Int(count as i64)); }
                Op::PostgresCancel => { let database=postgres_database(&self.runtime,pop(&mut stack,&function.name)?)?;database.lock().map_err(|_|VmError::Type("PostgreSQL connection poisoned".into()))?.cancel().map_err(postgres_error)?;stack.push(Value::Nil); }
                Op::PostgresClose => { let Value::Postgres(id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("close requires PostgreSQL connection".into()))};stack.push(Value::Bool(self.runtime.postgres.lock().map_err(|_|VmError::Type("PostgreSQL registry poisoned".into()))?.remove(&id).is_some())); }
                Op::PostgresPing => { let database=postgres_database(&self.runtime,pop(&mut stack,&function.name)?)?;stack.push(Value::Bool(database.lock().map_err(|_|VmError::Type("PostgreSQL connection poisoned".into()))?.ping().unwrap_or(false))); }
                Op::PostgresPoolNew => { require_network(self.capabilities,"std::postgres::pool")?;let Value::Bool(tls)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("PostgreSQL pool tls must be bool".into()))};let maximum=positive_limit(pop(&mut stack,&function.name)?,"PostgreSQL pool maximum")?;let Value::Str(url)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("PostgreSQL pool URL must be string".into()))};let pool=titan_postgres::Pool::new(url,maximum,tls).map_err(postgres_error)?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.postgres_pools.lock().map_err(|_|VmError::Type("PostgreSQL pool registry poisoned".into()))?.insert(id,pool);stack.push(Value::PostgresPool(id)); }
                Op::PostgresPoolAcquire => { let timeout=timeout_value(pop(&mut stack,&function.name)?)?;let pool=postgres_pool(&self.runtime,pop(&mut stack,&function.name)?)?;match pool.acquire(timeout){Ok(connection)=>{let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.postgres.lock().map_err(|_|VmError::Type("PostgreSQL registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(PostgresHandle::Pooled(connection))));stack.push(option_some(Value::Postgres(id)));}Err(titan_postgres::PgError::PoolTimeout)=>stack.push(option_none()),Err(error)=>return Err(postgres_error(error))} }
                Op::PostgresPoolStats => { let stats=postgres_pool(&self.runtime,pop(&mut stack,&function.name)?)?.stats().map_err(postgres_error)?;stack.push(Value::Map(BTreeMap::from([("maximum".into(),Value::Int(stats.maximum as i64)),("total".into(),Value::Int(stats.total as i64)),("idle".into(),Value::Int(stats.idle as i64)),("checked_out".into(),Value::Int(stats.checked_out as i64)),("closed".into(),Value::Bool(stats.closed)),("tls".into(),Value::Bool(stats.tls))]))); }
                Op::PostgresPoolHealth => { let timeout=timeout_value(pop(&mut stack,&function.name)?)?;let pool=postgres_pool(&self.runtime,pop(&mut stack,&function.name)?)?;stack.push(Value::Bool(pool.health_check(timeout))); }
                Op::PostgresPoolClose => { let Value::PostgresPool(id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("pool_close requires PostgreSQL pool".into()))};let pool=self.runtime.postgres_pools.lock().map_err(|_|VmError::Type("PostgreSQL pool registry poisoned".into()))?.remove(&id).ok_or_else(||VmError::Type("unknown PostgreSQL pool".into()))?;pool.close().map_err(postgres_error)?;stack.push(Value::Nil); }
                Op::MysqlConnect => { require_network(self.capabilities,"std::mysql::connect")?;let Value::Str(url)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("MySQL URL must be string".into()))};let database=titan_mysql::Database::connect(&url).map_err(mysql_error)?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.mysql.lock().map_err(|_|VmError::Type("MySQL registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(MysqlHandle::Direct(database))));stack.push(Value::Mysql(id)); }
                operation @ (Op::MysqlExecute|Op::MysqlQuery) => { let params=mysql_params(pop(&mut stack,&function.name)?)?;let Value::Str(sql)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("SQL must be string".into()))};let database=mysql_database(&self.runtime,pop(&mut stack,&function.name)?)?;let mut database=database.lock().map_err(|_|VmError::Type("MySQL connection poisoned".into()))?;if matches!(operation,Op::MysqlExecute){stack.push(Value::Int(i64::try_from(database.execute(&sql,&params).map_err(mysql_error)?).unwrap_or(i64::MAX)));}else{stack.push(Value::Array(database.query(&sql,&params).map_err(mysql_error)?.into_iter().map(mysql_row).collect()));} }
                operation @ (Op::MysqlBegin|Op::MysqlCommit|Op::MysqlRollback) => { let database=mysql_database(&self.runtime,pop(&mut stack,&function.name)?)?;let mut database=database.lock().map_err(|_|VmError::Type("MySQL connection poisoned".into()))?;match operation{Op::MysqlBegin=>database.begin(),Op::MysqlCommit=>database.commit(),_=>database.rollback()}.map_err(mysql_error)?;stack.push(Value::Nil); }
                Op::MysqlMigrate => { let migrations=mysql_migrations(pop(&mut stack,&function.name)?)?;let database=mysql_database(&self.runtime,pop(&mut stack,&function.name)?)?;let count=database.lock().map_err(|_|VmError::Type("MySQL connection poisoned".into()))?.migrate(&migrations).map_err(mysql_error)?;stack.push(Value::Int(count as i64)); }
                Op::MysqlLastId => { let database=mysql_database(&self.runtime,pop(&mut stack,&function.name)?)?;let id=database.lock().map_err(|_|VmError::Type("MySQL connection poisoned".into()))?.last_insert_id();stack.push(Value::Int(i64::try_from(id).unwrap_or(i64::MAX))); }
                Op::MysqlClose => { let Value::Mysql(id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("close requires MySQL connection".into()))};stack.push(Value::Bool(self.runtime.mysql.lock().map_err(|_|VmError::Type("MySQL registry poisoned".into()))?.remove(&id).is_some())); }
                Op::MysqlPing => { let database=mysql_database(&self.runtime,pop(&mut stack,&function.name)?)?;stack.push(Value::Bool(database.lock().map_err(|_|VmError::Type("MySQL connection poisoned".into()))?.ping().unwrap_or(false))); }
                Op::MysqlPoolNew => { require_network(self.capabilities,"std::mysql::pool")?;let maximum=positive_limit(pop(&mut stack,&function.name)?,"MySQL pool maximum")?;let Value::Str(url)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("MySQL pool URL must be string".into()))};let pool=titan_mysql::Pool::new(url,maximum).map_err(mysql_error)?;let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.mysql_pools.lock().map_err(|_|VmError::Type("MySQL pool registry poisoned".into()))?.insert(id,pool);stack.push(Value::MysqlPool(id)); }
                Op::MysqlPoolAcquire => { let timeout=timeout_value(pop(&mut stack,&function.name)?)?;let pool=mysql_pool(&self.runtime,pop(&mut stack,&function.name)?)?;match pool.acquire(timeout){Ok(connection)=>{let id=self.runtime.next_socket.fetch_add(1,Ordering::Relaxed);self.runtime.mysql.lock().map_err(|_|VmError::Type("MySQL registry poisoned".into()))?.insert(id,Arc::new(Mutex::new(MysqlHandle::Pooled(connection))));stack.push(option_some(Value::Mysql(id)));}Err(titan_mysql::MyError::PoolTimeout)=>stack.push(option_none()),Err(error)=>return Err(mysql_error(error))} }
                Op::MysqlPoolStats => { let stats=mysql_pool(&self.runtime,pop(&mut stack,&function.name)?)?.stats().map_err(mysql_error)?;stack.push(Value::Map(BTreeMap::from([("maximum".into(),Value::Int(stats.maximum as i64)),("total".into(),Value::Int(stats.total as i64)),("idle".into(),Value::Int(stats.idle as i64)),("checked_out".into(),Value::Int(stats.checked_out as i64)),("closed".into(),Value::Bool(stats.closed))]))); }
                Op::MysqlPoolHealth => { let timeout=timeout_value(pop(&mut stack,&function.name)?)?;let pool=mysql_pool(&self.runtime,pop(&mut stack,&function.name)?)?;stack.push(Value::Bool(pool.health_check(timeout))); }
                Op::MysqlPoolClose => { let Value::MysqlPool(id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("pool_close requires MySQL pool".into()))};let pool=self.runtime.mysql_pools.lock().map_err(|_|VmError::Type("MySQL pool registry poisoned".into()))?.remove(&id).ok_or_else(||VmError::Type("unknown MySQL pool".into()))?;pool.close().map_err(mysql_error)?;stack.push(Value::Nil); }
                operation @ (Op::DbExecute|Op::DbQuery) => { let params=pop(&mut stack,&function.name)?;let Value::Str(sql)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("SQL must be string".into()))};let database=pop(&mut stack,&function.name)?;let query=matches!(operation,Op::DbQuery);match database{Value::Sqlite(id)=>{let db=sqlite_database(&self.runtime,Value::Sqlite(id))?;let mut db=db.lock().map_err(|_|VmError::Type("SQLite connection poisoned".into()))?;let params=sqlite_params(params)?;if query{stack.push(Value::Array(db.query(&sql,&params).map_err(sqlite_error)?.into_iter().map(sqlite_row).collect()))}else{stack.push(Value::Int(db.execute(&sql,&params).map_err(sqlite_error)? as i64))}}Value::Postgres(id)=>{let db=postgres_database(&self.runtime,Value::Postgres(id))?;let mut db=db.lock().map_err(|_|VmError::Type("PostgreSQL connection poisoned".into()))?;let params=postgres_params(params)?;if query{stack.push(Value::Array(db.query(&sql,&params).map_err(postgres_error)?.into_iter().map(postgres_row).collect()))}else{stack.push(Value::Int(i64::try_from(db.execute(&sql,&params).map_err(postgres_error)?).unwrap_or(i64::MAX)))}}Value::Mysql(id)=>{let db=mysql_database(&self.runtime,Value::Mysql(id))?;let mut db=db.lock().map_err(|_|VmError::Type("MySQL connection poisoned".into()))?;let params=mysql_params(params)?;if query{stack.push(Value::Array(db.query(&sql,&params).map_err(mysql_error)?.into_iter().map(mysql_row).collect()))}else{stack.push(Value::Int(i64::try_from(db.execute(&sql,&params).map_err(mysql_error)?).unwrap_or(i64::MAX)))}}_=>return Err(VmError::Type("std::db operation requires database connection".into()))} }
                operation @ (Op::DbBegin|Op::DbCommit|Op::DbRollback) => { let database=pop(&mut stack,&function.name)?;match database{Value::Sqlite(id)=>{let db=sqlite_database(&self.runtime,Value::Sqlite(id))?;let mut db=db.lock().map_err(|_|VmError::Type("SQLite connection poisoned".into()))?;match operation{Op::DbBegin=>db.begin(),Op::DbCommit=>db.commit(),_=>db.rollback()}.map_err(sqlite_error)?;}Value::Postgres(id)=>{let db=postgres_database(&self.runtime,Value::Postgres(id))?;let mut db=db.lock().map_err(|_|VmError::Type("PostgreSQL connection poisoned".into()))?;match operation{Op::DbBegin=>db.begin(),Op::DbCommit=>db.commit(),_=>db.rollback()}.map_err(postgres_error)?;}Value::Mysql(id)=>{let db=mysql_database(&self.runtime,Value::Mysql(id))?;let mut db=db.lock().map_err(|_|VmError::Type("MySQL connection poisoned".into()))?;match operation{Op::DbBegin=>db.begin(),Op::DbCommit=>db.commit(),_=>db.rollback()}.map_err(mysql_error)?;}_=>return Err(VmError::Type("std::db transaction requires database connection".into()))}stack.push(Value::Nil); }
                Op::DbMigrate => { let migrations=pop(&mut stack,&function.name)?;let database=pop(&mut stack,&function.name)?;let count=match database{Value::Sqlite(id)=>{let db=sqlite_database(&self.runtime,Value::Sqlite(id))?;let migrations=sqlite_migrations(migrations)?;let result=db.lock().map_err(|_|VmError::Type("SQLite connection poisoned".into()))?.migrate(&migrations).map_err(sqlite_error)?;result}Value::Postgres(id)=>{let db=postgres_database(&self.runtime,Value::Postgres(id))?;let migrations=postgres_migrations(migrations)?;let result=db.lock().map_err(|_|VmError::Type("PostgreSQL connection poisoned".into()))?.migrate(&migrations).map_err(postgres_error)?;result}Value::Mysql(id)=>{let db=mysql_database(&self.runtime,Value::Mysql(id))?;let migrations=mysql_migrations(migrations)?;let result=db.lock().map_err(|_|VmError::Type("MySQL connection poisoned".into()))?.migrate(&migrations).map_err(mysql_error)?;result}_=>return Err(VmError::Type("std::db migrate requires database connection".into()))};stack.push(Value::Int(count as i64)); }
                Op::DbClose => { let database=pop(&mut stack,&function.name)?;let closed=match database{Value::Sqlite(id)=>self.runtime.sqlite.lock().map_err(|_|VmError::Type("SQLite registry poisoned".into()))?.remove(&id).is_some(),Value::Postgres(id)=>self.runtime.postgres.lock().map_err(|_|VmError::Type("PostgreSQL registry poisoned".into()))?.remove(&id).is_some(),Value::Mysql(id)=>self.runtime.mysql.lock().map_err(|_|VmError::Type("MySQL registry poisoned".into()))?.remove(&id).is_some(),_=>return Err(VmError::Type("std::db close requires database connection".into()))};stack.push(Value::Bool(closed)); }
                Op::DbPing => { let database=pop(&mut stack,&function.name)?;let healthy=match database{Value::Sqlite(id)=>sqlite_database(&self.runtime,Value::Sqlite(id))?.lock().map_err(|_|VmError::Type("SQLite connection poisoned".into()))?.ping().unwrap_or(false),Value::Postgres(id)=>postgres_database(&self.runtime,Value::Postgres(id))?.lock().map_err(|_|VmError::Type("PostgreSQL connection poisoned".into()))?.ping().unwrap_or(false),Value::Mysql(id)=>mysql_database(&self.runtime,Value::Mysql(id))?.lock().map_err(|_|VmError::Type("MySQL connection poisoned".into()))?.ping().unwrap_or(false),_=>return Err(VmError::Type("std::db ping requires database connection".into()))};stack.push(Value::Bool(healthy)); }
                Op::Try => {
                    let value = pop(&mut stack, &function.name)?;
                    match value {
                        Value::Enum { name, variant, payload: Some(payload) } if (name == "Result" && variant == "Ok") || (name == "Option" && variant == "Some") => stack.push(*payload),
                        Value::Enum { name, variant, payload } if (name == "Result" && variant == "Err") || (name == "Option" && variant == "None") => return Ok(Value::Enum { name, variant, payload }),
                        _ => return Err(VmError::Type("operator ? requires Result or Option".into())),
                    }
                }
                Op::CallNative { name, argc } => {
                    let args = take_args(&mut stack, argc, &function.name)?;
                    stack.push(native::invoke(&name, args, self.capabilities)?);
                }
                Op::Ret => return Ok(stack.pop().unwrap_or(Value::Nil)),
                Op::Print(argc) => {
                    let args = take_args(&mut stack, argc, &function.name)?;
                    let line = args.iter().map(val_to_string).collect::<Vec<_>>().join(" ");
                    if let Some(output) = &self.output { let _ = output.send(line); } else { println!("{line}"); }
                    stack.push(Value::Nil);
                }
                Op::Len => {
                    let value = pop(&mut stack, &function.name)?;
                    let length = match value { Value::Array(v) | Value::Tuple(v) => v.len(), Value::Str(v) => v.chars().count(), Value::Bytes(v) => v.len(), Value::Map(v) => v.len(), _ => return Err(VmError::Type("len requires an array, tuple, string, bytes, or map".into())) };
                    stack.push(Value::Int(length as i64));
                }
                Op::ToString => { let value = pop(&mut stack, &function.name)?; self.track_allocation(64)?; stack.push(Value::Str(val_to_string(&value))); }
                Op::NewArray(count) => { let values = take_args(&mut stack, count, &function.name)?; self.track_allocation(values.len().saturating_mul(32).saturating_add(64))?; stack.push(Value::Array(values)); }
                Op::NewTuple(count) => { let values = take_args(&mut stack, count, &function.name)?; self.track_allocation(values.len().saturating_mul(32).saturating_add(64))?; stack.push(Value::Tuple(values)); }
                Op::Index => {
                    let index_value = pop(&mut stack, &function.name)?; let target = pop(&mut stack, &function.name)?;
                    let value = match (target, index_value) {
                        (Value::Map(values), Value::Str(key)) => values.get(&key).cloned().ok_or(VmError::UnknownField(key))?,
                        (target, Value::Int(index)) => {
                            let index = usize::try_from(index).map_err(|_| VmError::IndexOutOfBounds { index: usize::MAX, length: 0 })?;
                            match target {
                                Value::Array(v) | Value::Tuple(v) => v.get(index).cloned().ok_or(VmError::IndexOutOfBounds { index, length: v.len() })?,
                                Value::Str(v) => v.chars().nth(index).map(Value::Char).ok_or(VmError::IndexOutOfBounds { index, length: v.chars().count() })?,
                                Value::Bytes(v) => v.get(index).map(|value| Value::Int(i64::from(*value))).ok_or(VmError::IndexOutOfBounds { index, length: v.len() })?,
                                _ => return Err(VmError::Type("value is not indexable by integer".into())),
                            }
                        }
                        _ => return Err(VmError::Type("index must be int, or string for maps".into())),
                    }; stack.push(value);
                }
                Op::NewStruct { name, fields } => {
                    let values = take_args(&mut stack, fields.len(), &function.name)?;
                    self.track_allocation(fields.len().saturating_mul(32).saturating_add(64))?;
                    stack.push(Value::Struct { name, fields: fields.into_iter().zip(values).collect() });
                }
                Op::GetField(field) => {
                    let value = pop(&mut stack, &function.name)?;
                    match value {
                        Value::Struct { fields, .. } | Value::Map(fields) => stack.push(fields.get(&field).cloned().ok_or(VmError::UnknownField(field))?),
                        _ => return Err(VmError::Type("field access requires a struct or map".into())),
                    }
                }
                Op::NewEnum { name, variant, has_payload } => {
                    let payload = if has_payload { Some(Box::new(pop(&mut stack, &function.name)?)) } else { None };
                    stack.push(Value::Enum { name, variant, payload });
                }
                Op::EnumIs { name, variant } => {
                    let value = pop(&mut stack, &function.name)?;
                    stack.push(Value::Bool(matches!(value, Value::Enum { name: n, variant: v, .. } if n == name && v == variant)));
                }
                Op::EnumPayload => {
                    let value = pop(&mut stack, &function.name)?;
                    if let Value::Enum { payload: Some(value), .. } = value { stack.push(*value); }
                    else { return Err(VmError::Type("enum variant has no payload".into())); }
                }
                Op::Nop => {}
                Op::Halt => return Ok(stack.pop().unwrap_or(Value::Nil)),
            }
            ip += 1;
        }
        Ok(stack.pop().unwrap_or(Value::Nil))
    }
}

fn http_request_value(request: titan_stdlib::http::Request, peer: &str) -> Value { let mut map=BTreeMap::new();map.insert("method".into(),Value::Str(request.method));map.insert("target".into(),Value::Str(request.target));map.insert("path".into(),Value::Str(request.path));map.insert("query".into(),request.query.map(Value::Str).unwrap_or(Value::Nil));map.insert("version".into(),Value::Str(request.version));map.insert("headers".into(),Value::Map(request.headers.into_iter().map(|(key,value)|(key,Value::Str(value))).collect()));map.insert("body".into(),Value::Bytes(request.body));map.insert("keep_alive".into(),Value::Bool(request.keep_alive));map.insert("peer".into(),Value::Str(peer.into()));Value::Map(map) }
fn http_router(runtime:&RuntimeState,id:u64)->Result<Arc<Mutex<HttpRouterState>>,VmError>{runtime.routers.lock().map_err(|_|VmError::Type("router registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown HTTP router {id}")))}
fn request_method_path(request:&Value)->Result<(String,String),VmError>{let Value::Map(map)=request else{return Err(VmError::Type("dispatch request must be map".into()))};let method=match map.get("method"){Some(Value::Str(value))=>value.to_ascii_uppercase(),_=>return Err(VmError::Type("request.method must be string".into()))};let path=match map.get("path"){Some(Value::Str(value))=>value.clone(),_=>return Err(VmError::Type("request.path must be string".into()))};Ok((method,path))}
fn http_error_value(error:&VmError)->Value{Value::Map(BTreeMap::from([("message".into(),Value::Str(error.to_string())),("kind".into(),Value::Str(match error{VmError::Type(_)=>"type",VmError::Native{..}=>"native",VmError::DivisionByZero=>"division_by_zero",VmError::Overflow=>"overflow",VmError::TaskCancelled=>"cancelled",_=>"runtime"}.into()))]))}
fn http_not_found()->Value{Value::Map(BTreeMap::from([("status".into(),Value::Int(404)),("headers".into(),Value::Map(BTreeMap::from([("Content-Type".into(),Value::Str("text/plain; charset=utf-8".into()))]))),("body".into(),Value::Str("Not Found".into())),("keep_alive".into(),Value::Bool(false))]))}
struct HttpResponseParts { status: u16, headers: BTreeMap<String, String>, body: Vec<u8>, keep_alive: bool }
fn http_response_value(value: Value, request_keep_alive: bool) -> Result<HttpResponseParts, VmError> {
    let Value::Map(mut map) = value else { return Err(VmError::Type("HTTP handler must return a response map".into())) };
    let status = match map.remove("status").unwrap_or(Value::Int(200)) { Value::Int(status) => u16::try_from(status).map_err(|_| VmError::Type("HTTP response status out of range".into()))?, _ => return Err(VmError::Type("HTTP response status must be int".into())) };
    let headers = match map.remove("headers").unwrap_or(Value::Map(BTreeMap::new())) { Value::Map(headers) => headers.into_iter().map(|(key, value)| if let Value::Str(value) = value { Ok((key, value)) } else { Err(VmError::Type("HTTP response headers must be strings".into())) }).collect::<Result<_, _>>()?, _ => return Err(VmError::Type("HTTP response headers must be a map".into())) };
    let body = match map.remove("body").unwrap_or(Value::Bytes(Vec::new())) { Value::Bytes(body) => body, Value::Str(body) => body.into_bytes(), _ => return Err(VmError::Type("HTTP response body must be bytes or string".into())) };
    let keep_alive = match map.remove("keep_alive").unwrap_or(Value::Bool(request_keep_alive)) { Value::Bool(value) => value && request_keep_alive, _ => return Err(VmError::Type("HTTP response keep_alive must be bool".into())) };
    Ok(HttpResponseParts { status, headers, body, keep_alive })
}
fn require_network(capabilities: RuntimeCapabilities, function: &str) -> Result<(), VmError> { if capabilities.network { Ok(()) } else { Err(VmError::PermissionDenied { function: function.into(), capability: "Network".into() }) } }
fn network_error(function: &str, error: std::io::Error) -> VmError { VmError::Native { function: function.into(), message: error.to_string() } }
fn mysql_pool(runtime:&RuntimeState,value:Value)->Result<titan_mysql::Pool,VmError>{let Value::MysqlPool(id)=value else{return Err(VmError::Type("operation requires MySQL pool".into()))};runtime.mysql_pools.lock().map_err(|_|VmError::Type("MySQL pool registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown MySQL pool {id}")))}
fn mysql_error(error:titan_mysql::MyError)->VmError{VmError::Native{function:"std::mysql".into(),message:error.to_string()}}
fn mysql_database(runtime:&RuntimeState,value:Value)->Result<Arc<Mutex<MysqlHandle>>,VmError>{let Value::Mysql(id)=value else{return Err(VmError::Type("operation requires MySQL connection".into()))};runtime.mysql.lock().map_err(|_|VmError::Type("MySQL registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown MySQL connection {id}")))}
fn mysql_migrations(value:Value)->Result<Vec<titan_mysql::Migration>,VmError>{array_value(value)?.into_iter().map(|value|{let Value::Map(mut map)=value else{return Err(VmError::Type("migration must be map".into()))};let version=match map.remove("version"){Some(Value::Int(value))=>value,_=>return Err(VmError::Type("migration.version must be int".into()))};let name=match map.remove("name"){Some(Value::Str(value))=>value,_=>return Err(VmError::Type("migration.name must be string".into()))};let sql=match map.remove("sql"){Some(Value::Str(value))=>value,_=>return Err(VmError::Type("migration.sql must be string".into()))};Ok(titan_mysql::Migration{version,name,sql})}).collect()}
fn mysql_params(value:Value)->Result<Vec<titan_mysql::DbValue>,VmError>{array_value(value)?.into_iter().map(|value|match value{Value::Nil=>Ok(titan_mysql::DbValue::Null),Value::Int(value)=>Ok(titan_mysql::DbValue::Integer(value)),Value::Float(value)if value.is_finite()=>Ok(titan_mysql::DbValue::Real(value)),Value::Str(value)=>Ok(titan_mysql::DbValue::Text(value)),Value::Bytes(value)=>Ok(titan_mysql::DbValue::Bytes(value)),_=>Err(VmError::Type("MySQL parameters must be nil, int, finite float, string, or bytes".into()))}).collect()}
fn mysql_row(row:BTreeMap<String,titan_mysql::DbValue>)->Value{Value::Map(row.into_iter().map(|(name,value)|(name,match value{titan_mysql::DbValue::Null=>Value::Nil,titan_mysql::DbValue::Integer(value)=>Value::Int(value),titan_mysql::DbValue::Unsigned(value)=>i64::try_from(value).map(Value::Int).unwrap_or_else(|_|Value::Str(value.to_string())),titan_mysql::DbValue::Real(value)=>Value::Float(value),titan_mysql::DbValue::Text(value)|titan_mysql::DbValue::Date(value)|titan_mysql::DbValue::Time(value)=>Value::Str(value),titan_mysql::DbValue::Bytes(value)=>Value::Bytes(value)})).collect())}
fn postgres_pool(runtime:&RuntimeState,value:Value)->Result<titan_postgres::Pool,VmError>{let Value::PostgresPool(id)=value else{return Err(VmError::Type("operation requires PostgreSQL pool".into()))};runtime.postgres_pools.lock().map_err(|_|VmError::Type("PostgreSQL pool registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown PostgreSQL pool {id}")))}
fn postgres_error(error:titan_postgres::PgError)->VmError{VmError::Native{function:"std::postgres".into(),message:error.to_string()}}
fn postgres_database(runtime:&RuntimeState,value:Value)->Result<Arc<Mutex<PostgresHandle>>,VmError>{let Value::Postgres(id)=value else{return Err(VmError::Type("operation requires PostgreSQL connection".into()))};runtime.postgres.lock().map_err(|_|VmError::Type("PostgreSQL registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown PostgreSQL connection {id}")))}
fn postgres_migrations(value:Value)->Result<Vec<titan_postgres::Migration>,VmError>{array_value(value)?.into_iter().map(|value|{let Value::Map(mut map)=value else{return Err(VmError::Type("migration must be map".into()))};let version=match map.remove("version"){Some(Value::Int(value))=>value,_=>return Err(VmError::Type("migration.version must be int".into()))};let name=match map.remove("name"){Some(Value::Str(value))=>value,_=>return Err(VmError::Type("migration.name must be string".into()))};let sql=match map.remove("sql"){Some(Value::Str(value))=>value,_=>return Err(VmError::Type("migration.sql must be string".into()))};Ok(titan_postgres::Migration{version,name,sql})}).collect()}
fn postgres_params(value:Value)->Result<Vec<titan_postgres::DbValue>,VmError>{array_value(value)?.into_iter().map(|value|match value{Value::Nil=>Ok(titan_postgres::DbValue::Null),Value::Bool(value)=>Ok(titan_postgres::DbValue::Bool(value)),Value::Int(value)=>Ok(titan_postgres::DbValue::Integer(value)),Value::Float(value)if value.is_finite()=>Ok(titan_postgres::DbValue::Real(value)),Value::Str(value)=>Ok(titan_postgres::DbValue::Text(value)),Value::Bytes(value)=>Ok(titan_postgres::DbValue::Bytes(value)),value=>vm_json(value).map(titan_postgres::DbValue::Json)}).collect()}
fn vm_json(value:Value)->Result<serde_json::Value,VmError>{Ok(match value{Value::Nil=>serde_json::Value::Null,Value::Bool(value)=>value.into(),Value::Int(value)=>value.into(),Value::Float(value)=>serde_json::Number::from_f64(value).map(serde_json::Value::Number).ok_or_else(||VmError::Type("JSON float must be finite".into()))?,Value::Str(value)=>value.into(),Value::Array(values)|Value::Tuple(values)=>serde_json::Value::Array(values.into_iter().map(vm_json).collect::<Result<_,_>>()?),Value::Map(values)=>serde_json::Value::Object(values.into_iter().map(|(key,value)|Ok((key,vm_json(value)?))).collect::<Result<_,VmError>>()?),_=>return Err(VmError::Type("unsupported PostgreSQL parameter".into()))})}
fn postgres_row(row:BTreeMap<String,titan_postgres::DbValue>)->Value{Value::Map(row.into_iter().map(|(name,value)|(name,match value{titan_postgres::DbValue::Null=>Value::Nil,titan_postgres::DbValue::Bool(value)=>Value::Bool(value),titan_postgres::DbValue::Integer(value)=>Value::Int(value),titan_postgres::DbValue::Real(value)=>Value::Float(value),titan_postgres::DbValue::Text(value)=>Value::Str(value),titan_postgres::DbValue::Bytes(value)=>Value::Bytes(value),titan_postgres::DbValue::Json(value)=>json_vm(value)})).collect())}
fn json_vm(value:serde_json::Value)->Value{match value{serde_json::Value::Null=>Value::Nil,serde_json::Value::Bool(value)=>Value::Bool(value),serde_json::Value::Number(value)=>value.as_i64().map(Value::Int).or_else(||value.as_f64().map(Value::Float)).unwrap_or(Value::Nil),serde_json::Value::String(value)=>Value::Str(value),serde_json::Value::Array(values)=>Value::Array(values.into_iter().map(json_vm).collect()),serde_json::Value::Object(values)=>Value::Map(values.into_iter().map(|(key,value)|(key,json_vm(value))).collect())}}
fn sqlite_pool(runtime:&RuntimeState,value:Value)->Result<titan_sqlite::Pool,VmError>{let Value::SqlitePool(id)=value else{return Err(VmError::Type("operation requires SQLite pool".into()))};runtime.sqlite_pools.lock().map_err(|_|VmError::Type("SQLite pool registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown SQLite pool {id}")))}
fn sqlite_error(error:titan_sqlite::DbError)->VmError{VmError::Native{function:"std::sqlite".into(),message:error.to_string()}}
fn sqlite_database(runtime:&RuntimeState,value:Value)->Result<Arc<Mutex<DatabaseHandle>>,VmError>{let Value::Sqlite(id)=value else{return Err(VmError::Type("operation requires SQLite connection".into()))};runtime.sqlite.lock().map_err(|_|VmError::Type("SQLite registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown SQLite connection {id}")))}
fn sqlite_migrations(value:Value)->Result<Vec<titan_sqlite::Migration>,VmError>{array_value(value)?.into_iter().map(|value|{let Value::Map(mut map)=value else{return Err(VmError::Type("migration must be map".into()))};let version=match map.remove("version"){Some(Value::Int(value))=>value,_=>return Err(VmError::Type("migration.version must be int".into()))};let name=match map.remove("name"){Some(Value::Str(value))=>value,_=>return Err(VmError::Type("migration.name must be string".into()))};let sql=match map.remove("sql"){Some(Value::Str(value))=>value,_=>return Err(VmError::Type("migration.sql must be string".into()))};Ok(titan_sqlite::Migration{version,name,sql})}).collect()}
fn sqlite_params(value:Value)->Result<Vec<titan_sqlite::DbValue>,VmError>{let values=array_value(value)?;values.into_iter().map(|value|match value{Value::Nil=>Ok(titan_sqlite::DbValue::Null),Value::Int(value)=>Ok(titan_sqlite::DbValue::Integer(value)),Value::Float(value)if value.is_finite()=>Ok(titan_sqlite::DbValue::Real(value)),Value::Str(value)=>Ok(titan_sqlite::DbValue::Text(value)),Value::Bytes(value)=>Ok(titan_sqlite::DbValue::Blob(value)),_=>Err(VmError::Type("SQLite parameters must be nil, int, finite float, string, or bytes".into()))}).collect()}
fn sqlite_row(row:BTreeMap<String,titan_sqlite::DbValue>)->Value{Value::Map(row.into_iter().map(|(name,value)|(name,match value{titan_sqlite::DbValue::Null=>Value::Nil,titan_sqlite::DbValue::Integer(value)=>Value::Int(value),titan_sqlite::DbValue::Real(value)=>Value::Float(value),titan_sqlite::DbValue::Text(value)=>Value::Str(value),titan_sqlite::DbValue::Blob(value)=>Value::Bytes(value)})).collect())}
fn server_control(runtime:&RuntimeState,value:Value)->Result<Arc<ServerControl>,VmError>{let Value::ServerControl(id)=value else{return Err(VmError::Type("operation requires server control".into()))};runtime.server_controls.lock().map_err(|_|VmError::Type("server control registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown server control {id}")))}
fn server_try_acquire(control: &ServerControl) -> bool {
    if control.shutting_down.load(Ordering::Acquire) {
        control.rejected.fetch_add(1, Ordering::Relaxed);
        return false;
    }
    loop {
        let active = control.active.load(Ordering::Acquire);
        if active >= control.maximum {
            control.rejected.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        if control
            .active
            .compare_exchange_weak(active, active + 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            control.accepted.fetch_add(1, Ordering::Relaxed);
            return true;
        }
    }
}
fn server_stats(control:&ServerControl)->Value{let shutting_down=control.shutting_down.load(Ordering::Acquire);Value::Map(BTreeMap::from([("maximum".into(),Value::Int(control.maximum as i64)),("active".into(),Value::Int(control.active.load(Ordering::Acquire)as i64)),("accepted".into(),Value::Int(control.accepted.load(Ordering::Relaxed)as i64)),("rejected".into(),Value::Int(control.rejected.load(Ordering::Relaxed)as i64)),("completed".into(),Value::Int(control.completed.load(Ordering::Relaxed)as i64)),("ready".into(),Value::Bool(!shutting_down)),("healthy".into(),Value::Bool(true)),("shutting_down".into(),Value::Bool(shutting_down))]))}
fn server_health_response(control:&ServerControl)->Value{let ready=!control.shutting_down.load(Ordering::Acquire);let active=control.active.load(Ordering::Acquire);let body=format!("{{\"status\":\"{}\",\"ready\":{},\"healthy\":true,\"active\":{}}}",if ready{"ok"}else{"draining"},ready,active);Value::Map(BTreeMap::from([("status".into(),Value::Int(if ready{200}else{503})),("headers".into(),Value::Map(BTreeMap::from([("Content-Type".into(),Value::Str("application/json; charset=utf-8".into())),("Cache-Control".into(),Value::Str("no-store".into()))]))),("body".into(),Value::Bytes(body.into_bytes())),("keep_alive".into(),Value::Bool(false))]))}
fn record_http_metrics(response:&Value,elapsed:Duration){let status=if let Value::Map(map)=response{if let Some(Value::Int(status))=map.get("status"){*status}else{500}}else{500};let class=format!("http.responses.{}xx",status.clamp(0,999)/100);let _=titan_stdlib::metrics::counter_add("http.requests.total",1);let _=titan_stdlib::metrics::counter_add(&class,1);let _=titan_stdlib::metrics::histogram_record("http.request.duration_ms",elapsed.as_secs_f64()*1000.0);}
fn websocket_connect(runtime: &RuntimeState, url: &str, protocol: &str, maximum: usize) -> Result<u64, VmError> {
    let parsed = titan_stdlib::websocket::parse_url(url).map_err(|error| VmError::Native { function: "std::ws::connect".into(), message: error.to_string() })?;
    let address = if parsed.host.contains(':') { format!("[{}]:{}", parsed.host, parsed.port) } else { format!("{}:{}", parsed.host, parsed.port) };
    let transport = if parsed.secure {
        WebSocketTransport::Tls(Arc::new(Mutex::new(titan_tls::connect(&address, &parsed.host, titan_tls::client_config()).map_err(|error| VmError::Native { function: "std::ws::connect".into(), message: error.to_string() })?)))
    } else {
        let socket = TcpStream::connect(&address).map_err(|error| network_error("std::ws::connect", error))?;
        socket.set_read_timeout(Some(Duration::from_secs(10))).and_then(|_| socket.set_write_timeout(Some(Duration::from_secs(10)))).map_err(|error| network_error("std::ws::connect", error))?;
        WebSocketTransport::Tcp(Arc::new(Mutex::new(socket)))
    };
    let key = titan_stdlib::websocket::client_key().map_err(|error| VmError::Native { function: "std::ws::connect".into(), message: error.to_string() })?;
    let mut request = format!("GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {}\r\n", parsed.path, parsed.authority, key);
    if !protocol.is_empty() { request.push_str(&format!("Sec-WebSocket-Protocol: {protocol}\r\n")); }
    request.push_str("\r\n"); websocket_transport_write(&transport, request.as_bytes())?;
    let mut response = Vec::new();
    loop {
        if response.windows(4).any(|window| window == b"\r\n\r\n") { break; }
        if response.len() > 64 * 1024 { return Err(VmError::Type("WebSocket handshake headers too large".into())); }
        let mut chunk = [0u8; 4096]; let count = websocket_transport_read(&transport, &mut chunk)?;
        if count == 0 { return Err(VmError::WebSocketDisconnected); }
        response.extend_from_slice(&chunk[..count]);
    }
    let consumed = titan_stdlib::websocket::validate_accept_response(&response, &key).map_err(|error| VmError::Native { function: "std::ws::connect".into(), message: error.to_string() })?;
    titan_stdlib::websocket::validate_subprotocol_response(&response, protocol).map_err(|error| VmError::Native { function: "std::ws::connect".into(), message: error.to_string() })?;
    let id = insert_websocket(runtime, transport, false, maximum)?;
    if response.len() > consumed { let socket = pop_websocket(runtime, Value::WebSocket(id))?; socket.decoder.lock().map_err(|_| VmError::Type("WebSocket decoder poisoned".into()))?.push(&response[consumed..]).map_err(|error| VmError::Native { function: "std::ws::connect".into(), message: error.to_string() })?; }
    Ok(id)
}
fn positive_limit(value:Value,name:&str)->Result<usize,VmError>{let Value::Int(value)=value else{return Err(VmError::Type(format!("{name} must be int")))};let value=usize::try_from(value).map_err(|_|VmError::Type(format!("{name} must be positive")))?;if value==0||value>64*1024*1024{return Err(VmError::Type(format!("{name} must be between 1 and 64 MiB")))}Ok(value)}
fn insert_websocket(runtime:&RuntimeState,transport:WebSocketTransport,server_side:bool,maximum:usize)->Result<u64,VmError>{let id=runtime.next_socket.fetch_add(1,Ordering::Relaxed);runtime.websockets.lock().map_err(|_|VmError::Type("WebSocket registry poisoned".into()))?.insert(id,Arc::new(WebSocketConnection{transport,decoder:Mutex::new(titan_stdlib::websocket::MessageDecoder::new(maximum)),require_mask:server_side,mask_outgoing:!server_side,close_sent:AtomicBool::new(false)}));Ok(id)}
fn pop_websocket(runtime:&RuntimeState,value:Value)->Result<Arc<WebSocketConnection>,VmError>{let Value::WebSocket(id)=value else{return Err(VmError::Type("operation requires WebSocket".into()))};runtime.websockets.lock().map_err(|_|VmError::Type("WebSocket registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown WebSocket {id}")))}
fn websocket_transport_write(transport:&WebSocketTransport,data:&[u8])->Result<(),VmError>{match transport{WebSocketTransport::Tcp(stream)=>stream.lock().map_err(|_|VmError::Type("TCP stream poisoned".into()))?.write_all(data).map_err(|error|network_error("std::ws",error)),WebSocketTransport::Tls(stream)=>stream.lock().map_err(|_|VmError::Type("TLS stream poisoned".into()))?.write_all(data).map_err(|error|network_error("std::ws",error))}}
fn websocket_transport_read(transport:&WebSocketTransport,buffer:&mut[u8])->Result<usize,VmError>{match transport{WebSocketTransport::Tcp(stream)=>stream.lock().map_err(|_|VmError::Type("TCP stream poisoned".into()))?.read(buffer).map_err(|error|network_error("std::ws",error)),WebSocketTransport::Tls(stream)=>stream.lock().map_err(|_|VmError::Type("TLS stream poisoned".into()))?.read(buffer).map_err(|error|network_error("std::ws",error))}}
fn websocket_write(socket:&WebSocketConnection,data:&[u8])->Result<(),VmError>{websocket_transport_write(&socket.transport,data)}
fn websocket_read(socket:&WebSocketConnection,buffer:&mut[u8])->Result<usize,VmError>{websocket_transport_read(&socket.transport,buffer)}
fn websocket_send(socket:&WebSocketConnection,opcode:u8,payload:&[u8])->Result<(),VmError>{if socket.close_sent.load(Ordering::Acquire){return Err(VmError::Type("WebSocket is closing".into()))}let frame=titan_stdlib::websocket::encode_frame_with_policy(opcode,payload,socket.mask_outgoing).map_err(|error|VmError::Native{function:"std::ws".into(),message:error.to_string()})?;websocket_write(socket,&frame)}
fn websocket_close(
    socket: &WebSocketConnection,
    code: u16,
    reason: &str,
) -> Result<(), VmError> {
    if !matches!(code, 1000..=1003 | 1007..=1014 | 3000..=4999) || reason.len() > 123 {
        return Err(VmError::Type("invalid WebSocket close code or reason".into()));
    }

    if socket.close_sent.swap(true, Ordering::AcqRel) {
        return Ok(());
    }

    let mut payload = code.to_be_bytes().to_vec();
    payload.extend_from_slice(reason.as_bytes());
    let frame = titan_stdlib::websocket::encode_frame_with_policy(
        8,
        &payload,
        socket.mask_outgoing,
    )
    .map_err(|error| VmError::Native {
        function: "std::ws::close".into(),
        message: error.to_string(),
    })?;
    websocket_write(socket, &frame)
}
fn websocket_receive(socket:&WebSocketConnection)->Result<titan_stdlib::websocket::Message,VmError>{loop{if let Some(message)=socket.decoder.lock().map_err(|_|VmError::Type("WebSocket decoder poisoned".into()))?.next(Some(socket.require_mask)).map_err(|error|VmError::Native{function:"std::ws::receive".into(),message:error.to_string()})?{match message{titan_stdlib::websocket::Message::Ping(payload)=>{let frame=titan_stdlib::websocket::encode_frame_with_policy(10,&payload,socket.mask_outgoing).map_err(|error|VmError::Native{function:"std::ws::receive".into(),message:error.to_string()})?;websocket_write(socket,&frame)?;continue}titan_stdlib::websocket::Message::Close{code,reason}=>{if !socket.close_sent.load(Ordering::Acquire){websocket_close(socket,code.unwrap_or(1000),&reason)?}return Ok(titan_stdlib::websocket::Message::Close{code,reason})}message=>return Ok(message)}}let mut bytes=[0u8;8192];let count=websocket_read(socket,&mut bytes)?;if count==0{return Err(VmError::WebSocketDisconnected)}socket.decoder.lock().map_err(|_|VmError::Type("WebSocket decoder poisoned".into()))?.push(&bytes[..count]).map_err(|error|VmError::Native{function:"std::ws::receive".into(),message:error.to_string()})?;}}
fn websocket_decoder(runtime:&RuntimeState,id:u64)->Result<Arc<Mutex<titan_stdlib::websocket::MessageDecoder>>,VmError>{runtime.websocket_decoders.lock().map_err(|_|VmError::Type("WebSocket decoder registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown WebSocket decoder {id}")))}
fn websocket_message_value(message:titan_stdlib::websocket::Message)->Value{use titan_stdlib::websocket::Message;match message{Message::Text(text)=>Value::Map(BTreeMap::from([("type".into(),Value::Str("text".into())),("text".into(),Value::Str(text))])),Message::Binary(data)=>Value::Map(BTreeMap::from([("type".into(),Value::Str("binary".into())),("data".into(),Value::Bytes(data))])),Message::Ping(data)=>Value::Map(BTreeMap::from([("type".into(),Value::Str("ping".into())),("data".into(),Value::Bytes(data))])),Message::Pong(data)=>Value::Map(BTreeMap::from([("type".into(),Value::Str("pong".into())),("data".into(),Value::Bytes(data))])),Message::Close{code,reason}=>Value::Map(BTreeMap::from([("type".into(),Value::Str("close".into())),("code".into(),code.map(|code|Value::Int(code as i64)).unwrap_or(Value::Nil)),("reason".into(),Value::Str(reason))]))}}
fn take_socket_stream(runtime:&RuntimeState,id:u64)->Result<Arc<Mutex<TcpStream>>,VmError>{runtime.streams.lock().map_err(|_|VmError::Type("stream registry poisoned".into()))?.remove(&id).ok_or_else(||VmError::Type(format!("unknown TCP stream {id}")))}
fn take_tls_stream(runtime:&RuntimeState,id:u64)->Result<Arc<Mutex<titan_tls::TlsStream>>,VmError>{runtime.tls_streams.lock().map_err(|_|VmError::Type("TLS stream registry poisoned".into()))?.remove(&id).ok_or_else(||VmError::Type(format!("unknown TLS stream {id}")))}
fn tls_stream(runtime:&RuntimeState,id:u64)->Result<Arc<Mutex<titan_tls::TlsStream>>,VmError>{runtime.tls_streams.lock().map_err(|_|VmError::Type("TLS stream registry poisoned".into()))?.get(&id).cloned().ok_or_else(||VmError::Type(format!("unknown TLS stream {id}")))}
fn socket_stream(runtime: &RuntimeState, id: u64) -> Result<Arc<Mutex<TcpStream>>, VmError> { runtime.streams.lock().map_err(|_| VmError::Type("stream registry poisoned".into()))?.get(&id).cloned().ok_or_else(|| VmError::Type(format!("unknown TCP stream {id}"))) }
fn option_some(value: Value) -> Value { Value::Enum { name: "Option".into(), variant: "Some".into(), payload: Some(Box::new(value)) } }
fn option_none() -> Value { Value::Enum { name: "Option".into(), variant: "None".into(), payload: None } }
fn timeout_value(value: Value) -> Result<Duration, VmError> { if let Value::Int(milliseconds) = value { Ok(Duration::from_millis(u64::try_from(milliseconds).map_err(|_| VmError::InvalidTimeout)?)) } else { Err(VmError::InvalidTimeout) } }
fn array_value(value: Value) -> Result<Vec<Value>, VmError> { match value { Value::Array(values) | Value::Tuple(values) => Ok(values), _ => Err(VmError::Type("operation requires an array".into())) } }
fn pop(stack: &mut Vec<Value>, function: &str) -> Result<Value, VmError> { stack.pop().ok_or_else(|| VmError::StackUnderflow(function.into())) }
fn take_args(stack: &mut Vec<Value>, count: usize, function: &str) -> Result<Vec<Value>, VmError> {
    if stack.len() < count { return Err(VmError::StackUnderflow(function.into())); }
    Ok(stack.split_off(stack.len() - count))
}
fn truthy(value: &Value) -> bool { !matches!(value, Value::Bool(false) | Value::Nil) }
fn binary<F>(stack: &mut Vec<Value>, function: &str, operation: F) -> Result<(), VmError> where F: FnOnce(Value, Value) -> Result<Value, VmError> {
    let right = pop(stack, function)?; let left = pop(stack, function)?; stack.push(operation(left, right)?); Ok(())
}
fn compare<F>(stack: &mut Vec<Value>, function: &str, operation: F) -> Result<(), VmError> where F: FnOnce(&Value, &Value) -> bool {
    let right = pop(stack, function)?; let left = pop(stack, function)?; stack.push(Value::Bool(operation(&left, &right))); Ok(())
}
fn ordered<F>(stack: &mut Vec<Value>, function: &str, operation: F) -> Result<(), VmError> where F: FnOnce(f64, f64) -> bool {
    let right = pop(stack, function)?; let left = pop(stack, function)?;
    let (a, b) = match (left, right) { (Value::Int(a), Value::Int(b)) => (a as f64, b as f64), (Value::Float(a), Value::Float(b)) => (a, b), _ => return Err(VmError::Type("ordered comparison requires matching numbers".into())) };
    stack.push(Value::Bool(operation(a, b))); Ok(())
}
fn integer_binary<F>(stack: &mut Vec<Value>, function: &str, operation: F) -> Result<(), VmError> where F: FnOnce(i64, i64) -> i64 {
    binary(stack, function, |a, b| match (a, b) { (Value::Int(a), Value::Int(b)) => Ok(Value::Int(operation(a, b))), _ => Err(VmError::Type("bitwise operation requires integers".into())) })
}
fn add(a: Value, b: Value) -> Result<Value, VmError> {
    match (a, b) {
        (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a.checked_add(b).ok_or(VmError::Overflow)?)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        // v0.16.0 QoL: String + Any coerces the right operand via
        // val_to_string(). Mirrors JS `"x " + n` and Python f-strings.
        // Applies symmetrically so `n + " suffix"` also works.
        (Value::Str(a),   Value::Str(b))   => Ok(Value::Str(a + &b)),
        (Value::Str(a),   other)           => Ok(Value::Str(a + &val_to_string(&other))),
        (other,           Value::Str(b))   => Ok(Value::Str(val_to_string(&other) + &b)),
        _ => Err(VmError::Type("addition requires matching numbers or strings".into())),
    }
}
fn sub(a: Value, b: Value) -> Result<Value, VmError> { match (a, b) { (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.checked_sub(b).ok_or(VmError::Overflow)?)), (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)), _ => Err(VmError::Type("subtraction requires matching numbers".into())) } }
fn mul(a: Value, b: Value) -> Result<Value, VmError> { match (a, b) { (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.checked_mul(b).ok_or(VmError::Overflow)?)), (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)), _ => Err(VmError::Type("multiplication requires matching numbers".into())) } }
fn div(a: Value, b: Value) -> Result<Value, VmError> { match (a, b) { (_, Value::Int(0)) => Err(VmError::DivisionByZero), (_, Value::Float(0.0)) => Err(VmError::DivisionByZero), (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.checked_div(b).ok_or(VmError::Overflow)?)), (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)), _ => Err(VmError::Type("division requires matching numbers".into())) } }
fn modulo(a: Value, b: Value) -> Result<Value, VmError> { match (a, b) { (_, Value::Int(0)) => Err(VmError::DivisionByZero), (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.checked_rem(b).ok_or(VmError::Overflow)?)), _ => Err(VmError::Type("modulo requires integers".into())) } }
fn make_range(args: Vec<Value>) -> Result<Value, VmError> {
    let [Value::Int(start), Value::Int(end), Value::Bool(inclusive)] = args.as_slice() else { return Err(VmError::Type("range requires two integers".into())); };
    let stop = if *inclusive { end.checked_add(1).ok_or(VmError::Overflow)? } else { *end };
    let length = stop.saturating_sub(*start);
    if length > 1_000_000 { return Err(VmError::Type("range exceeds the one-million element safety limit".into())); }
    let values = if start <= &stop { (*start..stop).map(Value::Int).collect() } else { Vec::new() };
    Ok(Value::Array(values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use titan_codegen::AstCompiler;
    use titan_lexer::Lexer;
    use titan_parser::Parser;

    fn compile(source: &str) -> Result<CompiledModule, String> { let mut lexer = Lexer::new(source); let tokens = lexer.tokenize().0.to_vec(); let program = Parser::new(tokens).parse_program().map_err(|e| e.to_string())?; AstCompiler::new().compile_program(&program).map_err(|e| e.to_string()) }
    fn run(source: &str) -> Result<Value, String> { Vm::new(compile(source)?).run().map_err(|e| e.to_string()).map(|v| v.unwrap()) }
    fn run_sandboxed(source: &str) -> Result<Value, String> { Vm::sandboxed(compile(source)?).run().map_err(|e| e.to_string()).map(|v| v.unwrap()) }
    #[test] fn arithmetic_returns_value() { assert_eq!(run("fn main() { 40 + 2 }").unwrap(), Value::Int(42)); }
    #[test] fn recursion_works() { assert_eq!(run("fn fact(n: int) -> int { if n <= 1 { return 1 } n * fact(n-1) } fn main() { fact(5) }").unwrap(), Value::Int(120)); }
    #[test] fn loops_and_ranges_work() { assert_eq!(run("fn main() { let x = 0 for i in 0..5 { x += i } x }").unwrap(), Value::Int(10)); }
    #[test] fn structs_work() { assert_eq!(run("struct Point { x: int, y: int } fn main() { let p = Point { x: 2, y: 3 } p.x + p.y }").unwrap(), Value::Int(5)); }
    #[test] fn enum_matching_works() { assert_eq!(run("enum Maybe { None, Some(int) } fn main() { let x = Maybe::Some(7) match x { Maybe::Some(n) => n, Maybe::None => 0 } }").unwrap(), Value::Int(7)); }
    #[test] fn native_text_and_encoding_work() { assert_eq!(run("fn main() { std::text::reverse(\"Titan\") }").unwrap(), Value::Str("natiT".into())); assert_eq!(run("fn main() { std::encoding::utf8_decode(std::encoding::base64_decode(\"VGl0YW4=\")) }").unwrap(), Value::Str("Titan".into())); }
    #[test] fn empty_map_constructor_and_length_work() { assert_eq!(run("fn main() { let values = std::map::new() std::map::length(values) }").unwrap(), Value::Int(0)); }
    #[test] fn persistent_map_insert_new_adds_entry() { assert_eq!(run("fn main() { let values = std::map::insert_new(std::map::new(), \"answer\", 42) std::map::length(values) }").unwrap(), Value::Int(1)); }
    #[test] fn map_lookup_uses_string_content() { assert_eq!(run("fn main() { let values = std::map::insert_new(std::map::new(), \"answer\", 42) if std::map::contains(values, \"ans\" + \"wer\") { std::map::get(values, \"answer\") } else { 0 } }").unwrap(), Value::Int(42)); }
    #[test] fn persistent_map_insert_replaces_by_content() { assert_eq!(run("fn main() { let first = std::map::insert(std::map::new(), \"answer\", 1) let updated = std::map::insert(first, \"ans\" + \"wer\", 42) std::map::length(updated) * 100 + std::map::get(updated, \"answer\") }").unwrap(), Value::Int(142)); }
    #[test] fn persistent_map_remove_preserves_original() { assert_eq!(run("fn main() { let original = std::map::insert(std::map::new(), \"answer\", 42) let removed = std::map::remove(original, \"ans\" + \"wer\") std::map::length(original) * 100 + std::map::length(removed) }").unwrap(), Value::Int(100)); }
    #[test] fn native_json_maps_support_fields() { assert_eq!(run(r#"fn main() { std::json::parse("{\"answer\":42}").answer }"#).unwrap(), Value::Int(42)); }
    #[test] fn interpolation_concatenates_every_segment() { assert_eq!(run("fn double(x: int) -> int { x * 2 } fn main() { let i = 3 \"value({i}) = {double(i)}!\" }").unwrap(), Value::Str("value(3) = 6!".into())); }
    #[test] fn native_generic_arrays_accept_concrete_elements() { assert_eq!(run("fn main() { std::stats::mean([10, 20, 30, 40]) }").unwrap(), Value::Float(25.0)); }
    #[test] fn closures_capture_lexical_values() { assert_eq!(run("fn main() { let base = 10 let add = |value: int| -> int value + base add(5) }").unwrap(), Value::Int(15)); }
    #[test] fn named_functions_are_first_class() { assert_eq!(run("fn double(x: int) -> int { x * 2 } fn main() { let operation = double operation(21) }").unwrap(), Value::Int(42)); }
    #[test] fn functional_array_pipeline_works() { assert_eq!(run("fn main() { [1, 2, 3, 4].map(|x: int| x * 2).filter(|x: int| x > 4).fold(0, |sum: int, x: int| sum + x) }").unwrap(), Value::Int(14)); }
    #[test] fn persistent_array_set_returns_updated_copy() { assert_eq!(run("fn main() { let original = [10, 20] let updated = std::array::set(original, 1, 99) updated[1] }").unwrap(), Value::Int(99)); }
    #[test] fn persistent_array_push_preserves_original() { assert_eq!(run("fn main() { let original = [10, 20] let updated = std::array::push(original, 30) len(original) * 100 + updated[2] }").unwrap(), Value::Int(230)); }
    #[test] fn persistent_array_pop_handles_values_and_empty_arrays() { assert_eq!(run("fn main() { let original = [10, 20, 30] let shorter = std::array::pop(original) let empty = std::array::pop([]) len(original) * 100 + len(shorter) * 10 + len(empty) }").unwrap(), Value::Int(320)); }
    #[test] fn persistent_array_slice_validates_and_copies_range() { assert_eq!(run("fn main() { let original = [10, 20, 30, 40] let part = std::array::slice(original, 1, 3) len(original) * 100 + part[0] + part[1] }").unwrap(), Value::Int(450)); }
    #[test] fn persistent_array_concat_preserves_both_inputs() { assert_eq!(run("fn main() { let left = [1, 2] let right = [3, 4] let joined = std::array::concat(left, right) len(left) * 1000 + len(right) * 100 + len(joined) * 10 + joined[3] }").unwrap(), Value::Int(2244)); }
    #[test] fn runtime_memory_quota_and_stats_work_from_titan() {
        assert_eq!(run("fn main() { let t = std::runtime::spawn_quota(500, || { let s = \"long allocation string creation for memory tracking in child task 1234567890\" + \" more bytes\" return 42 }) let r = join(t) let mem = std::runtime::memory_limit() let alloc = std::runtime::allocated_bytes() let live = std::runtime::gc_live_count() let coll = std::runtime::gc_collect() [r, mem, alloc >= 0, live >= 0, coll >= 0] }").unwrap(), Value::Array(vec![Value::Int(42), Value::Int(-1), Value::Bool(true), Value::Bool(true), Value::Bool(true)]));
    }
    #[test] fn runtime_heap_dump_and_gc_threshold_work_from_titan() {
        let path = std::env::temp_dir().join(format!("titan-vm-dump-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let escaped_path = path.display().to_string().replace('\\', "\\\\");
        let source = format!("fn main() {{ std::runtime::gc_set_threshold(2048 * 1024) let th = std::runtime::gc_threshold() let tasks = std::runtime::active_tasks() let ok = std::runtime::heap_dump(\"{}\") [th, tasks, ok] }}", escaped_path);
        assert_eq!(run(&source).unwrap(), Value::Array(vec![Value::Int(2048 * 1024), Value::Int(0), Value::Bool(true)]));
        let _ = std::fs::remove_file(path);
    }
    #[test] fn try_unwraps_success_and_propagates_failure() {
        assert_eq!(run("fn answer() -> Result { let value = Result::Ok(41)? Result::Ok(value + 1) } fn main() { match answer() { Result::Ok(value) => value, Result::Err(error) => 0 } }").unwrap(), Value::Int(42));
        assert_eq!(run("fn answer() -> Result { let value = Result::Err(\"no\")? Result::Ok(value) } fn main() { match answer() { Result::Ok(value) => 0, Result::Err(error) => 7 } }").unwrap(), Value::Int(7));
    }
    #[test] fn sandbox_blocks_effectful_natives() {
        let source = "fn main() { std::fs::read_text(\"secret\") }"; let mut lexer = Lexer::new(source); let tokens = lexer.tokenize().0.to_vec();
        let program = Parser::new(tokens).parse_program().unwrap(); let module = AstCompiler::new().compile_program(&program).unwrap();
        assert!(matches!(Vm::sandboxed(module).run(), Err(VmError::PermissionDenied { .. })));
    }
    #[test] fn debugger_breaks_steps_and_reports_state() {
        let mut lexer = Lexer::new("fn main() { let value = 40 value + 2 }"); let tokens = lexer.tokenize().0.to_vec();
        let program = Parser::new(tokens).parse_program().unwrap(); let module = AstCompiler::new().compile_program(&program).unwrap();
        let (controller, mut debugger) = Debugger::channel([Breakpoint::Instruction { function: module.entry, instruction: 0 }]);
        let thread = std::thread::spawn(move || { let mut vm = Vm::new(module); vm.run_debug(&mut debugger) });
        let DebugEvent::Stopped(first) = controller.recv().unwrap() else { panic!("expected breakpoint") }; assert_eq!(first.instruction, 0);
        controller.command(DebugCommand::StepIn).unwrap(); let DebugEvent::Stopped(second) = controller.recv().unwrap() else { panic!("expected step") }; assert!(second.instruction > first.instruction);
        controller.command(DebugCommand::Continue).unwrap(); assert!(matches!(controller.recv().unwrap(), DebugEvent::Terminated { error: None }));
        assert_eq!(thread.join().unwrap().unwrap(), Some(Value::Int(42)));
    }
    #[test] fn tasks_execute_on_threads_and_join() { assert_eq!(run("fn main() { let task = spawn || 40 + 2 join(task) }").unwrap(), Value::Int(42)); }
    #[test] fn channels_communicate_between_tasks() { assert_eq!(run("fn main() { let pair = channel(1) let tx = pair[0] let rx = pair[1] let task = spawn || { send(tx, 42) } let value = recv(rx) join(task) value }").unwrap(), Value::Int(42)); }
    #[test] fn task_and_channel_timeouts_are_explicit_options() { assert_eq!(run("fn main() { let pair = channel(1) let result = recv_timeout(pair[1], 1) match result { Option::None => 1, Option::Some(value) => 0 } }").unwrap(), Value::Int(1)); }
    #[test] fn select_returns_channel_index_and_value() { assert_eq!(run("fn main() { let a = channel(1) let b = channel(1) let task = spawn || { send(b[0], 77) } let selected = select([a[1], b[1]], 1000)? join(task) selected[1] }").unwrap(), Value::Int(77)); }
    #[test] fn cancellation_stops_cooperative_tasks() { assert!(run("fn main() { let task = spawn || loop {} let pending = join_timeout(task, 1) cancel(task) join(task) }").is_err()); }
    #[test] fn tcp_round_trip_works_across_tasks() { assert_eq!(run("fn main() { let listener = std::net::tcp_listen(\"127.0.0.1:0\") let address = std::net::tcp_local_addr(listener) let server = spawn || { let accepted = std::net::tcp_accept(listener) let stream = accepted[0] let request = std::net::tcp_read(stream, 16) std::net::tcp_write(stream, request) std::net::tcp_close(stream) } let client = std::net::tcp_connect(address) std::net::tcp_write(client, \"ping\") let response = std::net::tcp_read(client, 16) std::net::tcp_close(client) join(server) std::net::tcp_close(listener) std::encoding::utf8_decode(response) }").unwrap(), Value::Str("ping".into())); }
    #[test] fn http_codec_is_callable_from_titan() { assert_eq!(run("fn main() { let bytes = std::encoding::utf8_encode(\"GET /hello?name=titan HTTP/1.1\\r\\nHost: localhost\\r\\n\\r\\n\") let request = std::http::parse_request(bytes)? request.path }").unwrap(), Value::Str("/hello".into())); assert!(matches!(run("fn main() { let headers = std::json::parse(\"{\\\"Content-Type\\\":\\\"text/plain\\\"}\") std::http::build_response(200, headers, std::encoding::utf8_encode(\"ok\"), false) }").unwrap(), Value::Bytes(_))); }
    #[test] fn advanced_http_client_is_callable_from_titan(){assert_eq!(run("fn main(){let listener=std::net::tcp_listen(\"127.0.0.1:0\") let address=std::net::tcp_local_addr(listener) let server=spawn || {let accepted=std::net::tcp_accept(listener) let stream=accepted[0] std::net::tcp_read(stream,4096) std::net::tcp_write(stream,\"HTTP/1.1 200 OK\\r\\nContent-Length: 5\\r\\nConnection: close\\r\\n\\r\\nhello\") std::net::tcp_close(stream)} let response=std::http::request(\"GET\",\"http://\"+address+\"/\",std::json::parse(\"{}\"),std::encoding::utf8_encode(\"\"),1024,2,2000) join(server) std::net::tcp_close(listener) std::encoding::utf8_decode(response.body)}").unwrap(),Value::Str("hello".into()));}
    #[test] fn multipart_uploads_are_callable_from_titan(){assert_eq!(run("fn main(){let body=std::encoding::utf8_encode(\"--x\\r\\nContent-Disposition: form-data; name=\\\"title\\\"\\r\\n\\r\\nTitan\\r\\n--x--\\r\\n\") let parts=std::http::parse_multipart(\"multipart/form-data; boundary=x\",body,4,1024) std::encoding::utf8_decode(parts[0].data)}").unwrap(),Value::Str("Titan".into()));}
    #[test] fn metrics_are_callable_from_titan(){assert_eq!(run("fn main(){std::metrics::reset() std::metrics::counter_add(\"requests.total\",2) std::metrics::histogram_record(\"request.ms\",12.5) let metrics=std::metrics::snapshot() std::map::get(metrics.counters,\"requests.total\")}").unwrap(),Value::Int(2));}
    #[test] fn game_engine_surface_is_callable_from_titan(){assert_eq!(run("fn main(){std::game::init(\"Titan QA\",64,64) std::game::shutdown()}").unwrap(),Value::Bool(true));}
    #[test] fn gui_render_produces_real_rgba_bytes_from_titan(){assert_eq!(run("fn main(){std::gui::init() let root=std::gui::create_container(\"QA\",20,20) std::bytes::length(std::gui::render(root))}").unwrap(),Value::Int(1600));}
    #[test] fn gui_raster_feeds_image_pipeline_from_titan(){assert_eq!(run("fn main(){std::gui::init() let root=std::gui::create_container(\"QA\",8,8) let img=std::image::from_rgba(8,8,std::gui::render(root)) std::image::width(img)}").unwrap(),Value::Int(8));}
    #[test] fn input_setters_feed_readers_from_titan(){assert_eq!(run("fn main(){std::input::set_key_state(\"W\",true) std::input::is_key_pressed(\"W\")}").unwrap(),Value::Bool(true));}
    #[cfg(all(feature = "window_live", not(target_os = "android")))]
    #[test] fn live_window_services_reject_unknown_handles_from_titan(){assert_eq!(run("fn main(){std::window::live_pump(424242,1)}").unwrap(),Value::Int(-2));assert_eq!(run("fn main(){std::window::live_set_title(424242,\"ghost\")}").unwrap(),Value::Bool(false));}
    #[cfg(all(feature = "window_live", not(target_os = "android")))]
    #[test] fn live_window_events_are_empty_for_unknown_handles_from_titan(){assert_eq!(run("fn main(){len(std::window::live_poll_events(424242))}").unwrap(),Value::Int(0));}
    #[test] fn input_touch_round_trip_from_titan(){assert_eq!(run("fn main(){std::input::set_touch_point(0,33,44,true) std::input::touch_pos(0)}").unwrap(),Value::Array(vec![Value::Int(33),Value::Int(44),Value::Bool(true)]));}
    #[test] fn sqlite_migrations_are_idempotent_from_titan(){assert_eq!(run("fn main(){let db=std::sqlite::memory() let migrations=std::json::parse(\"[{\\\"version\\\":1,\\\"name\\\":\\\"create_items\\\",\\\"sql\\\":\\\"CREATE TABLE items(id INTEGER PRIMARY KEY);\\\"}]\") let first=std::sqlite::migrate(db,migrations) let second=std::sqlite::migrate(db,migrations) std::sqlite::close(db); [first,second]}").unwrap(),Value::Array(vec![Value::Int(1),Value::Int(0)]));}
    #[test] fn sqlite_pool_leases_are_reused_from_titan(){let path=std::env::temp_dir().join(format!("titan-vm-pool-{}.db",std::process::id()));let _=std::fs::remove_file(&path);let escaped_path=path.display().to_string().replace('\\',"\\\\");let source=format!("fn main(){{let pool=std::sqlite::pool(\"{}\",1) let first=std::sqlite::acquire(pool,1000)? let pending=std::sqlite::acquire(pool,1) std::sqlite::close(first) let second=std::sqlite::acquire(pool,1000)? let stats=std::sqlite::pool_stats(pool) std::sqlite::close(second) std::sqlite::pool_close(pool); [pending,stats.total]}}",escaped_path);assert_eq!(run(&source).unwrap(),Value::Array(vec![Value::Enum{name:"Option".into(),variant:"None".into(),payload:None},Value::Int(1)]));let _=std::fs::remove_file(path);}
    #[test] fn sqlite_ping_and_pool_health_work_from_titan(){let path=std::env::temp_dir().join(format!("titan-vm-health-{}.db",std::process::id()));let _=std::fs::remove_file(&path);let escaped_path=path.display().to_string().replace('\\',"\\\\");let source=format!("fn main(){{let pool=std::sqlite::pool(\"{}\",1) let health=std::sqlite::pool_health(pool,1000) let conn=std::sqlite::acquire(pool,1000)? let ping=std::sqlite::ping(conn) std::sqlite::close(conn) std::sqlite::pool_close(pool); [health,ping]}}",escaped_path);assert_eq!(run(&source).unwrap(),Value::Array(vec![Value::Bool(true),Value::Bool(true)]));let _=std::fs::remove_file(path);}
    #[test] fn common_database_api_works_with_sqlite(){assert_eq!(run("fn main(){let db=std::sqlite::memory() std::db::execute(db,\"CREATE TABLE values_(value INTEGER)\",[]) std::db::begin(db) std::db::execute(db,\"INSERT INTO values_ VALUES (?)\",[42]) std::db::commit(db) let rows=std::db::query(db,\"SELECT value FROM values_\",[]) std::db::close(db) rows[0].value}").unwrap(),Value::Int(42));}
    #[test] fn sqlite_prepared_queries_and_transactions_work(){assert_eq!(run("fn main(){let db=std::sqlite::memory() std::sqlite::execute(db,\"CREATE TABLE users(id INTEGER PRIMARY KEY,name TEXT)\",[]) std::sqlite::begin(db) std::sqlite::execute(db,\"INSERT INTO users(name) VALUES (?)\",[\"Ada\"]) std::sqlite::commit(db) let rows=std::sqlite::query(db,\"SELECT id,name FROM users WHERE name=?\",[\"Ada\"]) std::sqlite::close(db) rows[0].name}").unwrap(),Value::Str("Ada".into()));}
    #[test] fn server_control_enforces_backpressure_and_shutdown(){assert_eq!(run("fn main(){let control=std::server::control(2) let a=std::server::try_acquire(control) let b=std::server::try_acquire(control) let rejected=std::server::try_acquire(control) std::server::release(control) std::server::shutdown(control) let after=std::server::try_acquire(control) let stats=std::server::stats(control); [a,b,rejected,after,stats.active,stats.ready]}").unwrap(),Value::Array(vec![Value::Bool(true),Value::Bool(true),Value::Bool(false),Value::Bool(false),Value::Int(1),Value::Bool(false)]));}
    #[test] fn health_responses_and_dispatch_metrics_reflect_lifecycle(){assert_eq!(run("fn main(){let control=std::server::control(10) let healthy=std::server::health_response(control) let router=std::http::router() let request=std::json::parse(\"{\\\"method\\\":\\\"GET\\\",\\\"path\\\":\\\"/missing\\\"}\") std::http::dispatch(router,request) std::server::shutdown(control) let draining=std::server::health_response(control); [healthy.status,draining.status]}").unwrap(),Value::Array(vec![Value::Int(200),Value::Int(503)]));assert!(titan_stdlib::metrics::snapshot().unwrap().counters.get("http.requests.total").is_some_and(|count|*count>=1));}
    #[test] fn http_routing_is_callable_from_titan() { assert_eq!(run("fn main() { let params = std::http::route_match(\"/users/:id\", \"/users/42\")? params.id }").unwrap(), Value::Str("42".into())); assert_eq!(run("fn main() { let query = std::http::parse_query(\"tag=rust&tag=titan\", 10) query.tag[1] }").unwrap(), Value::Str("titan".into())); }
    #[test] fn composed_router_dispatches_middleware_and_params() { assert_eq!(run("fn main(){let router=std::http::router() std::http::middleware(router,|request|request) std::http::route(router,\"GET\",\"/users/:id\",|request|request.params.id) let request=std::json::parse(\"{\\\"method\\\":\\\"GET\\\",\\\"path\\\":\\\"/users/42\\\"}\") std::http::dispatch(router,request)}").unwrap(),Value::Str("42".into())); assert!(matches!(run("fn main(){let router=std::http::router() let request=std::json::parse(\"{\\\"method\\\":\\\"GET\\\",\\\"path\\\":\\\"/missing\\\"}\") std::http::dispatch(router,request)}").unwrap(),Value::Map(_))); }
    #[test] fn websocket_codec_is_callable_from_titan() { assert_eq!(run("fn main(){let frame=std::ws::encode(1,std::encoding::utf8_encode(\"hello\"),true) let parsed=std::ws::parse(frame,true,1024)? std::encoding::utf8_decode(parsed.payload)}").unwrap(),Value::Str("hello".into())); assert_eq!(run("fn main(){std::ws::accept_key(\"dGhlIHNhbXBsZSBub25jZQ==\")}").unwrap(),Value::Str("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=".into())); }
    #[test] fn websocket_http_upgrade_is_validated_from_titan(){assert_eq!(run("fn main(){let key=\"dGhlIHNhbXBsZSBub25jZQ==\" let raw=std::encoding::utf8_encode(\"GET /chat HTTP/1.1\\r\\nHost: localhost\\r\\nUpgrade: websocket\\r\\nConnection: keep-alive, Upgrade\\r\\nSec-WebSocket-Version: 13\\r\\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\\r\\nSec-WebSocket-Protocol: chat, superchat\\r\\n\\r\\n\") let request=std::http::parse_request(raw)? let response=std::ws::validate_upgrade(request,\"chat\") std::ws::validate_accept(response,key)}").unwrap(),Value::Bool(true));}
    #[test] fn stateful_websocket_decoder_is_callable_from_titan(){assert_eq!(run("fn main(){let decoder=std::ws::decoder(1024) let frame=std::ws::encode(1,std::encoding::utf8_encode(\"hello\"),true) std::ws::decoder_push(decoder,frame) let message=std::ws::decoder_next(decoder,true)? message.text}").unwrap(),Value::Str("hello".into()));}
    #[test] fn high_level_websocket_connection_round_trips_text(){assert_eq!(run("fn main(){let listener=std::net::tcp_listen(\"127.0.0.1:0\") let address=std::net::tcp_local_addr(listener) let server=spawn || {let accepted=std::net::tcp_accept(listener) let ws=std::ws::attach_tcp(accepted[0],true,1024) let message=std::ws::receive(ws) std::ws::send_text(ws,message.text) std::ws::close(ws,1000,\"done\")} let client_stream=std::net::tcp_connect(address) let client=std::ws::attach_tcp(client_stream,false,1024) std::ws::send_text(client,\"hello\") let response=std::ws::receive(client) std::ws::close(client,1000,\"done\") join(server) std::net::tcp_close(listener) response.text}").unwrap(),Value::Str("hello".into()));}
    #[test] fn websocket_client_performs_automatic_http_upgrade(){assert_eq!(run("fn main(){let listener=std::net::tcp_listen(\"127.0.0.1:0\") let address=std::net::tcp_local_addr(listener) let server=spawn || {let accepted=std::net::tcp_accept(listener) let stream=accepted[0] let raw=std::net::tcp_read(stream,65536) let request=std::http::parse_request(raw)? let upgrade=std::ws::validate_upgrade(request,\"\") std::net::tcp_write(stream,upgrade) let ws=std::ws::attach_tcp(stream,true,1024) let message=std::ws::receive(ws) std::ws::send_text(ws,message.text) std::ws::close(ws,1000,\"done\")} let client=std::ws::connect(\"ws://\"+address+\"/echo\",\"\",1024) std::ws::send_text(client,\"automatic\") let response=std::ws::receive(client) std::ws::close(client,1000,\"done\") join(server) std::net::tcp_close(listener) response.text}").unwrap(),Value::Str("automatic".into()));}
    #[test] fn router_recovers_handler_errors_as_http_responses() { assert_eq!(run("fn main(){let router=std::http::router() std::http::route(router,\"GET\",\"/fail\",|request|1/0) std::http::on_error(router,|request,error|std::http::error_response(500,error.message)) let request=std::json::parse(\"{\\\"method\\\":\\\"GET\\\",\\\"path\\\":\\\"/fail\\\"}\") let response=std::http::dispatch(router,request) response.status}").unwrap(),Value::Int(500)); assert!(matches!(run("fn main(){std::http::json_response(201,std::json::parse(\"{\\\"ok\\\":true}\"))}").unwrap(),Value::Map(_))); }
    #[test] fn response_middleware_and_rate_limits_work() { assert_eq!(run("fn main(){let router=std::http::router() std::http::route(router,\"GET\",\"/\",|request|std::json::parse(\"{\\\"status\\\":200,\\\"body\\\":\\\"ok\\\"}\")) std::http::after(router,|response|std::http::security_headers(response)) let request=std::json::parse(\"{\\\"method\\\":\\\"GET\\\",\\\"path\\\":\\\"/\\\"}\") let response=std::http::dispatch(router,request) std::map::get(response.headers,\"X-Frame-Options\")}").unwrap(),Value::Str("DENY".into())); assert_eq!(run("fn main(){let a=std::http::rate_limit(\"vm-test-key\",2,60000) let b=std::http::rate_limit(\"vm-test-key\",2,60000) let c=std::http::rate_limit(\"vm-test-key\",2,60000); [a,b,c]}").unwrap(),Value::Array(vec![Value::Bool(true),Value::Bool(true),Value::Bool(false)])); }
    #[test] fn high_level_http_server_invokes_titan_handler() { assert_eq!(run("fn main() { let listener=std::net::tcp_listen(\"127.0.0.1:0\") let address=std::net::tcp_local_addr(listener) let handler=|request| std::json::parse(\"{\\\"status\\\":200,\\\"headers\\\":{\\\"Content-Type\\\":\\\"text/plain\\\"},\\\"body\\\":\\\"hello titan\\\",\\\"keep_alive\\\":false}\") let server=spawn || std::http::serve_connection(listener,handler,10) let client=std::net::tcp_connect(address) std::net::tcp_write(client,\"GET / HTTP/1.1\\r\\nHost: localhost\\r\\nConnection: close\\r\\n\\r\\n\") let response=std::encoding::utf8_decode(std::net::tcp_read(client,4096)) std::net::tcp_close(client) join(server) std::net::tcp_close(listener) std::text::contains(response,\"hello titan\") }").unwrap(), Value::Bool(true)); }
    #[test] fn sandbox_denies_tcp_access() { assert!(run_sandboxed("fn main() { std::net::tcp_listen(\"127.0.0.1:0\") }").is_err()); assert!(run_sandboxed("fn main() { std::tls::connect(\"127.0.0.1:443\",\"localhost\") }").is_err()); assert!(run_sandboxed("fn main() { std::postgres::connect(\"postgresql://localhost/test\") }").is_err()); assert!(run_sandboxed("fn main() { std::postgres::connect_tls(\"postgresql://localhost/test\") }").is_err()); assert!(run_sandboxed("fn main() { std::postgres::pool(\"postgresql://localhost/test\",4,true) }").is_err()); assert!(run_sandboxed("fn main() { std::mysql::connect(\"mysql://localhost/test\") }").is_err()); assert!(run_sandboxed("fn main() { std::mysql::pool(\"mysql://localhost/test\",4) }").is_err()); }
    #[test] fn runtime_errors_are_reported() { assert!(run("fn main() { 1 / 0 }").is_err()); }
}
