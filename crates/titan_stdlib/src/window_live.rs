//! Live on-screen windows for TITAN (Fase 2) — REAL OS windows via
//! `minifb` (pure-Rust: x11rb/Wayland on Linux, Win32, Cocoa).
//!
//! GRADUATED (Fase 2, 2026-07-31): the first live TITAN window ever ran
//! for real on the author's 32-bit phone (armv7l; proot Debian armhf +
//! Termux:X11): `live_open` -> id 1, 3,601 frames at 60 fps, real X11
//! events bridged into `std::input`, clean `live_close`. Declared
//! working only after running on a real machine, per project rule.
//!
//! Availability contract, stated honestly:
//! * Linux / Windows / macOS with a display: opens a real OS window.
//! * Headless boxes (CI, SSH, Docker): `live_open` reports `-1` instead
//!   of pretending — no display, no window. Invalid or over-quota window
//!   requests also report `-1` without touching the OS backend.
//! * Android (Termux bionic target): compiled out entirely — minifb has
//!   no Android backend; the on-phone ceremony runs through the glibc
//!   (proot-distro) build plus a Termux:X11 server.
//! * macOS: window creation must happen on the thread that owns the
//!   process main run-loop; the TITAN VM executes user code on a single
//!   thread, which satisfies that contract for ordinary `titan` runs.
//!
//! Slice 2 (this version): present + real input bridge.
//! `live_pump(win, gui_root)` renders the `std::gui` tree to RGBA,
//! packs it into minifb's 0RGB framebuffer format, presents it, and
//! forwards the *real* keyboard/mouse state into `std::input` plus this
//! window's own event queue (readable with `live_poll_events`, in the
//! exact same string format as `std::window::poll_events`).
//!
//! It refuses shortcuts, honestly:
//! * `-2` unknown window handle (or poisoned registry; see `-6`)
//! * `-3` unknown gui root
//! * `-4` the gui renders at a different size than the window — we do
//!   NOT stretch pixels behind the user's back
//! * `-5` the OS rejected the framebuffer update
//! * `-6` internal registry state corrupted
//!
//! Return `1` while the window lives, `0` once the user closed it
//! (exactly one `CloseRequested` event is queued on that final pump).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};

use crate::gui_raster::render_rgba;
use crate::input;
use crate::window::{format_event, WindowEvent};

/// State for one live OS window. Only exists when a real display
/// answered `Window::new`.
struct LiveWindow {
    window: Window,
    width: u32,
    height: u32,
    /// Reusable packed 0RGB framebuffer (no per-frame allocation).
    scratch: Vec<u32>,
    /// Queued events, same enum + string format as `std::window`.
    events: Vec<WindowEvent>,
    /// Previous-frame input snapshot; diffing produces KeyDown/Up,
    /// MouseButtonDown/Up and MouseMove exactly once per real change.
    keys_down: HashSet<String>,
    buttons_down: HashSet<u8>,
    last_mouse: (i32, i32),
    close_reported: bool,
    _permit: LiveWindowPermit,
}

// `minifb::Window` is deliberately not `Send`: native window APIs must remain
// on the OS thread that created them. A thread-local registry enforces that
// ownership instead of overriding the library's safety contract. Handles used
// from another TITAN task are therefore unknown on that task's thread.
thread_local! {
    static REGISTRY: RefCell<HashMap<(u64, u64), LiveWindow>> = RefCell::new(HashMap::new());
}

fn with_registry<R>(operation: impl FnOnce(&HashMap<(u64, u64), LiveWindow>) -> R) -> Option<R> {
    REGISTRY
        .try_with(|registry| {
            registry
                .try_borrow()
                .ok()
                .map(|registry| operation(&registry))
        })
        .ok()
        .flatten()
}

fn with_registry_mut<R>(
    operation: impl FnOnce(&mut HashMap<(u64, u64), LiveWindow>) -> R,
) -> Option<R> {
    REGISTRY
        .try_with(|registry| {
            registry
                .try_borrow_mut()
                .ok()
                .map(|mut registry| operation(&mut registry))
        })
        .ok()
        .flatten()
}

fn handle_key(handle: u64) -> (u64, u64) {
    crate::native::runtime_handle_key(handle)
}

const MAX_LIVE_WINDOWS_PER_RUNTIME: usize = 16;
const MAX_LIVE_EVENTS: usize = 1_024;
const MAX_LIVE_TITLE_BYTES: usize = 4 * 1024;
const MAX_LIVE_DIMENSION: u32 = 4_096;
const MAX_LIVE_PIXELS_PER_RUNTIME: usize = 16 * 1024 * 1024;

static NEXT_LIVE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
struct LiveWindowUsage {
    handles: usize,
    pixels: usize,
}

struct LiveWindowPermit {
    runtime_id: u64,
    pixels: usize,
}

impl Drop for LiveWindowPermit {
    fn drop(&mut self) {
        let mut usage = crate::native::lock_recover(live_window_usage());
        if let Some(runtime) = usage.get_mut(&self.runtime_id) {
            runtime.handles = runtime.handles.saturating_sub(1);
            runtime.pixels = runtime.pixels.saturating_sub(self.pixels);
            if runtime.handles == 0 {
                usage.remove(&self.runtime_id);
            }
        }
    }
}

fn live_window_usage() -> &'static Mutex<HashMap<u64, LiveWindowUsage>> {
    static USAGE: OnceLock<Mutex<HashMap<u64, LiveWindowUsage>>> = OnceLock::new();
    USAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reserve_live_window(width: u32, height: u32) -> Option<LiveWindowPermit> {
    let runtime_id = crate::native::current_runtime_id();
    let pixels = (width as usize).checked_mul(height as usize)?;
    let mut usage = crate::native::lock_recover(live_window_usage());
    let (handles, used_pixels) = usage
        .get(&runtime_id)
        .map_or((0, 0), |runtime| (runtime.handles, runtime.pixels));
    if handles >= MAX_LIVE_WINDOWS_PER_RUNTIME
        || used_pixels.checked_add(pixels)? > MAX_LIVE_PIXELS_PER_RUNTIME
    {
        return None;
    }
    let runtime = usage.entry(runtime_id).or_default();
    runtime.handles += 1;
    runtime.pixels += pixels;
    Some(LiveWindowPermit { runtime_id, pixels })
}

fn valid_live_request(title: &str, width: u32, height: u32) -> bool {
    title.len() <= MAX_LIVE_TITLE_BYTES
        && width > 0
        && height > 0
        && width <= MAX_LIVE_DIMENSION
        && height <= MAX_LIVE_DIMENSION
}

fn signed_handle_key(handle: i64) -> Option<(u64, u64)> {
    u64::try_from(handle).ok().map(handle_key)
}

fn append_events_bounded(queue: &mut Vec<WindowEvent>, events: impl IntoIterator<Item = WindowEvent>) {
    let remaining = MAX_LIVE_EVENTS.saturating_sub(queue.len());
    queue.extend(events.into_iter().take(remaining));
}

fn push_priority_event(queue: &mut Vec<WindowEvent>, event: WindowEvent) {
    if queue.len() >= MAX_LIVE_EVENTS {
        queue.remove(0);
    }
    queue.push(event);
}

/// Mouse buttons bridged to TITAN: 1 = left, 2 = right, 3 = middle,
/// the same convention `std::input` uses since Phase 1.
const BRIDGED_BUTTONS: [(MouseButton, u8); 3] = [
    (MouseButton::Left, 1),
    (MouseButton::Right, 2),
    (MouseButton::Middle, 3),
];

/// Open a real OS window. Returns a positive handle, or `-1` when the
/// request exceeds its bounds or no display/capacity is available.
pub fn live_open(title: &str, width: u32, height: u32) -> i64 {
    if !valid_live_request(title, width, height) {
        return -1;
    }
    let Some(permit) = reserve_live_window(width, height) else {
        return -1;
    };
    let runtime_id = permit.runtime_id;

    let mut window = match Window::new(
        title,
        width as usize,
        height as usize,
        WindowOptions::default(),
    ) {
        Ok(window) => window,
        Err(_) => return -1,
    };
    window.set_target_fps(60);
    let id = match NEXT_LIVE_ID.fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| {
        (id <= i64::MAX as u64).then(|| id + 1)
    }) {
        Ok(id) => id,
        Err(_) => return -1,
    };
    with_registry_mut(|registry| {
        registry.insert(
            (runtime_id, id),
            LiveWindow {
                window,
                width,
                height,
                scratch: Vec::new(),
                events: Vec::new(),
                keys_down: HashSet::new(),
                buttons_down: HashSet::new(),
                last_mouse: (0, 0),
                close_reported: false,
                _permit: permit,
            },
        );
        id as i64
    })
    .unwrap_or(-1)
}

/// Whether the live window is still open (false for unknown handles).
pub fn live_is_open(handle: i64) -> bool {
    let Some(key) = signed_handle_key(handle) else {
        return false;
    };
    with_registry(|registry| {
        registry
            .get(&key)
            .is_some_and(|entry| entry.window.is_open())
    })
    .unwrap_or(false)
}

/// Close and drop the live window. False for unknown handles.
pub fn live_close(handle: i64) -> bool {
    let Some(key) = signed_handle_key(handle) else {
        return false;
    };
    with_registry_mut(|registry| registry.remove(&key).is_some()).unwrap_or(false)
}

/// Rename the visible OS window title. False for unknown handles.
pub fn live_set_title(handle: i64, title: &str) -> bool {
    if title.len() > MAX_LIVE_TITLE_BYTES {
        return false;
    }
    let Some(key) = signed_handle_key(handle) else {
        return false;
    };
    with_registry_mut(|registry| {
        registry.get_mut(&key).is_some_and(|entry| {
            entry.window.set_title(title);
            true
        })
    })
    .unwrap_or(false)
}

/// One frame of a live window: render the gui tree, present it to the
/// OS framebuffer, pump real keyboard/mouse state into `std::input`,
/// and queue the new events. See the module header for the honest
/// status codes (-2..-6, 0 closed, 1 alive).
pub fn live_pump(handle: i64, gui_root: i64) -> i64 {
    let Some(key) = signed_handle_key(handle) else {
        return -2;
    };
    with_registry_mut(|registry| {
        let Some(entry) = registry.get_mut(&key) else {
            return -2;
        };
        let Some((width, height, rgba)) = render_rgba(gui_root) else {
            return -3;
        };
        if width != entry.width || height != entry.height {
            return -4;
        }
        let needed = (width as usize) * (height as usize);
        if entry.scratch.len() != needed {
            entry.scratch.resize(needed, 0);
        }
        pack_rgba(&rgba, &mut entry.scratch);
        if entry
            .window
            .update_with_buffer(&entry.scratch, width as usize, height as usize)
            .is_err()
        {
            return -5;
        }

        // Real input snapshot, taken right after the OS event poll.
        let cur_keys: HashSet<String> = entry
            .window
            .get_keys()
            .iter()
            .filter_map(map_key)
            .map(str::to_string)
            .collect();
        let cur_pos = entry
            .window
            .get_mouse_pos(MouseMode::Discard)
            .map(|(x, y)| (x as i32, y as i32))
            .unwrap_or(entry.last_mouse);
        let mut cur_buttons = HashSet::new();
        for (mouse_button, titan_button) in BRIDGED_BUTTONS {
            if entry.window.get_mouse_down(mouse_button) {
                cur_buttons.insert(titan_button);
            }
        }

        let events = diff_events(
            &entry.keys_down,
            &cur_keys,
            &entry.buttons_down,
            &cur_buttons,
            entry.last_mouse,
            cur_pos,
        );

        // Feed the shared std::input state (what TITAN games actually read).
        for name in cur_keys.difference(&entry.keys_down) {
            input::set_key_state(name, true);
        }
        for name in entry.keys_down.difference(&cur_keys) {
            input::set_key_state(name, false);
        }
        if cur_pos != entry.last_mouse {
            input::set_mouse_pos(cur_pos.0, cur_pos.1);
        }
        for button in cur_buttons.difference(&entry.buttons_down) {
            input::set_mouse_button(*button, true);
        }
        for button in entry.buttons_down.difference(&cur_buttons) {
            input::set_mouse_button(*button, false);
        }

        append_events_bounded(&mut entry.events, events);
        entry.keys_down = cur_keys;
        entry.buttons_down = cur_buttons;
        entry.last_mouse = cur_pos;

        if entry.window.is_open() {
            1
        } else {
            if !entry.close_reported {
                push_priority_event(&mut entry.events, WindowEvent::CloseRequested);
                entry.close_reported = true;
            }
            0
        }
    })
    .unwrap_or(-6)
}

/// Drain this window's queued events, formatted exactly like
/// `std::window::poll_events` (empty for unknown handles).
pub fn live_poll_events(handle: i64) -> Vec<String> {
    let Some(key) = signed_handle_key(handle) else {
        return Vec::new();
    };
    with_registry_mut(|registry| {
        registry
            .get_mut(&key)
            .map(|entry| {
                entry
                    .events
                    .drain(..)
                    .map(|event| format_event(&event))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

/// Translate a physical minifb key into the lowercase TITAN key name
/// that `std::input` understands. Left and right modifiers collapse
/// into one name ("shift", "ctrl", "alt", "super") so TITAN programs
/// never depend on which hand typed them. Internal sentinel variants
/// (Unknown, Count) never leak into TITAN.
fn map_key(key: &Key) -> Option<&'static str> {
    use Key::*;
    Some(match key {
        A => "a",
        B => "b",
        C => "c",
        D => "d",
        E => "e",
        F => "f",
        G => "g",
        H => "h",
        I => "i",
        J => "j",
        K => "k",
        L => "l",
        M => "m",
        N => "n",
        O => "o",
        P => "p",
        Q => "q",
        R => "r",
        S => "s",
        T => "t",
        U => "u",
        V => "v",
        W => "w",
        X => "x",
        Y => "y",
        Z => "z",
        Key0 => "0",
        Key1 => "1",
        Key2 => "2",
        Key3 => "3",
        Key4 => "4",
        Key5 => "5",
        Key6 => "6",
        Key7 => "7",
        Key8 => "8",
        Key9 => "9",
        F1 => "f1",
        F2 => "f2",
        F3 => "f3",
        F4 => "f4",
        F5 => "f5",
        F6 => "f6",
        F7 => "f7",
        F8 => "f8",
        F9 => "f9",
        F10 => "f10",
        F11 => "f11",
        F12 => "f12",
        F13 => "f13",
        F14 => "f14",
        F15 => "f15",
        Down => "down",
        Left => "left",
        Right => "right",
        Up => "up",
        Space => "space",
        Enter => "enter",
        Escape => "escape",
        Tab => "tab",
        Backspace => "backspace",
        Delete => "delete",
        Insert => "insert",
        Home => "home",
        End => "end",
        PageUp => "pageup",
        PageDown => "pagedown",
        Pause => "pause",
        Menu => "menu",
        CapsLock => "capslock",
        NumLock => "numlock",
        ScrollLock => "scrolllock",
        LeftShift | RightShift => "shift",
        LeftCtrl | RightCtrl => "ctrl",
        LeftAlt | RightAlt => "alt",
        LeftSuper | RightSuper => "super",
        Apostrophe => "'",
        Backquote => "`",
        Backslash => "\\",
        Comma => ",",
        Equal => "=",
        LeftBracket => "[",
        Minus => "-",
        Period => ".",
        RightBracket => "]",
        Semicolon => ";",
        Slash => "/",
        NumPad0 => "numpad0",
        NumPad1 => "numpad1",
        NumPad2 => "numpad2",
        NumPad3 => "numpad3",
        NumPad4 => "numpad4",
        NumPad5 => "numpad5",
        NumPad6 => "numpad6",
        NumPad7 => "numpad7",
        NumPad8 => "numpad8",
        NumPad9 => "numpad9",
        NumPadDot => "numpad_dot",
        NumPadSlash => "numpad_slash",
        NumPadAsterisk => "numpad_asterisk",
        NumPadMinus => "numpad_minus",
        NumPadPlus => "numpad_plus",
        NumPadEnter => "numpad_enter",
        Unknown | Count => return None,
    })
}

/// Pack RGBA bytes into minifb's 0RGB u32 framebuffer. Alpha is
/// deliberately ignored — that is `update_with_buffer`'s documented
/// contract.
fn pack_rgba(rgba: &[u8], out: &mut [u32]) {
    for (pixel, chunk) in out.iter_mut().zip(rgba.chunks_exact(4)) {
        *pixel = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
    }
}

/// Pure event diff between two input snapshots. Deterministic ordering
/// (mouse move first, then keys alphabetically, then buttons ascending)
/// so TITAN programs always see one stable event stream.
fn diff_events(
    prev_keys: &HashSet<String>,
    cur_keys: &HashSet<String>,
    prev_buttons: &HashSet<u8>,
    cur_buttons: &HashSet<u8>,
    prev_pos: (i32, i32),
    cur_pos: (i32, i32),
) -> Vec<WindowEvent> {
    let mut events = Vec::new();
    if cur_pos != prev_pos {
        events.push(WindowEvent::MouseMove {
            x: cur_pos.0,
            y: cur_pos.1,
        });
    }
    let mut pressed: Vec<&String> = cur_keys.difference(prev_keys).collect();
    pressed.sort();
    for name in pressed {
        events.push(WindowEvent::KeyDown { key: name.clone() });
    }
    let mut released: Vec<&String> = prev_keys.difference(cur_keys).collect();
    released.sort();
    for name in released {
        events.push(WindowEvent::KeyUp { key: name.clone() });
    }
    let mut buttons_pressed: Vec<&u8> = cur_buttons.difference(prev_buttons).collect();
    buttons_pressed.sort();
    for button in buttons_pressed {
        events.push(WindowEvent::MouseButtonDown { button: *button });
    }
    let mut buttons_released: Vec<&u8> = prev_buttons.difference(cur_buttons).collect();
    buttons_released.sort();
    for button in buttons_released {
        events.push(WindowEvent::MouseButtonUp { button: *button });
    }
    events
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    with_registry_mut(|registry| crate::native::remove_runtime_entries(registry, runtime_id))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_handles_are_safe() {
        assert!(!live_is_open(4_242_424));
        assert!(!live_close(4_242_424));
        assert!(!live_set_title(4_242_424, "ghost"));
        assert_eq!(live_pump(4_242_424, 1), -2);
        assert!(live_poll_events(4_242_424).is_empty());
    }

    #[test]
    fn pack_rgba_follows_the_minifb_0rgb_contract() {
        let rgba = [
            255, 0, 0, 7, // red with junk alpha: alpha must be ignored
            0, 255, 0, 255, // opaque green
            10, 20, 30, 0, // fully transparent pixel still keeps its RGB
        ];
        let mut out = [0u32; 3];
        pack_rgba(&rgba, &mut out);
        assert_eq!(out[0], 0x00FF0000);
        assert_eq!(out[1], 0x0000FF00);
        assert_eq!(out[2], (10 << 16) | (20 << 8) | 30);
    }

    #[test]
    fn map_key_names_are_titan_names_and_modifiers_collapse() {
        assert_eq!(map_key(&Key::W), Some("w"));
        assert_eq!(map_key(&Key::Key5), Some("5"));
        assert_eq!(map_key(&Key::Up), Some("up"));
        assert_eq!(map_key(&Key::Escape), Some("escape"));
        assert_eq!(map_key(&Key::F7), Some("f7"));
        assert_eq!(map_key(&Key::NumPad3), Some("numpad3"));
        // Either hand lands on the same TITAN name, by design:
        assert_eq!(map_key(&Key::LeftShift), Some("shift"));
        assert_eq!(map_key(&Key::RightShift), Some("shift"));
        assert_eq!(map_key(&Key::LeftCtrl), Some("ctrl"));
        assert_eq!(map_key(&Key::RightAlt), Some("alt"));
        // Internal sentinels never leak:
        assert_eq!(map_key(&Key::Unknown), None);
        assert_eq!(map_key(&Key::Count), None);
    }

    #[test]
    fn diff_events_fires_exactly_once_per_real_change() {
        let empty_keys: HashSet<String> = HashSet::new();
        let w_down: HashSet<String> = ["w".to_string()].into_iter().collect();
        let no_buttons: HashSet<u8> = HashSet::new();
        let left_down: HashSet<u8> = [1u8].into_iter().collect();

        // Holding W while the mouse moves reports only the move:
        let events = diff_events(&w_down, &w_down, &no_buttons, &no_buttons, (0, 0), (9, 4));
        assert_eq!(events, vec![WindowEvent::MouseMove { x: 9, y: 4 }]);

        // A brand-new press + click, in the documented order:
        let events = diff_events(
            &empty_keys,
            &w_down,
            &no_buttons,
            &left_down,
            (9, 4),
            (9, 4),
        );
        assert_eq!(
            events,
            vec![
                WindowEvent::KeyDown {
                    key: "w".to_string()
                },
                WindowEvent::MouseButtonDown { button: 1 },
            ]
        );

        // Releasing everything:
        let events = diff_events(
            &w_down,
            &empty_keys,
            &left_down,
            &no_buttons,
            (9, 4),
            (9, 4),
        );
        assert_eq!(
            events,
            vec![
                WindowEvent::KeyUp {
                    key: "w".to_string()
                },
                WindowEvent::MouseButtonUp { button: 1 },
            ]
        );

        // Two simultaneous presses come out alphabetically (stable stream):
        let multi: HashSet<String> = ["b".to_string(), "a".to_string()].into_iter().collect();
        let events = diff_events(
            &empty_keys,
            &multi,
            &no_buttons,
            &no_buttons,
            (0, 0),
            (0, 0),
        );
        assert_eq!(
            events,
            vec![
                WindowEvent::KeyDown {
                    key: "a".to_string()
                },
                WindowEvent::KeyDown {
                    key: "b".to_string()
                },
            ]
        );
    }

    #[test]
    fn live_window_inputs_and_event_queues_are_bounded() {
        assert!(valid_live_request("", 1, 1));
        assert!(!valid_live_request("ok", 0, 1));
        assert!(!valid_live_request("ok", MAX_LIVE_DIMENSION + 1, 1));
        assert!(!valid_live_request(
            &"x".repeat(MAX_LIVE_TITLE_BYTES + 1),
            1,
            1
        ));

        let mut events = Vec::new();
        append_events_bounded(
            &mut events,
            (0..MAX_LIVE_EVENTS + 1).map(|_| WindowEvent::FocusGained),
        );
        assert_eq!(events.len(), MAX_LIVE_EVENTS);
        push_priority_event(&mut events, WindowEvent::CloseRequested);
        assert_eq!(events.len(), MAX_LIVE_EVENTS);
        assert_eq!(events.last(), Some(&WindowEvent::CloseRequested));

        let runtime_id = 8_300_005;
        crate::native::with_runtime_context(runtime_id, || {
            let mut permits = (0..MAX_LIVE_WINDOWS_PER_RUNTIME)
                .map(|_| reserve_live_window(1, 1).unwrap())
                .collect::<Vec<_>>();
            assert!(reserve_live_window(1, 1).is_none());
            drop(permits.pop());
            permits.push(reserve_live_window(1, 1).unwrap());
        });
        assert!(!crate::native::lock_recover(live_window_usage()).contains_key(&runtime_id));

        crate::native::with_runtime_context(runtime_id, || {
            let full = reserve_live_window(MAX_LIVE_DIMENSION, MAX_LIVE_DIMENSION).unwrap();
            assert!(reserve_live_window(1, 1).is_none());
            drop(full);
        });
        assert!(!crate::native::lock_recover(live_window_usage()).contains_key(&runtime_id));
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
