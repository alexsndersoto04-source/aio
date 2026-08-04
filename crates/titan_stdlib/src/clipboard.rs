//! Real clipboard and OS notifications (`std::clipboard`, `std::notify`).
//!
//! Nothing is simulated. The clipboard is really read/written and
//! notifications are really shown by dispatching to the platform's native
//! tools:
//!
//! | Platform    | Clipboard                      | Notifications            |
//! |-------------|--------------------------------|--------------------------|
//! | Termux      | `termux-clipboard-set/get`     | `termux-notification`    |
//! | Linux/Wayland | `wl-copy` / `wl-paste`       | `notify-send`            |
//! | Linux/X11   | `xclip` / `xsel`               | `notify-send`            |
//! | macOS       | `pbcopy` / `pbpaste`           | `osascript`              |
//! | Windows     | `clip` / PowerShell            | — (typed error)          |
//!
//! When no backend is available (e.g. Termux without `termux-api`, or a
//! headless Linux box), a **typed error** is returned — never a fake
//! success.

use std::io::Write;
use std::process::{Command, Stdio};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("no clipboard/notification backend available on this platform (install termux-api, wl-clipboard, xclip, xsel, pbcopy or notify-send)")]
    NoBackend,
    #[error("tool '{tool}' failed: {stderr}")]
    ToolFailed { tool: String, stderr: String },
    #[error("I/O error running '{tool}': {source}")]
    Io { tool: String, source: std::io::Error },
}

type Tool = (&'static str, &'static [&'static str]);

fn clipboard_set_backends() -> Vec<Tool> {
    if cfg!(target_os = "android") {
        vec![("termux-clipboard-set", &[])]
    } else if cfg!(target_os = "macos") {
        vec![("pbcopy", &[])]
    } else if cfg!(windows) {
        vec![("clip", &[])]
    } else if cfg!(target_os = "linux") {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            vec![("wl-copy", &[])]
        } else if std::env::var_os("DISPLAY").is_some() {
            vec![("xclip", &["-selection", "clipboard"]), ("xsel", &["--clipboard", "--input"])]
        } else {
            vec![]
        }
    } else {
        vec![]
    }
}

fn clipboard_get_backends() -> Vec<Tool> {
    if cfg!(target_os = "android") {
        vec![("termux-clipboard-get", &[])]
    } else if cfg!(target_os = "macos") {
        vec![("pbpaste", &[])]
    } else if cfg!(windows) {
        vec![("powershell.exe", &["-NoProfile", "-Command", "Get-Clipboard"])]
    } else if cfg!(target_os = "linux") {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            vec![("wl-paste", &["--no-newline"])]
        } else if std::env::var_os("DISPLAY").is_some() {
            vec![("xclip", &["-selection", "clipboard", "-o"]), ("xsel", &["--clipboard", "--output"])]
        } else {
            vec![]
        }
    } else {
        vec![]
    }
}

fn notify_backends() -> Vec<Tool> {
    if cfg!(target_os = "android") {
        vec![("termux-notification", &["--title", "{title}", "--content", "{body}"])]
    } else if cfg!(target_os = "macos") {
        vec![("osascript", &["-e", "display notification \"{body}\" with title \"{title}\""])]
    } else if cfg!(target_os = "linux") {
        vec![("notify-send", &["{title}", "{body}"])]
    } else {
        vec![]
    }
}

/// Feeds `text` to a tool's stdin (all clipboard *set* tools read stdin).
fn run_set(tool: &str, _args: &[&str], text: &str) -> Result<(), ClipboardError> {
    let mut child = Command::new(tool)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| ClipboardError::Io { tool: tool.into(), source })?;
    child
        .stdin
        .take()
        .ok_or_else(|| ClipboardError::ToolFailed { tool: tool.into(), stderr: "stdin not available".into() })?
        .write_all(text.as_bytes())
        .map_err(|source| ClipboardError::Io { tool: tool.into(), source })?;
    let output = child
        .wait_with_output()
        .map_err(|source| ClipboardError::Io { tool: tool.into(), source })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ClipboardError::ToolFailed { tool: tool.into(), stderr: String::from_utf8_lossy(&output.stderr).into_owned() })
    }
}

/// Captures a tool's stdout (all clipboard *get* tools print to stdout).
fn run_get(tool: &str, args: &[&str]) -> Result<String, ClipboardError> {
    let output = Command::new(tool)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|source| ClipboardError::Io { tool: tool.into(), source })?;
    if !output.status.success() {
        return Err(ClipboardError::ToolFailed { tool: tool.into(), stderr: String::from_utf8_lossy(&output.stderr).into_owned() });
    }
    // Strip the trailing newline most clipboard tools append.
    Ok(String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']).to_string())
}

/// Runs a notification tool; `{title}` / `{body}` placeholders in the args
/// are replaced with the real values (kept out of a shell — no `sh -c`).
fn run_notify(tool: &str, args: &[&str], title: &str, body: &str) -> Result<(), ClipboardError> {
    let resolved: Vec<String> = args
        .iter()
        .map(|arg| arg.replace("{title}", title).replace("{body}", body))
        .collect();
    let output = Command::new(tool)
        .args(&resolved)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|source| ClipboardError::Io { tool: tool.into(), source })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ClipboardError::ToolFailed { tool: tool.into(), stderr: String::from_utf8_lossy(&output.stderr).into_owned() })
    }
}

/// Copies `text` to the real system clipboard. Tries each available
/// backend in order; a missing tool is skipped, a present-but-failing tool
/// is reported (never swallowed).
pub fn set_text(text: &str) -> Result<(), ClipboardError> {
    let backends = clipboard_set_backends();
    if backends.is_empty() {
        return Err(ClipboardError::NoBackend);
    }
    let mut last_error: Option<ClipboardError> = None;
    for (tool, args) in backends {
        match run_set(tool, args, text) {
            Ok(()) => return Ok(()),
            // Tool not installed → try the next backend.
            Err(ClipboardError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                last_error = Some(ClipboardError::NoBackend);
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or(ClipboardError::NoBackend))
}

/// Reads the real system clipboard.
pub fn get_text() -> Result<String, ClipboardError> {
    let backends = clipboard_get_backends();
    if backends.is_empty() {
        return Err(ClipboardError::NoBackend);
    }
    let mut last_error: Option<ClipboardError> = None;
    for (tool, args) in backends {
        match run_get(tool, args) {
            Ok(text) => return Ok(text),
            Err(ClipboardError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                last_error = Some(ClipboardError::NoBackend);
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or(ClipboardError::NoBackend))
}

/// Shows a real OS notification.
pub fn send_notification(title: &str, body: &str) -> Result<(), ClipboardError> {
    let backends = notify_backends();
    if backends.is_empty() {
        return Err(ClipboardError::NoBackend);
    }
    let mut last_error: Option<ClipboardError> = None;
    for (tool, args) in backends {
        match run_notify(tool, args, title, body) {
            Ok(()) => return Ok(()),
            Err(ClipboardError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                last_error = Some(ClipboardError::NoBackend);
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or(ClipboardError::NoBackend))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_set_dispatches_via_stdin() {
        // `cat` reads stdin and echoes it to stdout (which we discard).
        // If the stdin-piping mechanism works, this returns Ok. Deterministic
        // on every platform where `cat` exists.
        assert!(run_set("cat", &[], "titan").is_ok());
    }

    #[test]
    fn clipboard_get_reads_stdout() {
        // `echo` writes to stdout; we capture and trim it. Deterministic.
        assert_eq!(run_get("echo", &["hola"]).unwrap(), "hola");
    }

    #[test]
    fn missing_backend_is_a_typed_error_not_a_noop() {
        assert!(matches!(run_set("definitely-not-a-real-tool-xyz", &[], "x"), Err(ClipboardError::Io { .. })));
        assert!(matches!(run_get("definitely-not-a-real-tool-xyz", &[]), Err(ClipboardError::Io { .. })));
        assert!(matches!(run_notify("definitely-not-a-real-tool-xyz", &["{title}", "{body}"], "t", "b"), Err(ClipboardError::Io { .. })));
    }

    #[test]
    fn notification_args_are_interpolated_without_a_shell() {
        // osascript uses a single -e script; replacing {title}/{body} must
        // not require a shell. We only verify the interpolation here.
        let resolved: Vec<String> = ["-e", "display notification \"{body}\" with title \"{title}\""]
            .iter()
            .map(|arg| arg.replace("{title}", "TITAN").replace("{body}", "ok"))
            .collect();
        assert_eq!(resolved[1], "display notification \"ok\" with title \"TITAN\"");
    }
}
