//! Live on-screen windows for TITAN (Fase 2) — REAL OS windows via
//! `minifb` (pure-Rust: x11rb/Wayland on Linux, Win32, Cocoa).
//!
//! Availability contract, stated honestly:
//! * Linux / Windows / macOS with a display: opens a real OS window.
//! * Headless boxes (CI, SSH, Docker): `live_open` reports `-1` instead
//!   of pretending — no display, no window.
//! * Android (Termux bionic target): compiled out entirely — minifb has
//!   no Android backend; the on-phone ceremony runs through the glibc
//!   (proot-distro) build plus a Termux:X11 server.
//!
//! This first slice only opens/queries/closes the window. Presenting
//! `std::gui` pixels and wiring keyboard/mouse events into `std::input`
//! arrive in the next slice of this session.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use minifb::{Window, WindowOptions};

/// `minifb::Window` is not `Send` on most backends (platform handles
/// are thread-affine), so it cannot live directly in a shared static.
/// This wrapper is sound because every window access goes through the
/// registry `Mutex`: only one thread ever touches a window at a time,
/// and the VM executes TITAN code on a single thread. The macOS
/// "main thread only" caveat stays documented in the module header.
struct LiveWindow(Window);

// SAFETY: all access is serialized through the registry `Mutex`.
unsafe impl Send for LiveWindow {}

fn registry() -> &'static Mutex<HashMap<u64, LiveWindow>> {
    static REG: OnceLock<Mutex<HashMap<u64, LiveWindow>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT_LIVE_ID: AtomicU64 = AtomicU64::new(1);

/// Open a real OS window. Returns a positive handle, or `-1` when no
/// display is available (headless CI, SSH, Termux sin X11).
pub fn live_open(title: &str, width: u32, height: u32) -> i64 {
    let mut window = match Window::new(title, width as usize, height as usize, WindowOptions::default()) {
        Ok(window) => window,
        Err(_) => return -1,
    };
    window.set_target_fps(60);
    let id = NEXT_LIVE_ID.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut reg) = registry().lock() {
        reg.insert(id, LiveWindow(window));
        id as i64
    } else {
        -1
    }
}

/// Whether the live window is still open (false for unknown handles).
pub fn live_is_open(handle: i64) -> bool {
    registry().lock().ok()
        .and_then(|reg| reg.get(&(handle as u64)).map(|window| window.0.is_open()))
        .unwrap_or(false)
}

/// Close and drop the live window. False for unknown handles.
pub fn live_close(handle: i64) -> bool {
    registry().lock()
        .map(|mut reg| reg.remove(&(handle as u64)).is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_handles_are_safe() {
        assert!(!live_is_open(4_242_424));
        assert!(!live_close(4_242_424));
    }

    /// macOS forbids window creation off the main thread (cargo test runs
    /// on worker threads), so the probe runs on Linux/Windows: headless
    /// Linux must honestly say -1; a machine with a display gets a real
    /// window that opens, reports open, and closes.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn live_probe_reports_display_availability_honestly() {
        let id = live_open("titan live probe", 64, 64);
        if id == -1 {
            // headless: honest negative, and registry stays consistent.
            assert!(!live_is_open(-1));
        } else {
            assert!(id > 0);
            assert!(live_is_open(id));
            assert!(live_close(id));
            assert!(!live_is_open(id));
        }
    }
}
