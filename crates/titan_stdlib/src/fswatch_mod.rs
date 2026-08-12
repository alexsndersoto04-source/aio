//! File-system watcher (`std::fswatch::*`) via the cross-platform `notify`
//! crate. Uses inotify on Linux/Android.
//!
//! Two flavours to keep the .titan surface simple:
//!
//! * `watch_once(path, timeout_ms)` — blocks until the first event or the
//!   timeout, then returns a description string. Perfect for demos and
//!   scripts that don't want to manage a long-lived watcher.
//! * Runtime-owned `open` / `next_event` / `close` handles for daemons that
//!   need a persistent watcher and pull events in a loop. Handles from one
//!   VM are rejected by every other VM in the process.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

const MAX_WATCHERS_PER_RUNTIME: usize = 32;
const MAX_PENDING_EVENTS: usize = 1_024;
const MAX_WATCH_PATH_BYTES: usize = 16 * 1024;
const MAX_EVENT_DESCRIPTION_BYTES: usize = 64 * 1024;
const MAX_WATCH_TIMEOUT_MS: u64 = 24 * 60 * 60 * 1_000;

#[derive(Debug, Error)]
pub enum FsWatchError {
    #[error("watcher error: {0}")]
    Notify(String),
    #[error("no watcher registered under handle {0}")]
    UnknownHandle(i64),
    #[error("watcher I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{resource} exceeds limit {limit}")]
    ResourceLimit { resource: &'static str, limit: u64 },
    #[error("watcher handle space exhausted")]
    HandleSpaceExhausted,
}

fn nerr(error: impl std::fmt::Display) -> FsWatchError {
    FsWatchError::Notify(error.to_string())
}

struct WatcherPermit {
    runtime_id: u64,
}

impl Drop for WatcherPermit {
    fn drop(&mut self) {
        let mut usage = crate::native::lock_recover(watcher_usage());
        if let Some(active) = usage.get_mut(&self.runtime_id) {
            *active = active.saturating_sub(1);
            if *active == 0 {
                usage.remove(&self.runtime_id);
            }
        }
    }
}

fn watcher_usage() -> &'static Mutex<HashMap<u64, usize>> {
    static USAGE: OnceLock<Mutex<HashMap<u64, usize>>> = OnceLock::new();
    USAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reserve_watcher() -> Result<WatcherPermit, FsWatchError> {
    let runtime_id = crate::native::current_runtime_id();
    let mut usage = crate::native::lock_recover(watcher_usage());
    let active = usage.entry(runtime_id).or_insert(0);
    if *active >= MAX_WATCHERS_PER_RUNTIME {
        return Err(FsWatchError::ResourceLimit {
            resource: "filesystem watchers",
            limit: MAX_WATCHERS_PER_RUNTIME as u64,
        });
    }
    *active += 1;
    Ok(WatcherPermit { runtime_id })
}

fn validate_request(path: &str, timeout_ms: Option<u64>) -> Result<(), FsWatchError> {
    if path.len() > MAX_WATCH_PATH_BYTES {
        return Err(FsWatchError::ResourceLimit {
            resource: "watch path bytes",
            limit: MAX_WATCH_PATH_BYTES as u64,
        });
    }
    if timeout_ms.is_some_and(|timeout| timeout > MAX_WATCH_TIMEOUT_MS) {
        return Err(FsWatchError::ResourceLimit {
            resource: "watch timeout milliseconds",
            limit: MAX_WATCH_TIMEOUT_MS,
        });
    }
    Ok(())
}

struct WatcherEntry {
    _permit: WatcherPermit,
    _watcher: RecommendedWatcher,
    events: Arc<Mutex<mpsc::Receiver<notify::Result<Event>>>>,
    root: PathBuf,
    root_canon: PathBuf,
    root_existed: bool,
}

struct Registry {
    entries: HashMap<(u64, i64), WatcherEntry>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        Mutex::new(Registry {
            entries: HashMap::new(),
            next_id: 1,
        })
    })
}

fn handle_key(handle: i64) -> (u64, i64) {
    crate::native::runtime_handle_key(handle)
}

/// Format an `EventKind` into a compact string like "create", "modify",
/// "remove", "rename" or "other".
fn kind_name(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::Access(_) => "access",
        EventKind::Create(_) => "create",
        EventKind::Modify(_) => "modify",
        EventKind::Remove(_) => "remove",
        EventKind::Other => "other",
        EventKind::Any => "any",
    }
}

fn push_bounded(output: &mut String, value: &str) -> bool {
    let remaining = MAX_EVENT_DESCRIPTION_BYTES.saturating_sub(output.len());
    if value.len() <= remaining {
        output.push_str(value);
        return true;
    }
    let mut end = remaining;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    output.push_str(&value[..end]);
    false
}

fn describe(event: &notify::Event) -> String {
    let mut output = format!("{}:", kind_name(&event.kind));
    for (index, path) in event.paths.iter().enumerate() {
        if index > 0 && !push_bounded(&mut output, ",") {
            break;
        }
        if !push_bounded(&mut output, &path.to_string_lossy()) {
            break;
        }
    }
    output
}

// ---------------- One-shot -------------------------------------------

/// True when `p` refers to the watched root. Compared twice — verbatim and
/// canonicalized — because on macOS `/var` is a symlink to `/private/var`:
/// callers typically pass `$TMPDIR` verbatim (`/var/folders/…`) while
/// FSEvents reports canonical kernel paths (`/private/var/folders/…`), so a
/// plain string compare never matches and the phantom `create:` of the
/// watched root leaks through (observed on GitHub's macos runners).
fn path_is_root(p: &std::path::Path, root: &std::path::Path, root_canon: &std::path::Path) -> bool {
    if p == root || p == root_canon {
        return true;
    }
    std::fs::canonicalize(p).ok().as_deref() == Some(root_canon)
}

/// Receive the first REAL event within `timeout`, preserving the remaining
/// budget across discards. Phantom events that predate the watch are dropped:
/// FSEvents (macOS) coalesces its journal and may deliver a `create` for the
/// watched root even though it already existed when we registered — a stale
/// echo from the past, not something that happened while watching. A `create`
/// of a path that existed at registration time is logically impossible.
fn recv_fresh(
    rx: &mpsc::Receiver<notify::Result<Event>>,
    timeout: Duration,
    root: &PathBuf,
    root_canon: &PathBuf,
    root_existed: bool,
) -> Result<notify::Result<Event>, mpsc::RecvTimeoutError> {
    let started = Instant::now();
    let deadline = started.checked_add(timeout).unwrap_or(started);
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(mpsc::RecvTimeoutError::Timeout);
        }
        match rx.recv_timeout(deadline - now) {
            Ok(Ok(ref event)) => {
                let stale = root_existed
                    && matches!(event.kind, EventKind::Create(_))
                    && !event.paths.is_empty()
                    && event
                        .paths
                        .iter()
                        .all(|p| path_is_root(p, root, root_canon));
                if stale {
                    continue;
                }
                return Ok(Ok(event.clone()));
            }
            Ok(Err(error)) => return Ok(Err(error)),
            Err(e) => return Err(e),
        }
    }
}

/// Watch `path` and return the first event (or "timeout" on expiry).
pub fn watch_once(path: &str, timeout_ms: u64, recursive: bool) -> Result<String, FsWatchError> {
    validate_request(path, Some(timeout_ms))?;
    let _permit = reserve_watcher()?;
    let (tx, rx) = mpsc::sync_channel(MAX_PENDING_EVENTS);
    let mut watcher = recommended_watcher(move |result| {
        let _ = tx.try_send(result);
    })
    .map_err(nerr)?;
    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    let root = PathBuf::from(path);
    let root_existed = root.exists();
    let root_canon = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    watcher.watch(&root, mode).map_err(nerr)?;
    match recv_fresh(
        &rx,
        Duration::from_millis(timeout_ms),
        &root,
        &root_canon,
        root_existed,
    ) {
        Ok(Ok(event)) => Ok(describe(&event)),
        Ok(Err(error)) => Err(nerr(error)),
        Err(_) => Ok("timeout".into()),
    }
}

// ---------------- Registry-based -------------------------------------

/// Open a watcher on `path` and return an opaque handle.
pub fn open(path: &str, recursive: bool) -> Result<i64, FsWatchError> {
    validate_request(path, None)?;
    let permit = reserve_watcher()?;
    let (tx, rx) = mpsc::sync_channel(MAX_PENDING_EVENTS);
    let mut watcher = recommended_watcher(move |result| {
        let _ = tx.try_send(result);
    })
    .map_err(nerr)?;
    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    let root = PathBuf::from(path);
    let root_existed = root.exists();
    let root_canon = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    watcher.watch(&root, mode).map_err(nerr)?;
    let mut reg = crate::native::lock_recover(registry());
    let id = reg.next_id;
    reg.next_id = id
        .checked_add(1)
        .ok_or(FsWatchError::HandleSpaceExhausted)?;
    reg.entries.insert(
        handle_key(id),
        WatcherEntry {
            _permit: permit,
            _watcher: watcher,
            events: Arc::new(Mutex::new(rx)),
            root,
            root_canon,
            root_existed,
        },
    );
    Ok(id)
}

/// Pull the next event on `handle`. Returns "timeout" if none arrives
/// within `timeout_ms`.
pub fn next_event(handle: i64, timeout_ms: u64) -> Result<String, FsWatchError> {
    validate_request("", Some(timeout_ms))?;
    let (events, root, root_canon, root_existed) = {
        let registry = crate::native::lock_recover(registry());
        let entry = registry
            .entries
            .get(&handle_key(handle))
            .ok_or(FsWatchError::UnknownHandle(handle))?;
        (
            Arc::clone(&entry.events),
            entry.root.clone(),
            entry.root_canon.clone(),
            entry.root_existed,
        )
    };
    let events = crate::native::lock_recover(&events);
    match recv_fresh(
        &events,
        Duration::from_millis(timeout_ms),
        &root,
        &root_canon,
        root_existed,
    ) {
        Ok(Ok(event)) => Ok(describe(&event)),
        Ok(Err(error)) => Err(nerr(error)),
        Err(_) => Ok("timeout".into()),
    }
}

pub fn close(handle: i64) {
    let removed = crate::native::lock_recover(registry())
        .entries
        .remove(&handle_key(handle));
    drop(removed);
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let removed = {
        let mut registry = crate::native::lock_recover(registry());
        let keys = registry
            .entries
            .keys()
            .filter(|(owner, _)| *owner == runtime_id)
            .copied()
            .collect::<Vec<_>>();
        keys.into_iter()
            .filter_map(|key| registry.entries.remove(&key))
            .collect::<Vec<_>>()
    };
    let released = removed.len();
    drop(removed);
    released
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_handle_reports_error() {
        assert!(next_event(9_999_999, 10).is_err());
        // close on unknown id is a no-op.
        close(9_999_999);
    }

    #[test]
    fn watcher_quota_paths_timeouts_and_event_descriptions_are_bounded() {
        let runtime_id = 8_300_004;
        crate::native::with_runtime_context(runtime_id, || {
            let mut permits = (0..MAX_WATCHERS_PER_RUNTIME)
                .map(|_| reserve_watcher().unwrap())
                .collect::<Vec<_>>();
            assert!(matches!(
                reserve_watcher(),
                Err(FsWatchError::ResourceLimit {
                    resource: "filesystem watchers",
                    ..
                })
            ));
            drop(permits.pop());
            permits.push(reserve_watcher().unwrap());
        });
        assert!(!crate::native::lock_recover(watcher_usage()).contains_key(&runtime_id));

        assert!(matches!(
            watch_once(&"x".repeat(MAX_WATCH_PATH_BYTES + 1), 1, false),
            Err(FsWatchError::ResourceLimit { .. })
        ));
        assert!(matches!(
            next_event(1, MAX_WATCH_TIMEOUT_MS + 1),
            Err(FsWatchError::ResourceLimit { .. })
        ));
        let event = Event {
            kind: EventKind::Any,
            paths: vec![PathBuf::from("x".repeat(MAX_EVENT_DESCRIPTION_BYTES + 1))],
            attrs: Default::default(),
        };
        assert_eq!(describe(&event).len(), MAX_EVENT_DESCRIPTION_BYTES);
    }

    #[test]
    fn watch_once_times_out_cleanly_on_idle_dir() {
        let dir = std::env::temp_dir().join(format!("titan-fswatch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = watch_once(dir.to_string_lossy().as_ref(), 100, false).unwrap();
        assert_eq!(result, "timeout");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
