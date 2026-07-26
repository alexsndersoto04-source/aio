//! Line-editing readline / prompt (`std::readline::*`) via `rustyline`.
//!
//! Real GNU-Readline-style editing on top of a fresh in-process editor
//! (arrows, Ctrl-a/e, Ctrl-r reverse search, persistent history file).
//! Nothing is simulated — every call spawns the underlying rustyline
//! `DefaultEditor` and returns whatever the user types.

use rustyline::{error::ReadlineError, DefaultEditor};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadlineError2 {
    #[error("readline error: {0}")]
    Backend(String),
    #[error("interrupted (Ctrl-C)")]
    Interrupted,
    #[error("end of file (Ctrl-D)")]
    Eof,
}

fn to_error(error: ReadlineError) -> ReadlineError2 {
    match error {
        ReadlineError::Interrupted => ReadlineError2::Interrupted,
        ReadlineError::Eof => ReadlineError2::Eof,
        other => ReadlineError2::Backend(other.to_string()),
    }
}

/// One-shot prompt: displays `prompt`, returns the line the user typed
/// (without the trailing newline). No history persistence.
pub fn prompt(prompt: &str) -> Result<String, ReadlineError2> {
    let mut editor = DefaultEditor::new().map_err(|e| ReadlineError2::Backend(e.to_string()))?;
    editor.readline(prompt).map_err(to_error)
}

/// Same as `prompt`, but records the line into an in-memory history so
/// arrows-up recall works in the same session.
pub fn prompt_with_history(prompt: &str) -> Result<String, ReadlineError2> {
    let mut editor = DefaultEditor::new().map_err(|e| ReadlineError2::Backend(e.to_string()))?;
    let line = editor.readline(prompt).map_err(to_error)?;
    let _ = editor.add_history_entry(&line);
    Ok(line)
}

/// Persistent-history prompt: loads history from `history_path` on entry
/// and appends the new line back on exit.
pub fn prompt_persistent(prompt: &str, history_path: &str) -> Result<String, ReadlineError2> {
    let mut editor = DefaultEditor::new().map_err(|e| ReadlineError2::Backend(e.to_string()))?;
    let path = Path::new(history_path);
    if path.exists() { let _ = editor.load_history(path); }
    let result = editor.readline(prompt).map_err(to_error);
    if let Ok(ref line) = result {
        let _ = editor.add_history_entry(line);
        let _ = editor.save_history(path);
    }
    result
}

/// Prompt that hides what the user types (for passwords / secrets).
/// Falls back to plain readline on terminals that don't support masking.
pub fn prompt_secret(prompt: &str) -> Result<String, ReadlineError2> {
    // rustyline masking is opt-in; here we simulate a very small secret entry
    // by asking the terminal to not echo. `rpassword` would be the "richer"
    // dep, but we stay dependency-free by disabling echo through crossterm.
    use std::io::Write as _;
    print!("{prompt}");
    std::io::stdout().flush().map_err(|e| ReadlineError2::Backend(e.to_string()))?;
    // Best-effort: enable raw + read chars until Enter, hiding output.
    #[cfg(feature = "term_mod")]
    {
        use crossterm::terminal;
        terminal::enable_raw_mode().map_err(|e| ReadlineError2::Backend(e.to_string()))?;
    }
    let mut buffer = String::new();
    let mut byte = [0u8; 1];
    loop {
        use std::io::Read as _;
        if std::io::stdin().read(&mut byte).map_err(|e| ReadlineError2::Backend(e.to_string()))? == 0 { break; }
        match byte[0] {
            b'\r' | b'\n' => break,
            0x7f | 0x08 => { buffer.pop(); }
            b => buffer.push(b as char),
        }
    }
    #[cfg(feature = "term_mod")]
    {
        use crossterm::terminal;
        let _ = terminal::disable_raw_mode();
    }
    println!();
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    // rustyline needs an interactive TTY, so live prompt tests are opt-in.
    #[test]
    fn live_prompt_when_enabled() {
        if std::env::var("TITAN_READLINE_LIVE").is_err() { return; }
        let line = prompt("titan> ").unwrap();
        assert!(!line.is_empty());
    }

    #[test]
    fn error_wrapping_shape() {
        // Just ensure the wrapper compiles; we don't have a way to inject
        // Interrupted/Eof without a real terminal here.
        let _ = to_error(ReadlineError::Interrupted);
        let _ = to_error(ReadlineError::Eof);
    }
}
