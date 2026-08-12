//! Cross-platform Windowing, GUI, and Input Management (`std::window`).
//! Manages native window configurations, event queues, keyboard/mouse input, and lifecycle.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, atomic::{AtomicU64, Ordering}};

static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

fn registry() -> &'static Arc<Mutex<HashMap<(u64, u64), WindowHandle>>> {
    static REGISTRY: OnceLock<Arc<Mutex<HashMap<(u64, u64), WindowHandle>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

fn handle_key(handle: u64) -> (u64, u64) { crate::native::runtime_handle_key(handle) }

#[derive(Debug, Clone, PartialEq)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub fullscreen: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "TITAN App".to_string(),
            width: 800,
            height: 600,
            resizable: true,
            fullscreen: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowEvent {
    CloseRequested,
    Resized { width: u32, height: u32 },
    FocusGained,
    FocusLost,
    KeyDown { key: String },
    KeyUp { key: String },
    MouseMove { x: i32, y: i32 },
    MouseButtonDown { button: u8 },
    MouseButtonUp { button: u8 },
}

#[derive(Debug, Clone)]
pub struct WindowHandle {
    pub id: u64,
    pub config: WindowConfig,
    pub is_open: bool,
    pub events: Vec<WindowEvent>,
}

pub fn create(title: &str, width: u32, height: u32) -> u64 {
    let id = NEXT_WINDOW_ID.fetch_add(1, Ordering::SeqCst);
    let config = WindowConfig {
        title: title.to_string(),
        width,
        height,
        ..Default::default()
    };
    let handle = WindowHandle {
        id,
        config,
        is_open: true,
        events: Vec::new(),
    };
    if let Ok(mut reg) = registry().lock() {
        reg.insert(handle_key(id), handle);
    }
    id
}

pub fn is_open(id: u64) -> bool {
    if let Ok(reg) = registry().lock() {
        if let Some(win) = reg.get(&handle_key(id)) {
            return win.is_open;
        }
    }
    false
}

pub fn close(id: u64) -> bool {
    if let Ok(mut reg) = registry().lock() {
        if let Some(win) = reg.get_mut(&handle_key(id)) {
            if win.is_open {
                win.is_open = false;
                win.events.push(WindowEvent::CloseRequested);
                return true;
            }
        }
    }
    false
}

pub fn set_title(id: u64, title: &str) -> bool {
    if let Ok(mut reg) = registry().lock() {
        if let Some(win) = reg.get_mut(&handle_key(id)) {
            win.config.title = title.to_string();
            return true;
        }
    }
    false
}

pub fn resize(id: u64, width: u32, height: u32) -> bool {
    if let Ok(mut reg) = registry().lock() {
        if let Some(win) = reg.get_mut(&handle_key(id)) {
            win.config.width = width;
            win.config.height = height;
            win.events.push(WindowEvent::Resized { width, height });
            return true;
        }
    }
    false
}

pub fn push_event(id: u64, event: WindowEvent) -> bool {
    if let Ok(mut reg) = registry().lock() {
        if let Some(win) = reg.get_mut(&handle_key(id)) {
            if win.is_open {
                win.events.push(event);
                return true;
            }
        }
    }
    false
}

pub fn poll_events(id: u64) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(mut reg) = registry().lock() {
        if let Some(win) = reg.get_mut(&handle_key(id)) {
            for ev in win.events.drain(..) {
                out.push(format_event(&ev));
            }
        }
    }
    out
}

/// Format one window event exactly as `poll_events` reports it. Shared
/// with the live-window backend (Fase 2) so a TITAN program cannot tell
/// which backend produced an event — one event language, two engines.
pub fn format_event(ev: &WindowEvent) -> String {
    match ev {
        WindowEvent::CloseRequested => "CloseRequested".to_string(),
        WindowEvent::Resized { width, height } => format!("Resized({width}, {height})"),
        WindowEvent::FocusGained => "FocusGained".to_string(),
        WindowEvent::FocusLost => "FocusLost".to_string(),
        WindowEvent::KeyDown { key } => format!("KeyDown({key})"),
        WindowEvent::KeyUp { key } => format!("KeyUp({key})"),
        WindowEvent::MouseMove { x, y } => format!("MouseMove({x}, {y})"),
        WindowEvent::MouseButtonDown { button } => format!("MouseButtonDown({button})"),
        WindowEvent::MouseButtonUp { button } => format!("MouseButtonUp({button})"),
    }
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let mut reg = crate::native::lock_recover(registry());
    crate::native::remove_runtime_entries(&mut reg, runtime_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_lifecycle_and_events() {
        let win_id = create("Test Window", 1024, 768);
        assert!(is_open(win_id));

        assert!(set_title(win_id, "Updated Title"));
        assert!(resize(win_id, 1280, 720));

        assert!(push_event(win_id, WindowEvent::KeyDown { key: "Escape".to_string() }));
        assert!(push_event(win_id, WindowEvent::MouseMove { x: 100, y: 200 }));

        let events = poll_events(win_id);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], "Resized(1280, 720)");
        assert_eq!(events[1], "KeyDown(Escape)");
        assert_eq!(events[2], "MouseMove(100, 200)");

        assert!(close(win_id));
        assert!(!is_open(win_id));
    }
}
