//! Structured child-process execution without invoking a shell.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)] pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub clear_env: bool,
}
#[derive(Debug, Clone)] pub struct ProcessOutput { pub status: Option<i32>, pub success: bool, pub stdout: Vec<u8>, pub stderr: Vec<u8>, pub timed_out: bool }
impl ProcessOutput { pub fn stdout_text(&self) -> String { String::from_utf8_lossy(&self.stdout).into() } pub fn stderr_text(&self) -> String { String::from_utf8_lossy(&self.stderr).into() } }

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self { Self { program: program.into(), ..Self::default() } }
    pub fn arg(mut self, value: impl Into<String>) -> Self { self.args.push(value.into()); self }
    pub fn args(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self { self.args.extend(values.into_iter().map(Into::into)); self }
    pub fn current_dir(mut self, path: impl Into<PathBuf>) -> Self { self.cwd = Some(path.into()); self }
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self { self.env.insert(key.into(), value.into()); self }
    fn command(&self) -> Command { let mut command = Command::new(&self.program); command.args(&self.args); if let Some(cwd) = &self.cwd { command.current_dir(cwd); } if self.clear_env { command.env_clear(); } command.envs(&self.env); command }
    pub fn status(&self) -> io::Result<std::process::ExitStatus> { self.command().status() }
    pub fn output(&self) -> io::Result<ProcessOutput> { let output = self.command().output()?; Ok(ProcessOutput { status: output.status.code(), success: output.status.success(), stdout: output.stdout, stderr: output.stderr, timed_out: false }) }
    pub fn output_timeout(&self, timeout: Duration) -> io::Result<ProcessOutput> {
        use std::io::Read;
        let mut command = self.command(); command.stdout(Stdio::piped()).stderr(Stdio::piped()); let mut child = command.spawn()?;
        let mut stdout = child.stdout.take().ok_or_else(|| io::Error::other("stdout pipe unavailable"))?;
        let mut stderr = child.stderr.take().ok_or_else(|| io::Error::other("stderr pipe unavailable"))?;
        let stdout_reader = std::thread::spawn(move || { let mut data = Vec::new(); stdout.read_to_end(&mut data).map(|_| data) });
        let stderr_reader = std::thread::spawn(move || { let mut data = Vec::new(); stderr.read_to_end(&mut data).map(|_| data) });
        let started = Instant::now(); let (status, timed_out) = loop {
            if let Some(status) = child.try_wait()? { break (status, false); }
            if started.elapsed() >= timeout { let _ = child.kill(); break (child.wait()?, true); }
            std::thread::sleep(Duration::from_millis(5));
        };
        let stdout = stdout_reader.join().map_err(|_| io::Error::other("stdout reader panicked"))??;
        let stderr = stderr_reader.join().map_err(|_| io::Error::other("stderr reader panicked"))??;
        Ok(ProcessOutput { status: status.code(), success: status.success() && !timed_out, stdout, stderr, timed_out })
    }
}

#[cfg(test)] mod tests { use super::*; #[test] fn captures_output_without_shell() { let output = CommandSpec::new("printf").arg("Titan").output().unwrap(); assert!(output.success); assert_eq!(output.stdout_text(), "Titan"); } }
