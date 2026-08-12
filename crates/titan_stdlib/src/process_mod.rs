//! std::process — Ejecución de programas externos, señales y control del entorno.
//!
//! Cubre la superficie completa que necesita un lenguaje de scripting/DevOps
//! serio: ejecución sincrónica con captura de stdout/stderr, spawn de
//! procesos en background con handles reutilizables, pipes encadenadas,
//! variables de entorno, señales (kill/term/hup), info del sistema, y
//! terminación controlada. Backend: `std::process::Command` (nativo) + `nix`
//! para señales portátiles en Unix.

use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[derive(Debug)]
pub enum ProcessError {
    Spawn(String),
    Io(String),
    UnknownHandle(u64),
    ShellSyntax(String),
    Signal(String),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::Spawn(e)         => write!(f, "spawn error: {e}"),
            ProcessError::Io(e)            => write!(f, "process io error: {e}"),
            ProcessError::UnknownHandle(h) => write!(f, "unknown process handle {h}"),
            ProcessError::ShellSyntax(e)   => write!(f, "shell syntax error: {e}"),
            ProcessError::Signal(e)        => write!(f, "signal error: {e}"),
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

// Background-process registry partitioned by VM runtime ownership.
static REGISTRY: OnceLock<Mutex<HashMap<(u64, u64), Child>>> = OnceLock::new();
static NEXT_HANDLE: OnceLock<std::sync::atomic::AtomicU64> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<(u64, u64), Child>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_handle() -> u64 {
    NEXT_HANDLE
        .get_or_init(|| std::sync::atomic::AtomicU64::new(1))
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn handle_key(handle: u64) -> (u64, u64) { crate::native::runtime_handle_key(handle) }

/// Parsea "cmd arg1 arg2" respetando comillas simples/dobles.
/// Sin soporte de escapes complejos (para eso, usar `shell()`).
pub fn parse_cmd(cmd: &str) -> Result<Vec<String>, ProcessError> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in cmd.chars() {
        match (c, quote) {
            (q, Some(open)) if q == open => { quote = None; }
            (c, Some(_))                 => { current.push(c); }
            ('"' | '\'', None)           => { quote = Some(c); }
            (c, None) if c.is_whitespace() => {
                if !current.is_empty() { parts.push(std::mem::take(&mut current)); }
            }
            (c, None) => { current.push(c); }
        }
    }
    if quote.is_some() { return Err(ProcessError::ShellSyntax("unclosed quote".into())); }
    if !current.is_empty() { parts.push(current); }
    if parts.is_empty() { return Err(ProcessError::ShellSyntax("empty command".into())); }
    Ok(parts)
}

/// Ejecuta un comando y captura toda su salida.
pub fn run(cmd: &str) -> Result<ProcessOutput, ProcessError> {
    let parts = parse_cmd(cmd)?;
    let start = Instant::now();
    let output = Command::new(&parts[0])
        .args(&parts[1..])
        .output()
        .map_err(|e| ProcessError::Spawn(e.to_string()))?;
    Ok(ProcessOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Ejecuta un comando pasándole datos por stdin.
pub fn run_with_input(cmd: &str, input: &[u8]) -> Result<ProcessOutput, ProcessError> {
    let parts = parse_cmd(cmd)?;
    let start = Instant::now();
    let mut child = Command::new(&parts[0])
        .args(&parts[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ProcessError::Spawn(e.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input).map_err(|e| ProcessError::Io(e.to_string()))?;
    }
    let output = child.wait_with_output().map_err(|e| ProcessError::Io(e.to_string()))?;
    Ok(ProcessOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Ejecuta el comando via `sh -c` (permite pipes, redirecciones, glob).
pub fn shell(cmd: &str) -> Result<ProcessOutput, ProcessError> {
    let start = Instant::now();
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .map_err(|e| ProcessError::Spawn(e.to_string()))?;
    Ok(ProcessOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Encadena N comandos: stdout de cada uno alimenta stdin del siguiente.
pub fn pipe(cmds: &[String]) -> Result<ProcessOutput, ProcessError> {
    if cmds.is_empty() { return Err(ProcessError::ShellSyntax("empty pipe".into())); }
    let start = Instant::now();
    let mut prev_stdout: Option<std::process::ChildStdout> = None;
    let mut children: Vec<Child> = Vec::new();
    for (i, cmd) in cmds.iter().enumerate() {
        let parts = parse_cmd(cmd)?;
        let is_last = i == cmds.len() - 1;
        let mut command = Command::new(&parts[0]);
        command.args(&parts[1..]);
        if let Some(prev) = prev_stdout.take() {
            command.stdin(Stdio::from(prev));
        }
        command.stdout(if is_last { Stdio::piped() } else { Stdio::piped() });
        command.stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|e| ProcessError::Spawn(e.to_string()))?;
        if !is_last {
            prev_stdout = child.stdout.take();
        }
        children.push(child);
    }
    // El último es el que capturamos, los intermedios los esperamos también.
    let last = children.pop().unwrap();
    let output = last.wait_with_output().map_err(|e| ProcessError::Io(e.to_string()))?;
    let mut aggregated_stderr = String::new();
    for mut c in children {
        if let Some(mut stderr) = c.stderr.take() {
            let mut buf = String::new();
            let _ = std::io::Read::read_to_string(&mut stderr, &mut buf);
            aggregated_stderr.push_str(&buf);
        }
        let _ = c.wait();
    }
    aggregated_stderr.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(ProcessOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: aggregated_stderr,
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Spawna un proceso en background y devuelve un handle.
pub fn spawn_bg(cmd: &str) -> Result<u64, ProcessError> {
    let parts = parse_cmd(cmd)?;
    let child = Command::new(&parts[0])
        .args(&parts[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ProcessError::Spawn(e.to_string()))?;
    let handle = next_handle();
    registry().lock().unwrap().insert(handle_key(handle), child);
    Ok(handle)
}

/// Espera a que un proceso spawned termine.
pub fn spawn_wait(handle: u64) -> Result<ProcessOutput, ProcessError> {
    let child = registry().lock().unwrap().remove(&handle_key(handle))
        .ok_or(ProcessError::UnknownHandle(handle))?;
    let start = Instant::now();
    let output = child.wait_with_output().map_err(|e| ProcessError::Io(e.to_string()))?;
    Ok(ProcessOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Chequea si un proceso spawned ya terminó (sin bloquear).
pub fn spawn_poll(handle: u64) -> Result<Option<i32>, ProcessError> {
    let mut reg = registry().lock().unwrap();
    let child = reg.get_mut(&handle_key(handle)).ok_or(ProcessError::UnknownHandle(handle))?;
    match child.try_wait().map_err(|e| ProcessError::Io(e.to_string()))? {
        Some(status) => Ok(Some(status.code().unwrap_or(-1))),
        None => Ok(None),
    }
}

/// Mata un proceso spawned (SIGKILL). En Termux/Linux funciona idéntico.
pub fn spawn_kill(handle: u64) -> Result<(), ProcessError> {
    let mut reg = registry().lock().unwrap();
    let child = reg.get_mut(&handle_key(handle)).ok_or(ProcessError::UnknownHandle(handle))?;
    child.kill().map_err(|e| ProcessError::Io(e.to_string()))?;
    Ok(())
}

/// PID de un handle spawned.
pub fn spawn_pid(handle: u64) -> Result<u32, ProcessError> {
    let reg = registry().lock().unwrap();
    let child = reg.get(&handle_key(handle)).ok_or(ProcessError::UnknownHandle(handle))?;
    Ok(child.id())
}

// --- Environment ---

pub fn env_get(name: &str) -> Option<String> { std::env::var(name).ok() }
pub fn env_set(name: &str, value: &str)     { std::env::set_var(name, value); }
pub fn env_unset(name: &str)                { std::env::remove_var(name); }

pub fn env_vars() -> Vec<(String, String)> { std::env::vars().collect() }

pub fn working_dir() -> Result<String, ProcessError> {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| ProcessError::Io(e.to_string()))
}

pub fn set_working_dir(path: &str) -> Result<(), ProcessError> {
    std::env::set_current_dir(path).map_err(|e| ProcessError::Io(e.to_string()))
}

// --- Info del proceso actual y del sistema ---

pub fn self_pid() -> u32 { std::process::id() }

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

pub fn args() -> Vec<String> { std::env::args().collect() }

/// Termina el proceso actual con el código dado.
pub fn exit(code: i32) -> ! { std::process::exit(code) }

/// Manda una señal POSIX a un PID cualquiera del sistema.
/// Usa la infraestructura de nix; solo funciona en Unix.
/// Signal codes: 1=HUP, 2=INT, 9=KILL, 15=TERM, 18=CONT, 19=STOP.
#[cfg(unix)]
pub fn send_signal(pid: i32, signal_num: i32) -> Result<(), ProcessError> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let sig = Signal::try_from(signal_num)
        .map_err(|e| ProcessError::Signal(e.to_string()))?;
    kill(Pid::from_raw(pid), sig).map_err(|e| ProcessError::Signal(e.to_string()))
}

#[cfg(not(unix))]
pub fn send_signal(_pid: i32, _signal_num: i32) -> Result<(), ProcessError> {
    Err(ProcessError::Signal("signals only supported on Unix".into()))
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let children = {
        let mut reg = crate::native::lock_recover(registry());
        let keys: Vec<_> = reg.keys()
            .filter(|(owner, _)| *owner == runtime_id)
            .copied()
            .collect();
        keys.into_iter().filter_map(|key| reg.remove(&key)).collect::<Vec<_>>()
    };
    let released = children.len();
    for mut child in children {
        match child.try_wait() {
            Ok(Some(_)) => {}
            _ => { let _ = child.kill(); let _ = child.wait(); }
        }
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
    fn run_with_input_pipes_stdin() {
        let out = run_with_input("cat", b"hello").unwrap();
        assert_eq!(out.stdout, "hello");
    }

    #[test]
    fn parse_respects_quotes() {
        let parts = parse_cmd("echo 'hello world' foo").unwrap();
        assert_eq!(parts, vec!["echo", "hello world", "foo"]);
    }

    #[test]
    fn shell_supports_pipes() {
        let out = shell("echo hello | tr a-z A-Z").unwrap();
        assert!(out.stdout.contains("HELLO"));
    }

    #[test]
    fn env_get_set_roundtrip() {
        env_set("TITAN_TEST_VAR", "42");
        assert_eq!(env_get("TITAN_TEST_VAR"), Some("42".into()));
        env_unset("TITAN_TEST_VAR");
        assert_eq!(env_get("TITAN_TEST_VAR"), None);
    }
}
