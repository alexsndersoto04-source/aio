//! Structured child-process execution without invoking a shell.

use std::collections::{BTreeMap, HashMap};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub(crate) const MAX_RUNTIME_PROCESSES: usize = 32;
const MAX_COMMAND_BYTES: usize = 64 * 1024;
const MAX_CAPTURE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct ProcessPermit {
    runtime_id: u64,
    count: usize,
}

static PROCESS_COUNTS: OnceLock<Mutex<HashMap<u64, usize>>> = OnceLock::new();

fn process_counts() -> &'static Mutex<HashMap<u64, usize>> {
    PROCESS_COUNTS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn reserve_processes(count: usize) -> io::Result<ProcessPermit> {
    let runtime_id = crate::native::current_runtime_id();
    let mut counts = crate::native::lock_recover(process_counts());
    let active = counts.get(&runtime_id).copied().unwrap_or(0);
    let requested = active.checked_add(count).ok_or_else(process_limit_error)?;
    if requested > MAX_RUNTIME_PROCESSES {
        return Err(process_limit_error());
    }
    counts.insert(runtime_id, requested);
    Ok(ProcessPermit { runtime_id, count })
}

fn process_limit_error() -> io::Error {
    io::Error::other(format!(
        "process count exceeds limit {MAX_RUNTIME_PROCESSES}"
    ))
}

#[cfg(any(test, feature = "process_mod"))]
pub(crate) fn active_processes(runtime_id: u64) -> usize {
    crate::native::lock_recover(process_counts())
        .get(&runtime_id)
        .copied()
        .unwrap_or(0)
}

impl Drop for ProcessPermit {
    fn drop(&mut self) {
        let mut counts = crate::native::lock_recover(process_counts());
        let remove = if let Some(active) = counts.get_mut(&self.runtime_id) {
            *active = active.saturating_sub(self.count);
            *active == 0
        } else {
            false
        };
        if remove {
            counts.remove(&self.runtime_id);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub clear_env: bool,
}

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub status: Option<i32>,
    pub success: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
}

impl ProcessOutput {
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into()
    }
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into()
    }
}

struct CaptureBudget {
    used: AtomicUsize,
    exceeded: AtomicBool,
}

impl CaptureBudget {
    fn new() -> Self {
        Self {
            used: AtomicUsize::new(0),
            exceeded: AtomicBool::new(false),
        }
    }

    fn append(&self, output: &mut Vec<u8>, chunk: &[u8]) {
        let mut used = self.used.load(Ordering::Acquire);
        loop {
            let available = MAX_CAPTURE_BYTES.saturating_sub(used);
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

fn read_bounded<R: Read>(mut reader: R, budget: Arc<CaptureBudget>) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            return Ok(output);
        }
        // Discard overflow while continuing to drain the pipe so the child
        // cannot block after reaching the memory cap.
        budget.append(&mut output, &chunk[..count]);
    }
}

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            ..Self::default()
        }
    }
    pub fn arg(mut self, value: impl Into<String>) -> Self {
        self.args.push(value.into());
        self
    }
    pub fn args<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(values.into_iter().map(Into::into));
        self
    }
    pub fn current_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
    fn command(&self) -> io::Result<Command> {
        let command_bytes = self
            .args
            .iter()
            .map(String::len)
            .chain(
                self.env
                    .iter()
                    .map(|(key, value)| key.len().saturating_add(value.len())),
            )
            .chain(
                self.cwd
                    .iter()
                    .map(|path| path.to_string_lossy().len()),
            )
            .try_fold(self.program.len(), |total, bytes| {
                total.checked_add(bytes).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "command size overflow")
                })
            })?;
        if command_bytes > MAX_COMMAND_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("command bytes exceed limit {MAX_COMMAND_BYTES}"),
            ));
        }
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        if self.clear_env {
            command.env_clear();
        }
        command.envs(&self.env);
        Ok(command)
    }
    pub fn status(&self) -> io::Result<std::process::ExitStatus> {
        let _permit = reserve_processes(1)?;
        self.command()?.status()
    }
    pub fn output(&self) -> io::Result<ProcessOutput> {
        self.output_inner(None)
    }
    pub fn output_timeout(&self, timeout: Duration) -> io::Result<ProcessOutput> {
        self.output_inner(Some(timeout))
    }

    fn output_inner(&self, timeout: Option<Duration>) -> io::Result<ProcessOutput> {
        let _permit = reserve_processes(1)?;
        let mut command = self.command()?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("stdout pipe unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("stderr pipe unavailable"))?;
        let budget = Arc::new(CaptureBudget::new());
        let stdout_budget = Arc::clone(&budget);
        let stdout_reader = std::thread::spawn(move || read_bounded(stdout, stdout_budget));
        let stderr_budget = Arc::clone(&budget);
        let stderr_reader = std::thread::spawn(move || read_bounded(stderr, stderr_budget));
        let started = Instant::now();
        let (status, timed_out) = if let Some(timeout) = timeout {
            loop {
                if let Some(status) = child.try_wait()? {
                    break (status, false);
                }
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    break (child.wait()?, true);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        } else {
            (child.wait()?, false)
        };
        let stdout = stdout_reader
            .join()
            .map_err(|_| io::Error::other("stdout reader panicked"))??;
        let stderr = stderr_reader
            .join()
            .map_err(|_| io::Error::other("stderr reader panicked"))??;
        if budget.exceeded.load(Ordering::Acquire) {
            return Err(io::Error::other(format!(
                "captured process output bytes exceed limit {MAX_CAPTURE_BYTES}"
            )));
        }
        Ok(ProcessOutput {
            status: status.code(),
            success: status.success() && !timed_out,
            stdout,
            stderr,
            timed_out,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_output_without_shell() {
        let output = CommandSpec::new("printf").arg("Titan").output().unwrap();
        assert!(output.success);
        assert_eq!(output.stdout_text(), "Titan");
    }

    #[test]
    fn bounded_reader_discards_bytes_after_shared_limit() {
        let budget = Arc::new(CaptureBudget::new());
        budget.used.store(MAX_CAPTURE_BYTES - 2, Ordering::Release);
        let output = read_bounded(std::io::Cursor::new(b"four"), Arc::clone(&budget)).unwrap();
        assert_eq!(output, b"fo");
        assert!(budget.exceeded.load(Ordering::Acquire));
    }
}
