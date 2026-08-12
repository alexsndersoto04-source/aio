//! Debug Adapter Protocol server backed by Titan's real VM debugger.

use serde_json::{json, Value as Json};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;
use thiserror::Error;
use titan_codegen::{BytecodeArtifact, CompiledModule};
use titan_vm::{
    Breakpoint, DebugCommand, DebugController, DebugEvent, DebugFrame, Debugger, Value, Vm,
};

#[derive(Error, Debug)]
pub enum DapError {
    #[error("DAP I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid DAP JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid DAP frame: {0}")]
    Frame(String),
}

pub fn run_stdio() -> Result<(), DapError> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        loop {
            match read_message(&mut input) {
                Ok(Some(message)) => {
                    if sender.send(Ok(message)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        }
    });
    let stdout = io::stdout();
    run(receiver, stdout.lock())
}

struct Session {
    module: Option<CompiledModule>,
    breakpoints: Vec<Breakpoint>,
    controller: Option<DebugController>,
    worker: Option<JoinHandle<Result<Option<Value>, titan_vm::VmError>>>,
    frame: Option<DebugFrame>,
    output: Option<Receiver<String>>,
    sandbox: bool,
}
impl Session {
    fn new() -> Self {
        Self {
            module: None,
            breakpoints: Vec::new(),
            controller: None,
            worker: None,
            frame: None,
            output: None,
            sandbox: false,
        }
    }
}

pub fn run<W: Write>(
    requests: Receiver<Result<Json, DapError>>,
    mut output: W,
) -> Result<(), DapError> {
    let mut session = Session::new();
    let mut sequence = 1i64;
    let mut disconnected = false;
    loop {
        drain_debug_events(&mut session, &mut output, &mut sequence)?;
        if disconnected && session.controller.is_none() {
            break;
        }
        match requests.recv_timeout(Duration::from_millis(10)) {
            Ok(Ok(request)) => {
                let request_seq = request.get("seq").and_then(Json::as_i64).unwrap_or(0);
                let command = request
                    .get("command")
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string();
                let arguments = request.get("arguments").cloned().unwrap_or(Json::Null);
                let response = handle_request(&command, &arguments, &mut session);
                match response {
                    Ok(body) => send_response(
                        &mut output,
                        &mut sequence,
                        request_seq,
                        &command,
                        true,
                        body,
                        None,
                    )?,
                    Err(message) => send_response(
                        &mut output,
                        &mut sequence,
                        request_seq,
                        &command,
                        false,
                        Json::Null,
                        Some(message),
                    )?,
                }
                if command == "initialize" {
                    send_event(&mut output, &mut sequence, "initialized", json!({}))?;
                }
                if command == "continue"
                    || command == "next"
                    || command == "stepIn"
                    || command == "stepOut"
                {
                    send_event(
                        &mut output,
                        &mut sequence,
                        "continued",
                        json!({"threadId":1,"allThreadsContinued":true}),
                    )?;
                }
                if command == "disconnect" {
                    disconnected = true;
                }
            }
            Ok(Err(error)) => return Err(error),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if session.controller.is_none() {
                    break;
                }
            }
        }
    }
    if let Some(worker) = session.worker.take() {
        if worker.join().is_err() {
            return Err(DapError::Frame("debuggee worker panicked".into()));
        }
    }
    Ok(())
}

fn handle_request(command: &str, arguments: &Json, session: &mut Session) -> Result<Json, String> {
    match command {
        "initialize" => Ok(
            json!({"supportsConfigurationDoneRequest":true,"supportsTerminateRequest":true,"supportsRestartRequest":false,"supportsSetVariable":false,"supportsStepBack":false,"supportsEvaluateForHovers":false}),
        ),
        "launch" => {
            let program = arguments
                .get("program")
                .and_then(Json::as_str)
                .ok_or("launch.program is required")?;
            session.sandbox = arguments
                .get("sandbox")
                .and_then(Json::as_bool)
                .unwrap_or(false);
            session.module = Some(load_program(program)?);
            Ok(json!({}))
        }
        "setBreakpoints" => set_breakpoints(arguments, session),
        "configurationDone" => {
            start_debuggee(session)?;
            Ok(json!({}))
        }
        "threads" => Ok(json!({"threads":[{"id":1,"name":"Titan main thread"}]})),
        "stackTrace" => Ok(
            json!({"stackFrames":session.frame.as_ref().map(stack_frame).into_iter().collect::<Vec<_>>(),"totalFrames":usize::from(session.frame.is_some())}),
        ),
        "scopes" => Ok(
            json!({"scopes":[{"name":"Locals","variablesReference":100,"expensive":false},{"name":"Operand Stack","variablesReference":101,"expensive":false}]}),
        ),
        "variables" => {
            let reference = arguments
                .get("variablesReference")
                .and_then(Json::as_i64)
                .ok_or("variablesReference is required")?;
            Ok(json!({"variables":variables(session.frame.as_ref(), reference)}))
        }
        "continue" => {
            command_debugger(session, DebugCommand::Continue)?;
            Ok(json!({"allThreadsContinued":true}))
        }
        "next" => {
            command_debugger(session, DebugCommand::StepOver)?;
            Ok(json!({}))
        }
        "stepIn" => {
            command_debugger(session, DebugCommand::StepIn)?;
            Ok(json!({}))
        }
        "stepOut" => {
            command_debugger(session, DebugCommand::StepOut)?;
            Ok(json!({}))
        }
        "pause" => {
            command_debugger(session, DebugCommand::Pause)?;
            Ok(json!({}))
        }
        "terminate" | "disconnect" => {
            if let Some(controller) = &session.controller {
                let _ = controller.command(DebugCommand::Terminate);
            }
            Ok(json!({}))
        }
        _ => Err(format!("unsupported DAP command '{command}'")),
    }
}

fn load_program(program: &str) -> Result<CompiledModule, String> {
    let path = Path::new(program);
    if path.extension().and_then(|value| value.to_str()) == Some("tbc") {
        return BytecodeArtifact::decode(&std::fs::read(path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string());
    }
    let entry = titan_pkg::default_entry(path);
    let project = titan_pkg::SourceProject::load(entry).map_err(|error| error.to_string())?;
    let mut types = titan_typechecker::TypeEnv::new();
    types.check_program(&project.program).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    titan_codegen::AstCompiler::new()
        .compile_program(&project.program)
        .map_err(|error| error.to_string())
}

fn set_breakpoints(arguments: &Json, session: &mut Session) -> Result<Json, String> {
    let source = arguments
        .get("source")
        .and_then(|source| source.get("path"))
        .and_then(Json::as_str)
        .ok_or("source.path is required")?;
    let canonical = PathBuf::from(source)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let source = canonical.to_string_lossy().into_owned();
    session.breakpoints.retain(|breakpoint| !matches!(breakpoint, Breakpoint::SourceLine { source_file, .. } if source_file == &source));
    let mut results = Vec::new();
    for requested in arguments
        .get("breakpoints")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        let line = requested
            .get("line")
            .and_then(Json::as_u64)
            .ok_or("breakpoint.line is required")? as usize;
        let verified = session.module.as_ref().is_some_and(|module| {
            module.functions.iter().any(|function| {
                function.source_file.as_deref() == Some(&source)
                    && function
                        .debug_locations
                        .iter()
                        .flatten()
                        .any(|location| location.line == line)
            })
        });
        if verified {
            session.breakpoints.push(Breakpoint::SourceLine {
                source_file: source.clone(),
                line,
            });
        }
        results.push(json!({"verified":verified,"line":line,"source":{"path":source.clone()},"message":if verified { Json::Null } else { json!("no executable instruction on this line") }}));
    }
    Ok(json!({"breakpoints":results}))
}

fn start_debuggee(session: &mut Session) -> Result<(), String> {
    if session.controller.is_some() {
        return Err("debuggee already started".into());
    }
    let module = session
        .module
        .take()
        .ok_or("launch must be sent before configurationDone")?;
    let (controller, mut debugger) = Debugger::channel(session.breakpoints.clone());
    let (output_tx, output_rx) = mpsc::channel();
    let sandbox = session.sandbox;
    let worker = std::thread::spawn(move || {
        let vm = if sandbox {
            Vm::sandboxed(module)
        } else {
            Vm::new(module)
        };
        let mut vm = vm.with_output_sender(output_tx);
        vm.run_debug(&mut debugger)
    });
    session.controller = Some(controller);
    session.worker = Some(worker);
    session.output = Some(output_rx);
    Ok(())
}
fn command_debugger(session: &Session, command: DebugCommand) -> Result<(), String> {
    session
        .controller
        .as_ref()
        .ok_or("debuggee is not running")?
        .command(command)
}

fn drain_debug_events(
    output_session: &mut Session,
    output: &mut impl Write,
    sequence: &mut i64,
) -> Result<(), DapError> {
    if let Some(receiver) = &output_session.output {
        while let Ok(line) = receiver.try_recv() {
            send_event(
                output,
                sequence,
                "output",
                json!({"category":"stdout","output":format!("{line}\n")}),
            )?;
        }
    }
    let event = output_session
        .controller
        .as_ref()
        .and_then(|controller| controller.try_recv().ok().flatten());
    if let Some(event) = event {
        match event {
            DebugEvent::Stopped(frame) => {
                let reason = if frame.depth > 0 {
                    "step"
                } else {
                    "breakpoint"
                };
                output_session.frame = Some(frame);
                send_event(
                    output,
                    sequence,
                    "stopped",
                    json!({"reason":reason,"threadId":1,"allThreadsStopped":true}),
                )?;
            }
            DebugEvent::Terminated { error } => {
                if let Some(error) = error {
                    send_event(
                        output,
                        sequence,
                        "output",
                        json!({"category":"stderr","output":format!("{error}\n")}),
                    )?;
                }
                send_event(output, sequence, "terminated", json!({}))?;
                output_session.controller = None;
                output_session.frame = None;
            }
        }
    }
    Ok(())
}
fn stack_frame(frame: &DebugFrame) -> Json {
    json!({"id":1,"name":frame.function_name,"line":frame.location.map_or(1,|location|location.line),"column":frame.location.map_or(1,|location|location.column),"source":frame.source_file.as_ref().map(|path|json!({"name":Path::new(path).file_name().and_then(|name|name.to_str()).unwrap_or(path),"path":path}))})
}
fn variables(frame: Option<&DebugFrame>, reference: i64) -> Vec<Json> {
    let Some(frame) = frame else {
        return Vec::new();
    };
    let values = if reference == 100 {
        &frame.locals
    } else if reference == 101 {
        &frame.stack
    } else {
        return Vec::new();
    };
    values.iter().enumerate().map(|(index,value)|json!({"name":index.to_string(),"value":titan_vm::val_to_string(value),"type":value_type(value),"variablesReference":0})).collect()
}
fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Bool(_) => "bool",
        Value::Char(_) => "char",
        Value::Str(_) => "string",
        Value::Bytes(_) => "bytes",
        Value::Nil => "nil",
        Value::Array(_) => "array",
        Value::Tuple(_) => "tuple",
        Value::Map(_) => "map",
        Value::Struct { .. } => "struct",
        Value::Enum { .. } => "enum",
        Value::Closure { .. } => "closure",
        Value::Task(_) => "task",
        Value::ChannelSender(_) => "sender",
        Value::ChannelReceiver(_) => "receiver",
        Value::TcpListener(_) => "tcp-listener",
        Value::TcpStream(_) => "tcp-stream",
        Value::HttpRouter(_) => "http-router",
        Value::TlsStream(_) => "tls-stream",
        Value::TlsServerConfig(_) => "tls-server-config",
        Value::WebSocketDecoder(_) => "websocket-decoder",
        Value::WebSocket(_) => "websocket",
        Value::ServerControl(_) => "server-control",
        Value::Sqlite(_) => "sqlite",
        Value::SqlitePool(_) => "sqlite-pool",
        Value::Postgres(_) => "postgres",
        Value::PostgresPool(_) => "postgres-pool",
        Value::Mysql(_) => "mysql",
        Value::MysqlPool(_) => "mysql-pool",
    }
}

fn send_response(
    output: &mut impl Write,
    sequence: &mut i64,
    request_seq: i64,
    command: &str,
    success: bool,
    body: Json,
    message: Option<String>,
) -> Result<(), DapError> {
    let value = json!({"seq":*sequence,"type":"response","request_seq":request_seq,"success":success,"command":command,"body":body,"message":message});
    *sequence += 1;
    write_message(output, &value)
}
fn send_event(
    output: &mut impl Write,
    sequence: &mut i64,
    event: &str,
    body: Json,
) -> Result<(), DapError> {
    let value = json!({"seq":*sequence,"type":"event","event":event,"body":body});
    *sequence += 1;
    write_message(output, &value)
}
fn read_message(input: &mut impl BufRead) -> Result<Option<Json>, DapError> {
    let mut length = None;
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some(value) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
        {
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| DapError::Frame("invalid Content-Length".into()))?,
            );
        }
    }
    let length = length.ok_or_else(|| DapError::Frame("missing Content-Length".into()))?;
    if length > 16 * 1024 * 1024 {
        return Err(DapError::Frame("message exceeds 16 MiB".into()));
    }
    let mut body = vec![0; length];
    std::io::Read::read_exact(input, &mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}
fn write_message(output: &mut impl Write, value: &Json) -> Result<(), DapError> {
    let body = serde_json::to_vec(value)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loads_source_and_artifact_programs() {
        let root = std::env::temp_dir().join(format!("titan-dap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        titan_pkg::create_project(&root, "dap_test").unwrap();
        let module = load_program(root.to_str().unwrap()).unwrap();
        let artifact = BytecodeArtifact::encode(&module).unwrap();
        let artifact_path = root.join("program.tbc");
        std::fs::write(&artifact_path, artifact).unwrap();
        assert_eq!(
            load_program(artifact_path.to_str().unwrap())
                .unwrap()
                .functions
                .len(),
            module.functions.len()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn serves_initialize_and_disconnect() {
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(
            json!({"seq":1,"type":"request","command":"initialize","arguments":{}}),
        ))
        .unwrap();
        tx.send(Ok(
            json!({"seq":2,"type":"request","command":"disconnect","arguments":{}}),
        ))
        .unwrap();
        drop(tx);
        let mut output = Vec::new();
        run(rx, &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("supportsConfigurationDoneRequest"));
        assert!(text.contains("initialized"));
        assert!(text.contains("disconnect"));
    }
}
