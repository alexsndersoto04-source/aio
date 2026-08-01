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
}

// SAFETY: every access is serialized through the registry `Mutex`; only
// one thread ever touches a window at a time, and the TITAN VM runs user
// code on a single thread. The macOS main-run-loop caveat stays
// documented in the module header.
unsafe impl Send for LiveWindow {}

fn registry() -> &'static Mutex<HashMap<u64, LiveWindow>> {
    static REG: OnceLock<Mutex<HashMap<u64, LiveWindow>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT_LIVE_ID: AtomicU64 = AtomicU64::new(1);

/// Mouse buttons bridged to TITAN: 1 = left, 2 = right, 3 = middle,
/// the same convention `std::input` uses since Phase 1.
const BRIDGED_BUTTONS: [(MouseButton, u8); 3] = [
    (MouseButton::Left, 1),
    (MouseButton::Right, 2),
    (MouseButton::Middle, 3),
];

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
        reg.insert(id, LiveWindow {
            window,
            width,
            height,
            scratch: Vec::new(),
            events: Vec::new(),
            keys_down: HashSet::new(),
            buttons_down: HashSet::new(),
            last_mouse: (0, 0),
            close_reported: false,
        });
        id as i64
    } else {
        -1
    }
}

/// Whether the live window is still open (false for unknown handles).
pub fn live_is_open(handle: i64) -> bool {
    registry().lock().ok()
        .and_then(|reg| reg.get(&(handle as u64)).map(|entry| entry.window.is_open()))
        .unwrap_or(false)
}

/// Close and drop the live window. False for unknown handles.
pub fn live_close(handle: i64) -> bool {
    registry().lock()
        .map(|mut reg| reg.remove(&(handle as u64)).is_some())
        .unwrap_or(false)
}

/// Rename the visible OS window title. False for unknown handles.
pub fn live_set_title(handle: i64, title: &str) -> bool {
    registry().lock().ok()
        .and_then(|mut reg| reg.get_mut(&(handle as u64)).map(|entry| entry.window.set_title(title)))
        .is_some()
}

/// One frame of a live window: render the gui tree, present it to the
/// OS framebuffer, pump real keyboard/mouse state into `std::input`,
/// and queue the new events. See the module header for the honest
/// status codes (-2..-6, 0 closed, 1 alive).
pub fn live_pump(handle: i64, gui_root: i64) -> i64 {
    let mut reg = match registry().lock() {
        Ok(guard) => guard,
        Err(_) => return -6,
    };
    let Some(entry) = reg.get_mut(&(handle as u64)) else { return -2 };
    let Some((width, height, rgba)) = render_rgba(gui_root) else { return -3 };
    if width != entry.width || height != entry.height {
        return -4;
    }
    let needed = (width as usize) * (height as usize);
    if entry.scratch.len() != needed {
        entry.scratch.resize(needed, 0);
    }
    pack_rgba(&rgba, &mut entry.scratch);
    if entry.window.update_with_buffer(&entry.scratch, width as usize, height as usize).is_err() {
        return -5;
    }

    // Real input snapshot, taken right after the OS event poll.
    let cur_keys: HashSet<String> = entry.window.get_keys()
        .iter().filter_map(map_key).map(str::to_string).collect();
    let cur_pos = entry.window.get_mouse_pos(MouseMode::Discard)
        .map(|(x, y)| (x as i32, y as i32))
        .unwrap_or(entry.last_mouse);
    let mut cur_buttons = HashSet::new();
    for (mouse_button, titan_button) in BRIDGED_BUTTONS {
        if entry.window.get_mouse_down(mouse_button) {
            cur_buttons.insert(titan_button);
        }
    }

    let events = diff_events(
        &entry.keys_down, &cur_keys,
        &entry.buttons_down, &cur_buttons,
        entry.last_mouse, cur_pos,
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

    entry.events.extend(events);
    entry.keys_down = cur_keys;
    entry.buttons_down = cur_buttons;
    entry.last_mouse = cur_pos;

    if entry.window.is_open() {
        1
    } else {
        if !entry.close_reported {
            entry.events.push(WindowEvent::CloseRequested);
            entry.close_reported = true;
        }
        0
    }
}

/// Drain this window's queued events, formatted exactly like
/// `std::window::poll_events` (empty for unknown handles).
pub fn live_poll_events(handle: i64) -> Vec<String> {
    registry().lock().ok()
        .map(|mut reg| {
            reg.get_mut(&(handle as u64))
                .map(|entry| entry.events.drain(..).map(|event| format_event(&event)).collect::<Vec<_>>())
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
        A => "a", B => "b", C => "c", D => "d", E => "e", F => "f",
        G => "g", H => "h", I => "i", J => "j", K => "k", L => "l",
        M => "m", N => "n", O => "o", P => "p", Q => "q", R => "r",
        S => "s", T => "t", U => "u", V => "v", W => "w", X => "x",
        Y => "y", Z => "z",
        Key0 => "0", Key1 => "1", Key2 => "2", Key3 => "3", Key4 => "4",
        Key5 => "5", Key6 => "6", Key7 => "7", Key8 => "8", Key9 => "9",
        F1 => "f1", F2 => "f2", F3 => "f3", F4 => "f4", F5 => "f5",
        F6 => "f6", F7 => "f7", F8 => "f8", F9 => "f9", F10 => "f10",
        F11 => "f11", F12 => "f12", F13 => "f13", F14 => "f14", F15 => "f15",
        Down => "down", Left => "left", Right => "right", Up => "up",
        Space => "space", Enter => "enter", Escape => "escape",
        Tab => "tab", Backspace => "backspace", Delete => "delete",
        Insert => "insert", Home => "home", End => "end",
        PageUp => "pageup", PageDown => "pagedown", Pause => "pause",
        Menu => "menu",
        CapsLock => "capslock", NumLock => "numlock", ScrollLock => "scrolllock",
        LeftShift | RightShift => "shift",
        LeftCtrl | RightCtrl => "ctrl",
        LeftAlt | RightAlt => "alt",
        LeftSuper | RightSuper => "super",
        Apostrophe => "'", Backquote => "`", Backslash => "\\",
        Comma => ",", Equal => "=", LeftBracket => "[", Minus => "-",
        Period => ".", RightBracket => "]", Semicolon => ";", Slash => "/",
        NumPad0 => "numpad0", NumPad1 => "numpad1", NumPad2 => "numpad2",
        NumPad3 => "numpad3", NumPad4 => "numpad4", NumPad5 => "numpad5",
        NumPad6 => "numpad6", NumPad7 => "numpad7", NumPad8 => "numpad8",
        NumPad9 => "numpad9", NumPadDot => "numpad_dot",
        NumPadSlash => "numpad_slash", NumPadAsterisk => "numpad_asterisk",
        NumPadMinus => "numpad_minus", NumPadPlus => "numpad_plus",
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
    prev_keys: &HashSet<String>, cur_keys: &HashSet<String>,
    prev_buttons: &HashSet<u8>, cur_buttons: &HashSet<u8>,
    prev_pos: (i32, i32), cur_pos: (i32, i32),
) -> Vec<WindowEvent> {
    let mut events = Vec::new();
    if cur_pos != prev_pos {
        events.push(WindowEvent::MouseMove { x: cur_pos.0, y: cur_pos.1 });
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
            255, 0, 0, 7,    // red with junk alpha: alpha must be ignored
            0, 255, 0, 255,  // opaque green
            10, 20, 30, 0,   // fully transparent pixel still keeps its RGB
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
        let events = diff_events(&empty_keys, &w_down, &no_buttons, &left_down, (9, 4), (9, 4));
        assert_eq!(events, vec![
            WindowEvent::KeyDown { key: "w".to_string() },
            WindowEvent::MouseButtonDown { button: 1 },
        ]);

        // Releasing everything:
        let events = diff_events(&w_down, &empty_keys, &left_down, &no_buttons, (9, 4), (9, 4));
        assert_eq!(events, vec![
            WindowEvent::KeyUp { key: "w".to_string() },
            WindowEvent::MouseButtonUp { button: 1 },
        ]);

        // Two simultaneous presses come out alphabetically (stable stream):
        let multi: HashSet<String> = ["b".to_string(), "a".to_string()].into_iter().collect();
        let events = diff_events(&empty_keys, &multi, &no_buttons, &no_buttons, (0, 0), (0, 0));
        assert_eq!(events, vec![
            WindowEvent::KeyDown { key: "a".to_string() },
            WindowEvent::KeyDown { key: "b".to_string() },
        ]);
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
