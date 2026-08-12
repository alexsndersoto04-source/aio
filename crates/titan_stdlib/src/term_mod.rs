//! Terminal control (`std::term::*`) powered by `crossterm`.
//!
//! Everything is real, pure Rust, and works on Termux. Nothing simulated.
//!
//! Exposes colour output, cursor positioning, screen clearing, terminal size
//! queries, keyboard events, and the raw-mode / alternate-screen switches
//! needed to build TUI apps (btop-style dashboards, spinners, forms, etc.).

use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{
        Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TermError {
    #[error("terminal I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("unknown color: '{0}'")]
    UnknownColor(String),
    #[error("unknown attribute: '{0}' (try bold, dim, italic, underline, reverse)")]
    UnknownAttribute(String),
}

fn parse_color(name: &str) -> Result<Color, TermError> {
    match name.to_ascii_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" | "purple" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "white" => Ok(Color::White),
        "grey" | "gray" => Ok(Color::Grey),
        "darkgrey" | "darkgray" => Ok(Color::DarkGrey),
        "reset" | "default" => Ok(Color::Reset),
        other => {
            // Accept "rgb:R,G,B" and "#RRGGBB" for custom colors.
            if let Some(rest) = other.strip_prefix("rgb:") {
                let parts: Vec<_> = rest.split(',').collect();
                if parts.len() == 3 {
                    if let (Ok(r), Ok(g), Ok(b)) =
                        (parts[0].parse(), parts[1].parse(), parts[2].parse())
                    {
                        return Ok(Color::Rgb { r, g, b });
                    }
                }
            }
            if let Some(hex) = other.strip_prefix('#') {
                if hex.len() == 6 {
                    if let (Ok(r), Ok(g), Ok(b)) = (
                        u8::from_str_radix(&hex[0..2], 16),
                        u8::from_str_radix(&hex[2..4], 16),
                        u8::from_str_radix(&hex[4..6], 16),
                    ) {
                        return Ok(Color::Rgb { r, g, b });
                    }
                }
            }
            Err(TermError::UnknownColor(name.into()))
        }
    }
}

fn parse_attr(name: &str) -> Result<Attribute, TermError> {
    match name.to_ascii_lowercase().as_str() {
        "bold" => Ok(Attribute::Bold),
        "dim" => Ok(Attribute::Dim),
        "italic" => Ok(Attribute::Italic),
        "underline" | "under" => Ok(Attribute::Underlined),
        "reverse" | "invert" => Ok(Attribute::Reverse),
        "hidden" => Ok(Attribute::Hidden),
        "reset" => Ok(Attribute::Reset),
        other => Err(TermError::UnknownAttribute(other.into())),
    }
}

// ---------------- Styled printing --------------------------------------

/// Print `text` with a foreground colour name (returns to default afterwards).
pub fn print_colored(color: &str, text: &str) -> Result<(), TermError> {
    let color = parse_color(color)?;
    let mut out = io::stdout();
    execute!(out, SetForegroundColor(color), Print(text), ResetColor)?;
    Ok(())
}

/// Print `text` with foreground + background colour names.
pub fn print_styled(fg: &str, bg: &str, text: &str) -> Result<(), TermError> {
    let fg = parse_color(fg)?;
    let bg = parse_color(bg)?;
    let mut out = io::stdout();
    execute!(
        out,
        SetForegroundColor(fg),
        SetBackgroundColor(bg),
        Print(text),
        ResetColor,
    )?;
    Ok(())
}

/// Apply a text attribute (bold, italic, underline, …) around `text`.
pub fn print_attr(attr: &str, text: &str) -> Result<(), TermError> {
    let attr = parse_attr(attr)?;
    let mut out = io::stdout();
    execute!(
        out,
        SetAttribute(attr),
        Print(text),
        SetAttribute(Attribute::Reset)
    )?;
    Ok(())
}

// ---------------- Screen / cursor --------------------------------------

pub fn clear_screen() -> Result<(), TermError> {
    execute!(io::stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    Ok(())
}

pub fn clear_line() -> Result<(), TermError> {
    execute!(io::stdout(), Clear(ClearType::CurrentLine))?;
    Ok(())
}

pub fn move_to(column: u16, row: u16) -> Result<(), TermError> {
    execute!(io::stdout(), cursor::MoveTo(column, row))?;
    Ok(())
}

pub fn hide_cursor() -> Result<(), TermError> {
    execute!(io::stdout(), cursor::Hide)?;
    Ok(())
}
pub fn show_cursor() -> Result<(), TermError> {
    execute!(io::stdout(), cursor::Show)?;
    Ok(())
}

/// Returns `(columns, rows)` for the current terminal.
pub fn size() -> Result<(u16, u16), TermError> {
    Ok(terminal::size()?)
}

pub fn flush() -> Result<(), TermError> {
    io::stdout().flush()?;
    Ok(())
}

// ---------------- Alt-screen / raw mode (for TUI apps) -----------------

pub fn enter_alt_screen() -> Result<(), TermError> {
    execute!(io::stdout(), EnterAlternateScreen)?;
    Ok(())
}
pub fn leave_alt_screen() -> Result<(), TermError> {
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}
pub fn enable_raw() -> Result<(), TermError> {
    terminal::enable_raw_mode()?;
    Ok(())
}
pub fn disable_raw() -> Result<(), TermError> {
    terminal::disable_raw_mode()?;
    Ok(())
}

// ---------------- Keyboard events --------------------------------------

/// Wait up to `timeout_ms` for a key. Returns a stringified key or empty on timeout.
/// Format examples: "a", "Enter", "Escape", "Up", "Ctrl+c", "Shift+F1".
pub fn read_key(timeout_ms: u64) -> Result<String, TermError> {
    if !event::poll(Duration::from_millis(timeout_ms))? {
        return Ok(String::new());
    }
    match event::read()? {
        Event::Key(KeyEvent {
            code, modifiers, ..
        }) => Ok(format_key(code, modifiers)),
        _ => Ok(String::new()),
    }
}

fn format_key(code: KeyCode, modifiers: KeyModifiers) -> String {
    let name = match code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Esc => "Escape".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::BackTab => "Shift+Tab".into(),
        KeyCode::Backspace => "Backspace".into(),
        KeyCode::Left => "Left".into(),
        KeyCode::Right => "Right".into(),
        KeyCode::Up => "Up".into(),
        KeyCode::Down => "Down".into(),
        KeyCode::Home => "Home".into(),
        KeyCode::End => "End".into(),
        KeyCode::PageUp => "PageUp".into(),
        KeyCode::PageDown => "PageDown".into(),
        KeyCode::Insert => "Insert".into(),
        KeyCode::Delete => "Delete".into(),
        KeyCode::F(n) => format!("F{n}"),
        other => format!("{other:?}"),
    };
    let mut parts = Vec::new();
    if modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl");
    }
    if modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt");
    }
    if modifiers.contains(KeyModifiers::SHIFT) && !matches!(code, KeyCode::Char(_)) {
        parts.push("Shift");
    }
    if parts.is_empty() {
        name
    } else {
        format!("{}+{}", parts.join("+"), name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_colors() {
        assert_eq!(parse_color("red").unwrap(), Color::Red);
        assert_eq!(parse_color("Blue").unwrap(), Color::Blue);
        assert_eq!(parse_color("gray").unwrap(), Color::Grey);
        assert_eq!(
            parse_color("rgb:10,20,30").unwrap(),
            Color::Rgb {
                r: 10,
                g: 20,
                b: 30
            }
        );
        assert_eq!(
            parse_color("#ff8800").unwrap(),
            Color::Rgb {
                r: 0xff,
                g: 0x88,
                b: 0
            }
        );
        assert!(parse_color("banana").is_err());
    }

    #[test]
    fn parses_attributes() {
        assert!(matches!(parse_attr("bold").unwrap(), Attribute::Bold));
        assert!(matches!(
            parse_attr("Underline").unwrap(),
            Attribute::Underlined
        ));
        assert!(parse_attr("blink123").is_err());
    }

    #[test]
    fn format_key_examples() {
        assert_eq!(format_key(KeyCode::Char('a'), KeyModifiers::NONE), "a");
        assert_eq!(
            format_key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            "Ctrl+c"
        );
        assert_eq!(format_key(KeyCode::Enter, KeyModifiers::NONE), "Enter");
        assert_eq!(format_key(KeyCode::F(1), KeyModifiers::SHIFT), "Shift+F1");
    }
}
