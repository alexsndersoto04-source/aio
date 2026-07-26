//! Termux / Android integration (`std::termux::*`).
//!
//! Real bindings to the `termux-*` command-line utilities shipped by the
//! Termux:API app. Nothing is simulated: every helper `Command`-spawns the
//! matching binary from `PATH` and returns whatever the OS/Android hardware
//! actually reports.
//!
//! **Runtime requirement.** The user must have both installed on-device:
//!   * The **Termux:API** app (Play Store or F-Droid).
//!   * The `termux-api` package (`pkg install termux-api`).
//!
//! If `termux-*` isn't in `PATH`, every function returns
//! [`TermuxError::MissingCli`] with a clear message, so `.titan` programs
//! stay diagnosable on non-Termux platforms.

use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TermuxError {
    #[error("termux-api CLI '{tool}' is not installed. Run: pkg install termux-api")]
    MissingCli { tool: String },
    #[error("termux-api '{tool}' failed with status {status}: {stderr}")]
    Failed { tool: String, status: i32, stderr: String },
    #[error("could not parse termux-api output as JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error running termux-api: {0}")]
    Io(#[from] std::io::Error),
}

fn spawn(tool: &str, args: &[&str]) -> Result<Vec<u8>, TermuxError> {
    let output = Command::new(tool)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => TermuxError::MissingCli { tool: tool.into() },
            _ => TermuxError::Io(error),
        })?;
    if !output.status.success() {
        return Err(TermuxError::Failed {
            tool: tool.into(),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(output.stdout)
}

fn spawn_text(tool: &str, args: &[&str]) -> Result<String, TermuxError> {
    let bytes = spawn(tool, args)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn spawn_json(tool: &str, args: &[&str]) -> Result<Value, TermuxError> {
    let bytes = spawn(tool, args)?;
    Ok(serde_json::from_slice(&bytes)?)
}

// ---------------- Battery / device state ------------------------------

/// Returns the raw battery status map:
/// `{ health, percentage, plugged, status, temperature, current, ... }`.
pub fn battery_status() -> Result<Value, TermuxError> {
    spawn_json("termux-battery-status", &[])
}

/// Get the current WiFi connection info as JSON.
pub fn wifi_info() -> Result<Value, TermuxError> {
    spawn_json("termux-wifi-connectioninfo", &[])
}

/// Get info about the phone / SIM.
pub fn telephony_info() -> Result<Value, TermuxError> {
    spawn_json("termux-telephony-deviceinfo", &[])
}

// ---------------- Location (GPS / network) ----------------------------

/// Returns the device location. `provider` is "gps", "network" or "passive".
/// `request` is "once", "last" or "updates" (the last is streaming; here we
/// only capture the first sample).
pub fn location(provider: &str, request: &str) -> Result<Value, TermuxError> {
    spawn_json("termux-location", &["-p", provider, "-r", request])
}

// ---------------- Sensors ---------------------------------------------

/// Lists the sensor names available on this device.
pub fn sensor_list() -> Result<Vec<String>, TermuxError> {
    let value = spawn_json("termux-sensor", &["-l"])?;
    Ok(value.get("sensors")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default())
}

/// Reads a single sample from a named sensor (e.g. "accelerometer").
pub fn sensor_read(sensor: &str) -> Result<Value, TermuxError> {
    spawn_json("termux-sensor", &["-s", sensor, "-n", "1"])
}

// ---------------- Clipboard (REAL — writes to the system clipboard) --

pub fn clipboard_get() -> Result<String, TermuxError> {
    Ok(spawn_text("termux-clipboard-get", &[])?.trim_end_matches('\n').to_string())
}

pub fn clipboard_set(text: &str) -> Result<(), TermuxError> {
    let mut child = Command::new("termux-clipboard-set")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => TermuxError::MissingCli { tool: "termux-clipboard-set".into() },
            _ => TermuxError::Io(e),
        })?;
    use std::io::Write as _;
    if let Some(mut stdin) = child.stdin.take() { stdin.write_all(text.as_bytes())?; }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(TermuxError::Failed {
            tool: "termux-clipboard-set".into(),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

// ---------------- Feedback (vibrate, torch, toast, notification) -----

/// Vibrate for `duration`. Passes `-f` to force even in silent mode.
pub fn vibrate(duration: Duration, force: bool) -> Result<(), TermuxError> {
    let ms = duration.as_millis().to_string();
    let mut args = vec!["-d", ms.as_str()];
    if force { args.push("-f"); }
    spawn("termux-vibrate", &args).map(|_| ())
}

pub fn torch(on: bool) -> Result<(), TermuxError> {
    spawn("termux-torch", &[if on { "on" } else { "off" }]).map(|_| ())
}

pub fn toast(message: &str) -> Result<(), TermuxError> {
    spawn("termux-toast", &[message]).map(|_| ())
}

/// Show a system notification. `id` is optional; pass 0 to auto-generate.
pub fn notify(title: &str, content: &str, id: i64) -> Result<(), TermuxError> {
    let id_string = id.to_string();
    let mut args = vec!["-t", title, "-c", content];
    if id > 0 { args.extend(["--id", id_string.as_str()]); }
    spawn("termux-notification", &args).map(|_| ())
}

pub fn notify_remove(id: i64) -> Result<(), TermuxError> {
    let id_string = id.to_string();
    spawn("termux-notification-remove", &[id_string.as_str()]).map(|_| ())
}

// ---------------- Text-to-speech --------------------------------------

/// Speak `text` through the Android TTS engine.
pub fn tts_speak(text: &str) -> Result<(), TermuxError> {
    let mut child = Command::new("termux-tts-speak")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => TermuxError::MissingCli { tool: "termux-tts-speak".into() },
            _ => TermuxError::Io(e),
        })?;
    use std::io::Write as _;
    if let Some(mut stdin) = child.stdin.take() { stdin.write_all(text.as_bytes())?; }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(TermuxError::Failed {
            tool: "termux-tts-speak".into(),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

// ---------------- SMS -------------------------------------------------

/// List the SMS inbox. `limit` bounds the number of messages returned.
pub fn sms_list(limit: i64) -> Result<Value, TermuxError> {
    let limit_string = limit.to_string();
    spawn_json("termux-sms-list", &["-l", limit_string.as_str()])
}

/// Send an SMS. Multiple recipients can be passed comma-separated.
pub fn sms_send(recipient: &str, message: &str) -> Result<(), TermuxError> {
    let mut child = Command::new("termux-sms-send")
        .args(["-n", recipient])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => TermuxError::MissingCli { tool: "termux-sms-send".into() },
            _ => TermuxError::Io(e),
        })?;
    use std::io::Write as _;
    if let Some(mut stdin) = child.stdin.take() { stdin.write_all(message.as_bytes())?; }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(TermuxError::Failed {
            tool: "termux-sms-send".into(),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

// ---------------- Contacts --------------------------------------------

pub fn contacts() -> Result<Value, TermuxError> {
    spawn_json("termux-contact-list", &[])
}

// ---------------- Camera ----------------------------------------------

pub fn camera_info() -> Result<Value, TermuxError> {
    spawn_json("termux-camera-info", &[])
}

/// Take a photo from `camera_id` ("0" = back, "1" = front) into `output_path`.
pub fn camera_photo(camera_id: &str, output_path: &str) -> Result<(), TermuxError> {
    spawn("termux-camera-photo", &["-c", camera_id, output_path]).map(|_| ())
}

// ---------------- Screen brightness -----------------------------------

/// Set brightness (0..=255). Requires WRITE_SETTINGS permission on-device.
pub fn brightness(value: i64) -> Result<(), TermuxError> {
    let value_string = value.clamp(0, 255).to_string();
    spawn("termux-brightness", &[value_string.as_str()]).map(|_| ())
}

// ---------------- Dialog / share --------------------------------------

/// Show a simple confirm/text dialog. `dialog_type` matches termux-dialog's
/// `-t` flag: `confirm`, `text`, `checkbox`, `date`, `time`, `radio`, ...
pub fn dialog(dialog_type: &str, title: &str) -> Result<Value, TermuxError> {
    spawn_json("termux-dialog", &[dialog_type, "-t", title])
}

/// Share a file through the system share sheet.
pub fn share(path: &str) -> Result<(), TermuxError> {
    spawn("termux-share", &[path]).map(|_| ())
}

/// Returns `true` if `termux-battery-status` is on PATH — used from .titan to
/// gracefully degrade on non-Termux hosts.
pub fn is_available() -> bool {
    which::which("termux-battery-status").is_ok()
        || Command::new("termux-battery-status")
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().is_ok()
}

// A tiny fallback resolver so we don't add another dep just for `which`.
mod which {
    use std::path::PathBuf;
    pub fn which(name: &str) -> Result<PathBuf, ()> {
        let path = std::env::var_os("PATH").ok_or(())?;
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() { return Ok(candidate); }
        }
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_cli_returns_typed_error() {
        // Point to a name that cannot exist on CI runners.
        let out = spawn("termux-definitely-does-not-exist-xyz", &[]);
        assert!(matches!(out, Err(TermuxError::MissingCli { .. })));
    }

    /// Live tests only run on a real Termux device. Opt-in with
    /// TITAN_TERMUX_LIVE=1 so CI/dev machines skip them cleanly.
    #[test]
    fn live_battery_when_enabled() {
        if std::env::var("TITAN_TERMUX_LIVE").is_err() { return; }
        let status = battery_status().expect("battery");
        // percentage is always present on Android.
        assert!(status.get("percentage").is_some());
    }

    #[test]
    fn live_toast_when_enabled() {
        if std::env::var("TITAN_TERMUX_LIVE").is_err() { return; }
        toast("titan smoke test").unwrap();
    }
}
