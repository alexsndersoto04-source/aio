//! Unix signals (`std::signals::*`) via the `signal-hook` crate.
//!
//! Provides two levels of API:
//!
//! * `install(signal_name)` — starts counting occurrences of a signal.
//!   Subsequent `pending(signal_name)` calls return how many hits landed
//!   since the last poll. Ideal for a graceful-shutdown loop:
//!
//!         std::signals::install("SIGINT")
//!         loop {
//!             if std::signals::pending("SIGINT") > 0 { break }
//!             // ... work ...
//!         }
//!
//! * `wait_any(timeout_ms)` — blocks up to `timeout_ms` for ANY installed
//!   signal, returning its name (or "timeout").

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignalError {
    #[error("unknown signal '{0}' (try SIGINT, SIGTERM, SIGHUP, SIGUSR1, SIGUSR2, SIGQUIT)")]
    Unknown(String),
    #[error("signal-hook error: {0}")]
    Hook(String),
}

fn resolve(name: &str) -> Result<i32, SignalError> {
    match name.to_ascii_uppercase().as_str() {
        "SIGINT"  | "INT"  => Ok(SIGINT),
        "SIGTERM" | "TERM" => Ok(SIGTERM),
        "SIGHUP"  | "HUP"  => Ok(SIGHUP),
        "SIGUSR1" | "USR1" => Ok(SIGUSR1),
        "SIGUSR2" | "USR2" => Ok(SIGUSR2),
        "SIGQUIT" | "QUIT" => Ok(SIGQUIT),
        "SIGPIPE" | "PIPE" => Ok(SIGPIPE),
        "SIGCHLD" | "CHLD" => Ok(SIGCHLD),
        other => Err(SignalError::Unknown(other.into())),
    }
}

fn name_from(sig: i32) -> &'static str {
    match sig {
        SIGINT  => "SIGINT",
        SIGTERM => "SIGTERM",
        SIGHUP  => "SIGHUP",
        SIGUSR1 => "SIGUSR1",
        SIGUSR2 => "SIGUSR2",
        SIGQUIT => "SIGQUIT",
        SIGPIPE => "SIGPIPE",
        SIGCHLD => "SIGCHLD",
        _       => "OTHER",
    }
}

/// A per-signal counter incremented by a background thread.
struct Counter { count: Arc<AtomicUsize> }

fn counters() -> &'static Mutex<HashMap<i32, Counter>> {
    static COUNTERS: OnceLock<Mutex<HashMap<i32, Counter>>> = OnceLock::new();
    COUNTERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a signal handler if not already installed. Idempotent.
pub fn install(signal_name: &str) -> Result<(), SignalError> {
    let sig = resolve(signal_name)?;
    let mut map = counters().lock().expect("signal counters poisoned");
    if map.contains_key(&sig) { return Ok(()); }
    let count = Arc::new(AtomicUsize::new(0));
    let count_thread = Arc::clone(&count);
    let mut signals = Signals::new([sig]).map_err(|e| SignalError::Hook(e.to_string()))?;
    thread::spawn(move || {
        for _ in signals.forever() {
            count_thread.fetch_add(1, Ordering::SeqCst);
        }
    });
    map.insert(sig, Counter { count });
    Ok(())
}

/// Number of times `signal_name` fired since last `pending` call.
/// The counter is reset to zero on read (like `sig_atomic_t` polling).
pub fn pending(signal_name: &str) -> Result<usize, SignalError> {
    let sig = resolve(signal_name)?;
    let map = counters().lock().expect("signal counters poisoned");
    if let Some(entry) = map.get(&sig) {
        Ok(entry.count.swap(0, Ordering::SeqCst))
    } else {
        Ok(0)
    }
}

/// Blocks up to `timeout_ms` for ANY installed signal to fire. Returns
/// the signal name that fired first, or "timeout".
pub fn wait_any(timeout_ms: u64) -> Result<String, SignalError> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        {
            let map = counters().lock().expect("signal counters poisoned");
            for (sig, entry) in map.iter() {
                if entry.count.load(Ordering::SeqCst) > 0 {
                    entry.count.fetch_sub(1, Ordering::SeqCst);
                    return Ok(name_from(*sig).into());
                }
            }
        }
        if Instant::now() >= deadline { return Ok("timeout".into()); }
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_common_signals() {
        assert_eq!(resolve("SIGINT").unwrap(),  SIGINT);
        assert_eq!(resolve("term").unwrap(),    SIGTERM);
        assert_eq!(resolve("USR1").unwrap(),    SIGUSR1);
        assert!(resolve("SIGZOMBIE").is_err());
    }

    #[test]
    fn install_is_idempotent() {
        install("SIGUSR2").unwrap();
        install("SIGUSR2").unwrap();
        // After install, pending must not blow up.
        assert_eq!(pending("SIGUSR2").unwrap(), 0);
    }

    #[test]
    fn wait_any_times_out_cleanly() {
        install("SIGUSR2").unwrap();
        assert_eq!(wait_any(50).unwrap(), "timeout");
    }
}
