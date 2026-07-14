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
    TcpListener(u64), TcpStream(u64), HttpRouter(u64),
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
        Value::Task(id) => format!("<task:{id}>"), Value::ChannelSender(id) => format!("<sender:{id}>"), Value::ChannelReceiver(id) => format!("<receiver:{id}>"), Value::TcpListener(id) => format!("<tcp-listener:{id}>"), Value::TcpStream(id) => format!("<tcp-stream:{id}>"), Value::HttpRouter(id) => format!("<http-router:{id}>"),
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
#[derive(Default)] struct HttpRouterState { routes: Vec<HttpRoute>, middleware: Vec<HttpCallable>, after: Vec<HttpCallable> }
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
}
impl RuntimeState { fn new() -> Self { Self { next_task: AtomicU64::new(1), next_channel: AtomicU64::new(1), next_socket: AtomicU64::new(1), next_router: AtomicU64::new(1), tasks: Mutex::new(HashMap::new()), channels: Mutex::new(HashMap::new()), listeners: Mutex::new(HashMap::new()), streams: Mutex::new(HashMap::new()), routers: Mutex::new(HashMap::new()) } } }

pub struct Vm {
    module: CompiledModule,
    instruction_limit: usize,
    instructions: usize,
    max_call_depth: usize,
    capabilities: RuntimeCapabilities,
    output: Option<std::sync::mpsc::Sender<String>>,
    runtime: Arc<RuntimeState>,
    cancellation: Option<Arc<AtomicBool>>,
}

impl Vm {
    pub fn new(module: CompiledModule) -> Self { Self { module, instruction_limit: 10_000_000, instructions: 0, max_call_depth: 4096, capabilities: RuntimeCapabilities::all(), output: None, runtime: Arc::new(RuntimeState::new()), cancellation: None } }
    pub fn sandboxed(module: CompiledModule) -> Self { Self { capabilities: RuntimeCapabilities::sandboxed(), ..Self::new(module) } }
    pub fn with_instruction_limit(mut self, limit: usize) -> Self { self.instruction_limit = limit; self }
    pub fn with_capabilities(mut self, capabilities: RuntimeCapabilities) -> Self { self.capabilities = capabilities; self }
    pub fn with_output_sender(mut self, output: std::sync::mpsc::Sender<String>) -> Self { self.output = Some(output); self }

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
                Op::Add => binary(&mut stack, &function.name, add)?, Op::Sub => binary(&mut stack, &function.name, sub)?,
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
                Op::Spawn => {
                    let callable = pop(&mut stack, &function.name)?;
                    let Value::Closure { function: task_function, captures } = callable else { return Err(VmError::Type("spawn requires a closure".into())); };
                    let task_id = self.runtime.next_task.fetch_add(1, Ordering::Relaxed);
                    let module = self.module.clone(); let runtime = Arc::clone(&self.runtime); let capabilities = self.capabilities; let output = self.output.clone();
                    let cancelled = Arc::new(AtomicBool::new(false)); let task_cancelled = Arc::clone(&cancelled); let (result_tx, result_rx) = mpsc::sync_channel(1);
                    let handle = std::thread::spawn(move || { let mut child = Vm { module, instruction_limit: 10_000_000, instructions: 0, max_call_depth: 4096, capabilities, output, runtime, cancellation: Some(task_cancelled) }; let result = child.execute(task_function, Vec::new(), captures, 0, &mut None); let _ = result_tx.send(result); });
                    self.runtime.tasks.lock().map_err(|_| VmError::Type("task registry poisoned".into()))?.insert(task_id, TaskRecord { handle, result: result_rx, cancelled });
                    stack.push(Value::Task(task_id));
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
                Op::HttpDispatch => { let mut request=pop(&mut stack,&function.name)?;let Value::HttpRouter(router_id)=pop(&mut stack,&function.name)? else{return Err(VmError::Type("dispatch requires router".into()))};let router=http_router(&self.runtime,router_id)?;let (middleware,routes,after)={let router=router.lock().map_err(|_|VmError::Type("HTTP router poisoned".into()))?;(router.middleware.clone(),router.routes.clone(),router.after.clone())};for layer in middleware{request=self.execute(layer.function,vec![request],layer.captures,depth+1,debugger)?;if !matches!(&request,Value::Map(_)){return Err(VmError::Type("middleware must return request map".into()))}}let (method,path)=request_method_path(&request)?;let mut matched=None;for route in routes{if route.method==method{if let Some(params)=titan_stdlib::http::match_route(&route.pattern,&path).map_err(|error|VmError::Native{function:"std::http::dispatch".into(),message:error.to_string()})?{matched=Some((route,params));break}}}let mut response=if let Some((route,params))=matched{if let Value::Map(map)=&mut request{map.insert("params".into(),Value::Map(params.into_iter().map(|(key,value)|(key,Value::Str(value))).collect()));}self.execute(route.handler.function,vec![request],route.handler.captures,depth+1,debugger)?}else{http_not_found()};for layer in after.into_iter().rev(){response=self.execute(layer.function,vec![response],layer.captures,depth+1,debugger)?;if !matches!(&response,Value::Map(_)){return Err(VmError::Type("response middleware must return response map".into()))}}stack.push(response); }
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
                Op::ToString => { let value = pop(&mut stack, &function.name)?; stack.push(Value::Str(val_to_string(&value))); }
                Op::NewArray(count) => { let values = take_args(&mut stack, count, &function.name)?; stack.push(Value::Array(values)); }
                Op::NewTuple(count) => { let values = take_args(&mut stack, count, &function.name)?; stack.push(Value::Tuple(values)); }
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
fn add(a: Value, b: Value) -> Result<Value, VmError> { match (a, b) { (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.checked_add(b).ok_or(VmError::Overflow)?)), (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)), (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)), _ => Err(VmError::Type("addition requires matching numbers or strings".into())) } }
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
    #[test] fn native_json_maps_support_fields() { assert_eq!(run(r#"fn main() { std::json::parse("{\"answer\":42}").answer }"#).unwrap(), Value::Int(42)); }
    #[test] fn interpolation_concatenates_every_segment() { assert_eq!(run("fn double(x: int) -> int { x * 2 } fn main() { let i = 3 \"value({i}) = {double(i)}!\" }").unwrap(), Value::Str("value(3) = 6!".into())); }
    #[test] fn native_generic_arrays_accept_concrete_elements() { assert_eq!(run("fn main() { std::stats::mean([10, 20, 30, 40]) }").unwrap(), Value::Float(25.0)); }
    #[test] fn closures_capture_lexical_values() { assert_eq!(run("fn main() { let base = 10 let add = |value: int| -> int value + base add(5) }").unwrap(), Value::Int(15)); }
    #[test] fn named_functions_are_first_class() { assert_eq!(run("fn double(x: int) -> int { x * 2 } fn main() { let operation = double operation(21) }").unwrap(), Value::Int(42)); }
    #[test] fn functional_array_pipeline_works() { assert_eq!(run("fn main() { [1, 2, 3, 4].map(|x: int| x * 2).filter(|x: int| x > 4).fold(0, |sum: int, x: int| sum + x) }").unwrap(), Value::Int(14)); }
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
    #[test] fn http_routing_is_callable_from_titan() { assert_eq!(run("fn main() { let params = std::http::route_match(\"/users/:id\", \"/users/42\")? params.id }").unwrap(), Value::Str("42".into())); assert_eq!(run("fn main() { let query = std::http::parse_query(\"tag=rust&tag=titan\", 10) query.tag[1] }").unwrap(), Value::Str("titan".into())); }
    #[test] fn composed_router_dispatches_middleware_and_params() { assert_eq!(run("fn main(){let router=std::http::router() std::http::middleware(router,|request|request) std::http::route(router,\"GET\",\"/users/:id\",|request|request.params.id) let request=std::json::parse(\"{\\\"method\\\":\\\"GET\\\",\\\"path\\\":\\\"/users/42\\\"}\") std::http::dispatch(router,request)}").unwrap(),Value::Str("42".into())); assert!(matches!(run("fn main(){let router=std::http::router() let request=std::json::parse(\"{\\\"method\\\":\\\"GET\\\",\\\"path\\\":\\\"/missing\\\"}\") std::http::dispatch(router,request)}").unwrap(),Value::Map(_))); }
    #[test] fn response_middleware_and_rate_limits_work() { assert_eq!(run("fn main(){let router=std::http::router() std::http::route(router,\"GET\",\"/\",|request|std::json::parse(\"{\\\"status\\\":200,\\\"body\\\":\\\"ok\\\"}\")) std::http::after(router,|response|std::http::security_headers(response)) let request=std::json::parse(\"{\\\"method\\\":\\\"GET\\\",\\\"path\\\":\\\"/\\\"}\") let response=std::http::dispatch(router,request) std::map::get(response.headers,\"X-Frame-Options\")}").unwrap(),Value::Str("DENY".into())); assert_eq!(run("fn main(){let a=std::http::rate_limit(\"vm-test-key\",2,60000) let b=std::http::rate_limit(\"vm-test-key\",2,60000) let c=std::http::rate_limit(\"vm-test-key\",2,60000); [a,b,c]}").unwrap(),Value::Array(vec![Value::Bool(true),Value::Bool(true),Value::Bool(false)])); }
    #[test] fn high_level_http_server_invokes_titan_handler() { assert_eq!(run("fn main() { let listener=std::net::tcp_listen(\"127.0.0.1:0\") let address=std::net::tcp_local_addr(listener) let handler=|request| std::json::parse(\"{\\\"status\\\":200,\\\"headers\\\":{\\\"Content-Type\\\":\\\"text/plain\\\"},\\\"body\\\":\\\"hello titan\\\",\\\"keep_alive\\\":false}\") let server=spawn || std::http::serve_connection(listener,handler,10) let client=std::net::tcp_connect(address) std::net::tcp_write(client,\"GET / HTTP/1.1\\r\\nHost: localhost\\r\\nConnection: close\\r\\n\\r\\n\") let response=std::encoding::utf8_decode(std::net::tcp_read(client,4096)) std::net::tcp_close(client) join(server) std::net::tcp_close(listener) std::text::contains(response,\"hello titan\") }").unwrap(), Value::Bool(true)); }
    #[test] fn sandbox_denies_tcp_access() { assert!(run_sandboxed("fn main() { std::net::tcp_listen(\"127.0.0.1:0\") }").is_err()); }
    #[test] fn runtime_errors_are_reported() { assert!(run("fn main() { 1 / 0 }").is_err()); }
}
