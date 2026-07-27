//! WiFi introspection (`std::wifi::*`) via the Termux:API CLI.
//!
//! Real bindings to `termux-wifi-scaninfo`, `termux-wifi-connectioninfo`
//! and `termux-wifi-enable` from the official Termux:API package. Nothing
//! is simulated: every function `Command::spawn`s the matching binary and
//! surfaces whatever Android's WifiManager reports.
//!
//! Complements Fase 5 (`std::termux::*`) with 3 dedicated Wi-Fi helpers.
//! Kept in its own module because Wi-Fi has different failure modes
//! (needs LOCATION permission, disabled by default on Android ≥ 10,
//! empty scans when the screen is locked, etc.) and its own JSON schema.
//!
//! **Runtime requirement.** Same as `std::termux::*`:
//!   * The **Termux:API** app installed (Play Store / F-Droid).
//!   * The `termux-api` package (`pkg install termux-api`).
//!
//! If the `termux-*` CLI isn't in `PATH`, every function returns
//! [`WifiError::MissingCli`] with a clear message.

use std::process::{Command, Stdio};

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WifiError {
    #[error("termux-api CLI '{tool}' is not installed. Run: pkg install termux-api")]
    MissingCli { tool: String },
    #[error("termux-api '{tool}' failed with status {status}: {stderr}")]
    Failed { tool: String, status: i32, stderr: String },
    #[error("could not parse termux-api output as JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error running termux-api: {0}")]
    Io(#[from] std::io::Error),
}

fn spawn(tool: &str, args: &[&str]) -> Result<Vec<u8>, WifiError> {
    let output = Command::new(tool)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => WifiError::MissingCli { tool: tool.into() },
            _ => WifiError::Io(error),
        })?;
    if !output.status.success() {
        return Err(WifiError::Failed {
            tool: tool.into(),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(output.stdout)
}

fn spawn_json(tool: &str, args: &[&str]) -> Result<Value, WifiError> {
    let bytes = spawn(tool, args)?;
    // termux-wifi-scaninfo returns `[]` on empty scan (perfectly valid).
    let text = String::from_utf8_lossy(&bytes);
    let trimmed = text.trim();
    if trimmed.is_empty() { return Ok(Value::Null); }
    Ok(serde_json::from_str(trimmed)?)
}

// ---- Public API -----------------------------------------------------

/// A single access point returned by `scan()`.
///
/// All fields come directly from Android's `WifiManager.getScanResults()`.
/// Note that from Android ≥ 9 the WifiManager throttles scans; from
/// Android ≥ 10 you also need Location permission granted to the
/// Termux:API app or you'll get an empty list.
#[derive(Debug, Clone)]
pub struct AccessPoint {
    pub ssid:                  String,
    pub bssid:                 String,
    pub rssi:                  i64,
    pub frequency_mhz:         i64,
    pub timestamp:             i64,
    pub channel_bandwidth_mhz: String,
    pub center_frequency_mhz:  i64,
}

/// Connection info returned by `connection_info()` — the currently
/// associated network. Every field can be missing depending on
/// Android version and permissions.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub ssid:              String,
    pub bssid:             String,
    pub ip:                String,
    pub mac_address:       String,
    pub link_speed_mbps:   i64,
    pub rssi:              i64,
    pub frequency_mhz:     i64,
    pub network_id:        i64,
    pub supplicant_state:  String,
    pub hidden_ssid:       bool,
}

/// Scan nearby Wi-Fi networks. Returns whatever Android's WifiManager
/// last cached — Android throttles rescans, so consecutive calls may
/// return the same list. If Location is disabled or missing permission,
/// the returned list is typically empty (Android quirk, not a bug).
pub fn scan() -> Result<Vec<AccessPoint>, WifiError> {
    let value = spawn_json("termux-wifi-scaninfo", &[])?;
    let mut out = Vec::new();
    if let Some(array) = value.as_array() {
        for item in array { out.push(access_point_from(item)); }
    }
    Ok(out)
}

/// Info about the currently associated Wi-Fi network. Returns `None`
/// when the device is not connected.
pub fn connection_info() -> Result<Option<ConnectionInfo>, WifiError> {
    let value = spawn_json("termux-wifi-connectioninfo", &[])?;
    // When disconnected the CLI still returns an object with `ssid == null`
    // (or the literal `<unknown ssid>`), never a top-level null. We treat
    // any output whose ssid field is missing / null / "<unknown ssid>"
    // as "not connected".
    let ssid = value.get("ssid").and_then(Value::as_str).unwrap_or("").to_string();
    if ssid.is_empty() || ssid == "<unknown ssid>" || ssid == "0x" {
        return Ok(None);
    }
    Ok(Some(ConnectionInfo {
        ssid,
        bssid:             value.get("bssid").and_then(Value::as_str).unwrap_or("").into(),
        ip:                value.get("ip").and_then(Value::as_str).unwrap_or("").into(),
        mac_address:       value.get("mac_address").and_then(Value::as_str).unwrap_or("").into(),
        link_speed_mbps:   value.get("link_speed_mbps").and_then(Value::as_i64).unwrap_or(0),
        rssi:              value.get("rssi").and_then(Value::as_i64).unwrap_or(0),
        frequency_mhz:     value.get("frequency_mhz").and_then(Value::as_i64).unwrap_or(0),
        network_id:        value.get("network_id").and_then(Value::as_i64).unwrap_or(-1),
        supplicant_state:  value.get("supplicant_state").and_then(Value::as_str).unwrap_or("").into(),
        hidden_ssid:       value.get("ssid_hidden").and_then(Value::as_bool).unwrap_or(false),
    }))
}

/// Toggle the Wi-Fi radio on/off. Requires WRITE_SETTINGS granted to
/// Termux:API. On Android ≥ 10 this may silently fail if the screen
/// is locked (upstream Android restriction).
pub fn set_enabled(enabled: bool) -> Result<(), WifiError> {
    let arg = if enabled { "true" } else { "false" };
    let _ = spawn("termux-wifi-enable", &[arg])?;
    Ok(())
}

// ---- Helpers --------------------------------------------------------

fn access_point_from(item: &Value) -> AccessPoint {
    AccessPoint {
        ssid:                  item.get("ssid").and_then(Value::as_str).unwrap_or("").into(),
        bssid:                 item.get("bssid").and_then(Value::as_str).unwrap_or("").into(),
        rssi:                  item.get("rssi").and_then(Value::as_i64).unwrap_or(0),
        frequency_mhz:         item.get("frequency_mhz").and_then(Value::as_i64).unwrap_or(0),
        timestamp:             item.get("timestamp").and_then(Value::as_i64).unwrap_or(0),
        channel_bandwidth_mhz: item.get("channel_bandwidth_mhz").and_then(Value::as_str).unwrap_or("").into(),
        center_frequency_mhz:  item.get("center_frequency_mhz").and_then(Value::as_i64).unwrap_or(0),
    }
}

/// Approximate the human-friendly "bars" (0-4) from an RSSI in dBm.
/// Uses Android's `WifiManager.calculateSignalLevel` heuristic. Pure Rust,
/// runs without touching the CLI so it can be used to render UI from
/// a cached AccessPoint list.
pub fn signal_bars(rssi_dbm: i64) -> u8 {
    if rssi_dbm >= -50 { 4 }
    else if rssi_dbm >= -60 { 3 }
    else if rssi_dbm >= -70 { 2 }
    else if rssi_dbm >= -80 { 1 }
    else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_bars_matches_typical_thresholds() {
        assert_eq!(signal_bars(-30),  4);
        assert_eq!(signal_bars(-55),  3);
        assert_eq!(signal_bars(-65),  2);
        assert_eq!(signal_bars(-75),  1);
        assert_eq!(signal_bars(-90),  0);
    }

    /// Live tests only run under Termux with termux-api installed.
    /// We can't rely on those in CI so the tests below only assert
    /// that missing binaries produce the typed error, not a panic.
    #[test]
    fn missing_cli_reports_typed_error() {
        let missing = Command::new("termux-wifi-scaninfo").arg("--help").output();
        if missing.is_err() {
            // We're on a non-Termux platform — the module should surface
            // MissingCli, not io::Error, when we try to call it.
            assert!(matches!(scan(), Err(WifiError::MissingCli { .. })));
        }
    }
}
