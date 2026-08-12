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
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsWatchError {
    #[error("watcher error: {0}")]
    Notify(String),
    #[error("no watcher registered under handle {0}")]
    UnknownHandle(i64),
    #[error("watcher I/O error: {0}")]
    Io(#[from] std::io::Error),
}

fn nerr(error: impl std::fmt::Display) -> FsWatchError {
    FsWatchError::Notify(error.to_string())
}

struct WatcherEntry {
    _watcher: RecommendedWatcher,
    events: mpsc::Receiver<notify::Result<Event>>,
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

fn describe(event: &notify::Event) -> String {
    let paths: Vec<String> = event
        .paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    format!("{}:{}", kind_name(&event.kind), paths.join(","))
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
    let deadline = Instant::now() + timeout;
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
    let (tx, rx) = mpsc::channel();
    let mut watcher = recommended_watcher(move |result| {
        let _ = tx.send(result);
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
    let (tx, rx) = mpsc::channel();
    let mut watcher = recommended_watcher(move |result| {
        let _ = tx.send(result);
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
    let mut reg = registry().lock().expect("fswatch registry poisoned");
    let id = reg.next_id;
    reg.next_id += 1;
    reg.entries.insert(
        handle_key(id),
        WatcherEntry {
            _watcher: watcher,
            events: rx,
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
    let reg = registry().lock().expect("fswatch registry poisoned");
    let entry = reg
        .entries
        .get(&handle_key(handle))
        .ok_or(FsWatchError::UnknownHandle(handle))?;
    match recv_fresh(
        &entry.events,
        Duration::from_millis(timeout_ms),
        &entry.root,
        &entry.root_canon,
        entry.root_existed,
    ) {
        Ok(Ok(event)) => Ok(describe(&event)),
        Ok(Err(error)) => Err(nerr(error)),
        Err(_) => Ok("timeout".into()),
    }
}

pub fn close(handle: i64) {
    if let Ok(mut reg) = registry().lock() {
        reg.entries.remove(&handle_key(handle));
    }
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut reg = crate::native::lock_recover(registry());
    crate::native::remove_runtime_entries(&mut reg.entries, runtime_id)
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
    fn watch_once_times_out_cleanly_on_idle_dir() {
        let dir = std::env::temp_dir().join(format!("titan-fswatch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = watch_once(dir.to_string_lossy().as_ref(), 100, false).unwrap();
        assert_eq!(result, "timeout");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
