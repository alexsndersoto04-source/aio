//! Unix signals (`std::signals::*`) via the `signal-hook` crate.
//!
//! Each runtime gets an independent pending counter. A process signal is
//! broadcast to every runtime that installed it, so polling in one VM cannot
//! consume another VM's notification.

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
        "SIGINT" | "INT" => Ok(SIGINT),
        "SIGTERM" | "TERM" => Ok(SIGTERM),
        "SIGHUP" | "HUP" => Ok(SIGHUP),
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
        SIGINT => "SIGINT",
        SIGTERM => "SIGTERM",
        SIGHUP => "SIGHUP",
        SIGUSR1 => "SIGUSR1",
        SIGUSR2 => "SIGUSR2",
        SIGQUIT => "SIGQUIT",
        SIGPIPE => "SIGPIPE",
        SIGCHLD => "SIGCHLD",
        _ => "OTHER",
    }
}

#[derive(Default)]
struct SignalSubscribers {
    runtimes: HashMap<u64, Arc<AtomicUsize>>,
}

fn subscribers() -> &'static Mutex<HashMap<i32, SignalSubscribers>> {
    static SUBSCRIBERS: OnceLock<Mutex<HashMap<i32, SignalSubscribers>>> = OnceLock::new();
    SUBSCRIBERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a signal listener for the current runtime. Idempotent per runtime.
pub fn install(signal_name: &str) -> Result<(), SignalError> {
    let signal = resolve(signal_name)?;
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(subscribers());
    if let Some(entry) = registry.get_mut(&signal) {
        entry
            .runtimes
            .entry(runtime_id)
            .or_insert_with(|| Arc::new(AtomicUsize::new(0)));
        return Ok(());
    }

    let mut signals =
        Signals::new([signal]).map_err(|error| SignalError::Hook(error.to_string()))?;
    let mut entry = SignalSubscribers::default();
    entry
        .runtimes
        .insert(runtime_id, Arc::new(AtomicUsize::new(0)));
    registry.insert(signal, entry);
    drop(registry);

    thread::spawn(move || {
        for _ in signals.forever() {
            let registry = crate::native::lock_recover(subscribers());
            if let Some(entry) = registry.get(&signal) {
                for counter in entry.runtimes.values() {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
    });
    Ok(())
}

/// Number of times `signal_name` fired since this runtime's last poll.
pub fn pending(signal_name: &str) -> Result<usize, SignalError> {
    let signal = resolve(signal_name)?;
    let runtime_id = crate::native::current_runtime_id();
    let registry = crate::native::lock_recover(subscribers());
    Ok(registry
        .get(&signal)
        .and_then(|entry| entry.runtimes.get(&runtime_id))
        .map_or(0, |counter| counter.swap(0, Ordering::SeqCst)))
}

/// Blocks up to `timeout_ms` for any signal installed by this runtime.
pub fn wait_any(timeout_ms: u64) -> Result<String, SignalError> {
    let runtime_id = crate::native::current_runtime_id();
    let timeout = Duration::from_millis(timeout_ms);
    let started = Instant::now();
    loop {
        {
            let registry = crate::native::lock_recover(subscribers());
            for (signal, entry) in registry.iter() {
                if let Some(counter) = entry.runtimes.get(&runtime_id) {
                    if counter
                        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                            count.checked_sub(1)
                        })
                        .is_ok()
                    {
                        return Ok(name_from(*signal).into());
                    }
                }
            }
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            return Ok("timeout".into());
        }
        thread::sleep((timeout - elapsed).min(Duration::from_millis(20)));
    }
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut registry = crate::native::lock_recover(subscribers());
    registry
        .values_mut()
        .map(|entry| usize::from(entry.runtimes.remove(&runtime_id).is_some()))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_common_signals() {
        assert_eq!(resolve("SIGINT").unwrap(), SIGINT);
        assert_eq!(resolve("term").unwrap(), SIGTERM);
        assert_eq!(resolve("USR1").unwrap(), SIGUSR1);
        assert!(resolve("SIGZOMBIE").is_err());
    }

    #[test]
    fn install_is_idempotent() {
        install("SIGUSR2").unwrap();
        install("SIGUSR2").unwrap();
        assert_eq!(pending("SIGUSR2").unwrap(), 0);
    }

    #[test]
    fn pending_signals_and_cleanup_are_runtime_scoped() {
        let first = 83_001;
        let second = 83_002;
        crate::native::with_runtime_context(first, || install("SIGUSR1").unwrap());
        crate::native::with_runtime_context(second, || install("SIGUSR1").unwrap());

        {
            let registry = crate::native::lock_recover(subscribers());
            registry[&SIGUSR1].runtimes[&first].fetch_add(1, Ordering::SeqCst);
        }
        crate::native::with_runtime_context(second, || assert_eq!(pending("SIGUSR1").unwrap(), 0));
        crate::native::with_runtime_context(first, || assert_eq!(pending("SIGUSR1").unwrap(), 1));
        assert_eq!(cleanup_runtime(first), 1);
        assert_eq!(cleanup_runtime(second), 1);
    }
}
