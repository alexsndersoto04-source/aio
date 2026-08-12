//! std::process — Ejecución de programas externos, señales y control del entorno.
//!
//! Cubre ejecución sincrónica con captura acotada de stdout/stderr, procesos
//! background aislados por runtime, pipelines, entorno y señales. El backend
//! es `std::process::Command`; no se simulan procesos ni resultados.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Instant;

use crate::process::{reserve_processes, ProcessPermit, MAX_RUNTIME_PROCESSES};

const MAX_COMMAND_BYTES: usize = 64 * 1024;
const MAX_PROCESS_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_CAPTURE_BYTES: usize = 4 * 1024 * 1024;
const MAX_PIPE_COMMANDS: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub enum ProcessError {
    Spawn(String),
    Io(String),
    UnknownHandle(u64),
    ShellSyntax(String),
    Signal(String),
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::Spawn(e) => write!(f, "spawn error: {e}"),
            ProcessError::Io(e) => write!(f, "process io error: {e}"),
            ProcessError::UnknownHandle(h) => write!(f, "unknown process handle {h}"),
            ProcessError::ShellSyntax(e) => write!(f, "shell syntax error: {e}"),
            ProcessError::Signal(e) => write!(f, "signal error: {e}"),
            ProcessError::ResourceLimit { resource, limit } => {
                write!(f, "{resource} exceeds limit {limit}")
            }
        }
    }
}

impl std::error::Error for ProcessError {}

/// Resultado completo de una ejecución sincrónica.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

struct CaptureBudget {
    used: AtomicUsize,
    limit: usize,
    exceeded: AtomicBool,
}

impl CaptureBudget {
    fn new(limit: usize) -> Self {
        Self {
            used: AtomicUsize::new(0),
            limit,
            exceeded: AtomicBool::new(false),
        }
    }

    fn append(&self, output: &mut Vec<u8>, chunk: &[u8]) {
        let mut used = self.used.load(Ordering::Acquire);
        loop {
            let available = self.limit.saturating_sub(used);
            let accepted = available.min(chunk.len());
            match self.used.compare_exchange_weak(
                used,
                used + accepted,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    output.extend_from_slice(&chunk[..accepted]);
                    if accepted < chunk.len() {
                        self.exceeded.store(true, Ordering::Release);
                    }
                    return;
                }
                Err(current) => used = current,
            }
        }
    }
}

type ReaderThread = JoinHandle<Result<Vec<u8>, String>>;

fn spawn_bounded_reader<R: Read + Send + 'static>(
    mut reader: R,
    budget: Arc<CaptureBudget>,
) -> ReaderThread {
    std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let count = reader.read(&mut chunk).map_err(|error| error.to_string())?;
            if count == 0 {
                return Ok(output);
            }
            // Continue draining after saturation so a child cannot deadlock on
            // a full pipe; bytes beyond the cap are discarded, not allocated.
            budget.append(&mut output, &chunk[..count]);
        }
    })
}

fn join_reader(reader: ReaderThread) -> Result<Vec<u8>, ProcessError> {
    reader
        .join()
        .map_err(|_| ProcessError::Io("process output reader panicked".into()))?
        .map_err(ProcessError::Io)
}

struct ProcessCapture {
    stdout: ReaderThread,
    stderr: ReaderThread,
    budget: Arc<CaptureBudget>,
}

impl ProcessCapture {
    fn start(child: &mut Child) -> Result<Self, ProcessError> {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ProcessError::Io("process stdout pipe unavailable".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ProcessError::Io("process stderr pipe unavailable".into()))?;
        let budget = Arc::new(CaptureBudget::new(MAX_CAPTURE_BYTES));
        Ok(Self {
            stdout: spawn_bounded_reader(stdout, Arc::clone(&budget)),
            stderr: spawn_bounded_reader(stderr, Arc::clone(&budget)),
            budget,
        })
    }

    fn finish(self) -> Result<(Vec<u8>, Vec<u8>), ProcessError> {
        let stdout = join_reader(self.stdout);
        let stderr = join_reader(self.stderr);
        let exceeded = self.budget.exceeded.load(Ordering::Acquire);
        let stdout = stdout?;
        let stderr = stderr?;
        if exceeded {
            return Err(ProcessError::ResourceLimit {
                resource: "captured process output bytes",
                limit: MAX_CAPTURE_BYTES,
            });
        }
        Ok((stdout, stderr))
    }
}

fn start_input_writer(
    child: &mut Child,
    input: Option<&[u8]>,
) -> Result<Option<JoinHandle<Result<(), String>>>, ProcessError> {
    let Some(input) = input else {
        return Ok(None);
    };
    if input.len() > MAX_PROCESS_INPUT_BYTES {
        return Err(ProcessError::ResourceLimit {
            resource: "process input bytes",
            limit: MAX_PROCESS_INPUT_BYTES,
        });
    }
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| ProcessError::Io("process stdin pipe unavailable".into()))?;
    let input = input.to_vec();
    Ok(Some(std::thread::spawn(move || {
        stdin.write_all(&input).map_err(|error| error.to_string())
    })))
}

fn finish_process(
    mut child: Child,
    capture: ProcessCapture,
    writer: Option<JoinHandle<Result<(), String>>>,
    started: Instant,
) -> Result<ProcessOutput, ProcessError> {
    let status = child
        .wait()
        .map_err(|error| ProcessError::Io(error.to_string()));
    let write_result = writer.map(|writer| {
        writer
            .join()
            .map_err(|_| ProcessError::Io("process input writer panicked".into()))?
            .map_err(ProcessError::Io)
    });
    let captured = capture.finish();
    let status = status?;
    if let Some(write_result) = write_result {
        write_result?;
    }
    let (stdout, stderr) = captured?;
    Ok(ProcessOutput {
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        exit_code: status.code().unwrap_or(-1),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn execute_command(
    mut command: Command,
    input: Option<&[u8]>,
) -> Result<ProcessOutput, ProcessError> {
    if input.is_some_and(|input| input.len() > MAX_PROCESS_INPUT_BYTES) {
        return Err(ProcessError::ResourceLimit {
            resource: "process input bytes",
            limit: MAX_PROCESS_INPUT_BYTES,
        });
    }
    let _permit = reserve_processes(1).map_err(|_| ProcessError::ResourceLimit {
        resource: "process count",
        limit: MAX_RUNTIME_PROCESSES,
    })?;
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|error| ProcessError::Spawn(error.to_string()))?;
    let capture = match ProcessCapture::start(&mut child) {
        Ok(capture) => capture,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let writer = match start_input_writer(&mut child, input) {
        Ok(writer) => writer,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = capture.finish();
            return Err(error);
        }
    };
    finish_process(child, capture, writer, started)
}

struct BackgroundProcess {
    child: Child,
    capture: ProcessCapture,
    started: Instant,
    _permit: ProcessPermit,
}

// Background-process registry partitioned by VM runtime ownership.
static REGISTRY: OnceLock<Mutex<HashMap<(u64, u64), BackgroundProcess>>> = OnceLock::new();
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

fn registry() -> &'static Mutex<HashMap<(u64, u64), BackgroundProcess>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_handle() -> u64 {
    NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
}

fn handle_key(handle: u64) -> (u64, u64) {
    crate::native::runtime_handle_key(handle)
}

fn validate_command(command: &str) -> Result<(), ProcessError> {
    if command.len() > MAX_COMMAND_BYTES {
        return Err(ProcessError::ResourceLimit {
            resource: "command bytes",
            limit: MAX_COMMAND_BYTES,
        });
    }
    Ok(())
}

/// Parsea "cmd arg1 arg2" respetando comillas simples/dobles.
/// Sin soporte de escapes complejos (para eso, usar `shell()`).
pub fn parse_cmd(cmd: &str) -> Result<Vec<String>, ProcessError> {
    validate_command(cmd)?;
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in cmd.chars() {
        match (c, quote) {
            (q, Some(open)) if q == open => quote = None,
            (c, Some(_)) => current.push(c),
            ('"' | '\'', None) => quote = Some(c),
            (c, None) if c.is_whitespace() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            (c, None) => current.push(c),
        }
    }
    if quote.is_some() {
        return Err(ProcessError::ShellSyntax("unclosed quote".into()));
    }
    if !current.is_empty() {
        parts.push(current);
    }
    if parts.is_empty() {
        return Err(ProcessError::ShellSyntax("empty command".into()));
    }
    Ok(parts)
}

/// Ejecuta un comando con captura acotada de stdout y stderr.
pub fn run(cmd: &str) -> Result<ProcessOutput, ProcessError> {
    let parts = parse_cmd(cmd)?;
    let mut command = Command::new(&parts[0]);
    command.args(&parts[1..]);
    execute_command(command, None)
}

/// Ejecuta un comando alimentando stdin y drenando stdout/stderr en paralelo.
pub fn run_with_input(cmd: &str, input: &[u8]) -> Result<ProcessOutput, ProcessError> {
    let parts = parse_cmd(cmd)?;
    let mut command = Command::new(&parts[0]);
    command.args(&parts[1..]);
    execute_command(command, Some(input))
}

/// Ejecuta el comando via `sh -c` (permite pipes, redirecciones, glob).
pub fn shell(cmd: &str) -> Result<ProcessOutput, ProcessError> {
    validate_command(cmd)?;
    let mut command = Command::new("sh");
    command.arg("-c").arg(cmd);
    execute_command(command, None)
}

fn terminate_pipeline(children: &mut [Child]) {
    for child in children.iter_mut() {
        let _ = child.kill();
    }
    for child in children.iter_mut() {
        let _ = child.wait();
    }
}

/// Encadena comandos, drenando en paralelo todos los stderr y el stdout final.
pub fn pipe(cmds: &[String]) -> Result<ProcessOutput, ProcessError> {
    if cmds.is_empty() {
        return Err(ProcessError::ShellSyntax("empty pipe".into()));
    }
    if cmds.len() > MAX_PIPE_COMMANDS {
        return Err(ProcessError::ResourceLimit {
            resource: "pipeline commands",
            limit: MAX_PIPE_COMMANDS,
        });
    }
    let parsed = cmds
        .iter()
        .map(|command| parse_cmd(command))
        .collect::<Result<Vec<_>, _>>()?;
    let _permit = reserve_processes(parsed.len()).map_err(|_| ProcessError::ResourceLimit {
        resource: "process count",
        limit: MAX_RUNTIME_PROCESSES,
    })?;
    let started = Instant::now();
    let budget = Arc::new(CaptureBudget::new(MAX_CAPTURE_BYTES));
    let mut previous_stdout = None;
    let mut children = Vec::with_capacity(parsed.len());
    let mut stderr_readers = Vec::with_capacity(parsed.len());
    let mut stdout_reader = None;

    for (index, parts) in parsed.iter().enumerate() {
        let is_last = index + 1 == parsed.len();
        let mut command = Command::new(&parts[0]);
        command.args(&parts[1..]);
        command.stdin(previous_stdout.take().map_or_else(Stdio::null, Stdio::from));
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                terminate_pipeline(&mut children);
                for reader in stderr_readers {
                    let _ = join_reader(reader);
                }
                return Err(ProcessError::Spawn(error.to_string()));
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                children.push(child);
                terminate_pipeline(&mut children);
                for reader in stderr_readers {
                    let _ = join_reader(reader);
                }
                return Err(ProcessError::Io("pipeline stderr pipe unavailable".into()));
            }
        };
        stderr_readers.push(spawn_bounded_reader(stderr, Arc::clone(&budget)));
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                children.push(child);
                terminate_pipeline(&mut children);
                for reader in stderr_readers {
                    let _ = join_reader(reader);
                }
                return Err(ProcessError::Io("pipeline stdout pipe unavailable".into()));
            }
        };
        if is_last {
            stdout_reader = Some(spawn_bounded_reader(stdout, Arc::clone(&budget)));
        } else {
            previous_stdout = Some(stdout);
        }
        children.push(child);
    }

    let last_index = children.len() - 1;
    let last_status = children[last_index]
        .wait()
        .map_err(|error| ProcessError::Io(error.to_string()));
    let mut wait_error = None;
    for child in &mut children[..last_index] {
        if let Err(error) = child.wait() {
            if wait_error.is_none() {
                wait_error = Some(ProcessError::Io(error.to_string()));
            }
        }
    }
    let stdout = join_reader(stdout_reader.expect("nonempty pipeline has final stdout"));
    let mut stderr = Vec::new();
    let mut read_error = None;
    for reader in stderr_readers {
        match join_reader(reader) {
            Ok(bytes) => stderr.extend_from_slice(&bytes),
            Err(error) => {
                if read_error.is_none() {
                    read_error = Some(error);
                }
            }
        }
    }
    let status = last_status?;
    if let Some(error) = wait_error {
        return Err(error);
    }
    let stdout = stdout?;
    if let Some(error) = read_error {
        return Err(error);
    }
    if budget.exceeded.load(Ordering::Acquire) {
        return Err(ProcessError::ResourceLimit {
            resource: "captured process output bytes",
            limit: MAX_CAPTURE_BYTES,
        });
    }
    Ok(ProcessOutput {
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        exit_code: status.code().unwrap_or(-1),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

/// Spawna un proceso en background y devuelve un handle acotado por runtime.
pub fn spawn_bg(cmd: &str) -> Result<u64, ProcessError> {
    let parts = parse_cmd(cmd)?;
    let permit = reserve_processes(1).map_err(|_| ProcessError::ResourceLimit {
        resource: "process count",
        limit: MAX_RUNTIME_PROCESSES,
    })?;
    let started = Instant::now();
    let mut child = Command::new(&parts[0])
        .args(&parts[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ProcessError::Spawn(error.to_string()))?;
    let capture = match ProcessCapture::start(&mut child) {
        Ok(capture) => capture,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let handle = next_handle();
    crate::native::lock_recover(registry()).insert(
        handle_key(handle),
        BackgroundProcess {
            child,
            capture,
            started,
            _permit: permit,
        },
    );
    Ok(handle)
}

/// Espera a que un proceso background termine y libera su slot aun si falla.
pub fn spawn_wait(handle: u64) -> Result<ProcessOutput, ProcessError> {
    let background = crate::native::lock_recover(registry())
        .remove(&handle_key(handle))
        .ok_or(ProcessError::UnknownHandle(handle))?;
    finish_process(
        background.child,
        background.capture,
        None,
        background.started,
    )
}

/// Chequea si un proceso background ya terminó (sin bloquear).
pub fn spawn_poll(handle: u64) -> Result<Option<i32>, ProcessError> {
    let mut registry = crate::native::lock_recover(registry());
    let background = registry
        .get_mut(&handle_key(handle))
        .ok_or(ProcessError::UnknownHandle(handle))?;
    match background
        .child
        .try_wait()
        .map_err(|error| ProcessError::Io(error.to_string()))?
    {
        Some(status) => Ok(Some(status.code().unwrap_or(-1))),
        None => Ok(None),
    }
}

/// Mata un proceso background (SIGKILL). `spawn_wait` recolecta el handle.
pub fn spawn_kill(handle: u64) -> Result<(), ProcessError> {
    let mut registry = crate::native::lock_recover(registry());
    let background = registry
        .get_mut(&handle_key(handle))
        .ok_or(ProcessError::UnknownHandle(handle))?;
    background
        .child
        .kill()
        .map_err(|error| ProcessError::Io(error.to_string()))
}

/// PID de un handle background.
pub fn spawn_pid(handle: u64) -> Result<u32, ProcessError> {
    let registry = crate::native::lock_recover(registry());
    let background = registry
        .get(&handle_key(handle))
        .ok_or(ProcessError::UnknownHandle(handle))?;
    Ok(background.child.id())
}

// --- Environment ---

pub fn env_get(name: &str) -> Option<String> {
    std::env::var(name).ok()
}
pub fn env_set(name: &str, value: &str) {
    std::env::set_var(name, value);
}
pub fn env_unset(name: &str) {
    std::env::remove_var(name);
}

pub fn env_vars() -> Vec<(String, String)> {
    std::env::vars().collect()
}

pub fn working_dir() -> Result<String, ProcessError> {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| ProcessError::Io(e.to_string()))
}

pub fn set_working_dir(path: &str) -> Result<(), ProcessError> {
    std::env::set_current_dir(path).map_err(|e| ProcessError::Io(e.to_string()))
}

// --- Info del proceso actual y del sistema ---

pub fn self_pid() -> u32 {
    std::process::id()
}

pub fn hostname() -> String {
    // Sin depender de crates externos: leer /proc/sys/kernel/hostname o
    // caer a HOSTNAME env var.
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "unknown".into())
}

pub fn username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

pub fn args() -> Vec<String> {
    std::env::args().collect()
}

/// Termina el proceso actual con el código dado.
pub fn exit(code: i32) -> ! {
    std::process::exit(code)
}

/// Manda una señal POSIX a un PID cualquiera del sistema.
/// Usa la infraestructura de nix; solo funciona en Unix.
/// Signal codes: 1=HUP, 2=INT, 9=KILL, 15=TERM, 18=CONT, 19=STOP.
#[cfg(unix)]
pub fn send_signal(pid: i32, signal_num: i32) -> Result<(), ProcessError> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let sig = Signal::try_from(signal_num).map_err(|e| ProcessError::Signal(e.to_string()))?;
    kill(Pid::from_raw(pid), sig).map_err(|e| ProcessError::Signal(e.to_string()))
}

#[cfg(not(unix))]
pub fn send_signal(_pid: i32, _signal_num: i32) -> Result<(), ProcessError> {
    Err(ProcessError::Signal(
        "signals only supported on Unix".into(),
    ))
}

fn terminate_background(mut background: BackgroundProcess) {
    if !matches!(background.child.try_wait(), Ok(Some(_))) {
        let _ = background.child.kill();
    }
    let _ = background.child.wait();
    let _ = background.capture.finish();
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let children = {
        let mut registry = crate::native::lock_recover(registry());
        let keys = registry
            .keys()
            .filter(|(owner, _)| *owner == runtime_id)
            .copied()
            .collect::<Vec<_>>();
        keys.into_iter()
            .filter_map(|key| registry.remove(&key))
            .collect::<Vec<_>>()
    };
    let released = children.len();
    for background in children {
        terminate_background(background);
    }
    released
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_echo_returns_stdout() {
        let out = run("echo hola-titan").unwrap();
        assert!(out.stdout.contains("hola-titan"));
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn run_with_input_pipes_stdin_without_pipe_deadlock() {
        let out = run_with_input("cat", b"hello").unwrap();
        assert_eq!(out.stdout, "hello");
    }

    #[test]
    fn parse_respects_quotes_and_rejects_oversized_commands() {
        let parts = parse_cmd("echo 'hello world' foo").unwrap();
        assert_eq!(parts, vec!["echo", "hello world", "foo"]);
        assert_eq!(
            parse_cmd(&"x".repeat(MAX_COMMAND_BYTES + 1)).unwrap_err(),
            ProcessError::ResourceLimit {
                resource: "command bytes",
                limit: MAX_COMMAND_BYTES,
            }
        );
    }

    #[test]
    fn shell_supports_pipes() {
        let out = shell("echo hello | tr a-z A-Z").unwrap();
        assert!(out.stdout.contains("HELLO"));
    }

    #[cfg(unix)]
    #[test]
    fn pipeline_drains_intermediate_stderr_larger_than_an_os_pipe() {
        let out = pipe(&[
            "sh -c 'head -c 131072 /dev/zero >&2; echo hello'".into(),
            "cat".into(),
        ])
        .unwrap();
        assert_eq!(out.stdout.trim(), "hello");
        assert_eq!(out.stderr.len(), 131_072);
    }

    #[cfg(unix)]
    #[test]
    fn input_writer_and_output_readers_run_concurrently() {
        let input = vec![b'x'; 131_072];
        let out = run_with_input("sh -c 'head -c 131072 /dev/zero >&2; cat'", &input).unwrap();
        assert_eq!(out.stdout.as_bytes(), input);
        assert_eq!(out.stderr.len(), 131_072);
    }

    #[cfg(unix)]
    #[test]
    fn output_above_the_combined_capture_limit_is_rejected() {
        assert_eq!(
            shell(&format!("head -c {} /dev/zero", MAX_CAPTURE_BYTES + 1)).unwrap_err(),
            ProcessError::ResourceLimit {
                resource: "captured process output bytes",
                limit: MAX_CAPTURE_BYTES,
            }
        );
    }

    #[test]
    fn capture_budget_keeps_only_the_bounded_prefix_and_continues_draining() {
        let budget = Arc::new(CaptureBudget::new(5));
        let reader = spawn_bounded_reader(
            std::io::Cursor::new(b"123456789".to_vec()),
            Arc::clone(&budget),
        );
        assert_eq!(join_reader(reader).unwrap(), b"12345");
        assert!(budget.exceeded.load(Ordering::Acquire));
    }

    #[test]
    fn input_and_pipeline_counts_are_rejected_before_spawning() {
        assert_eq!(
            run_with_input("cat", &vec![0; MAX_PROCESS_INPUT_BYTES + 1]).unwrap_err(),
            ProcessError::ResourceLimit {
                resource: "process input bytes",
                limit: MAX_PROCESS_INPUT_BYTES,
            }
        );
        assert_eq!(
            pipe(&vec!["echo ok".into(); MAX_PIPE_COMMANDS + 1]).unwrap_err(),
            ProcessError::ResourceLimit {
                resource: "pipeline commands",
                limit: MAX_PIPE_COMMANDS,
            }
        );
    }

    #[test]
    fn process_quota_is_runtime_scoped_and_recovers() {
        let runtime_id = 8_000_001;
        crate::native::with_runtime_context(runtime_id, || {
            let mut permits = (0..MAX_RUNTIME_PROCESSES)
                .map(|_| reserve_processes(1).unwrap())
                .collect::<Vec<_>>();
            assert!(reserve_processes(1).is_err());
            permits.pop();
            permits.push(reserve_processes(1).unwrap());
            drop(permits);
        });
        assert_eq!(crate::process::active_processes(runtime_id), 0);
    }

    #[cfg(unix)]
    #[test]
    fn background_output_is_drained_before_wait() {
        let runtime_id = 8_000_003;
        let handle = crate::native::with_runtime_context(runtime_id, || {
            spawn_bg("sh -c 'head -c 131072 /dev/zero; echo done >&2'").unwrap()
        });
        let mut exited = false;
        for _ in 0..200 {
            exited = crate::native::with_runtime_context(runtime_id, || spawn_poll(handle))
                .unwrap()
                .is_some();
            if exited {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            exited,
            "background child remained blocked on an undrained pipe"
        );
        let output =
            crate::native::with_runtime_context(runtime_id, || spawn_wait(handle)).unwrap();
        assert_eq!(output.stdout.len(), 131_072);
        assert!(output.stderr.contains("done"));
        assert_eq!(crate::process::active_processes(runtime_id), 0);
    }

    #[cfg(unix)]
    #[test]
    fn runtime_cleanup_kills_and_reaps_background_processes() {
        let runtime_id = 8_000_002;
        let handle = crate::native::with_runtime_context(runtime_id, || {
            spawn_bg("sh -c 'sleep 30'").unwrap()
        });
        assert!(crate::native::with_runtime_context(runtime_id, || spawn_pid(handle)).is_ok());

        assert_eq!(cleanup_runtime(runtime_id), 1);
        assert_eq!(
            crate::native::with_runtime_context(runtime_id, || spawn_poll(handle)).unwrap_err(),
            ProcessError::UnknownHandle(handle)
        );
        assert_eq!(crate::process::active_processes(runtime_id), 0);
    }

    #[test]
    fn env_get_set_roundtrip() {
        env_set("TITAN_TEST_VAR", "42");
        assert_eq!(env_get("TITAN_TEST_VAR"), Some("42".into()));
        env_unset("TITAN_TEST_VAR");
        assert_eq!(env_get("TITAN_TEST_VAR"), None);
    }
}
