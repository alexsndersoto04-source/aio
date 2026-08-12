//! Cross-platform Windowing, GUI, and Input Management (`std::window`).
//! Manages native window configurations, event queues, keyboard/mouse input, and lifecycle.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, OnceLock,
};

const MAX_WINDOW_HANDLES: usize = 64;
const MAX_WINDOW_EVENTS: usize = 1_024;
const MAX_WINDOW_TITLE_BYTES: usize = 64 * 1024;
const MAX_WINDOW_KEY_BYTES: usize = 256;
const MAX_WINDOW_DIMENSION: u32 = 16_384;

static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

fn registry() -> &'static Arc<Mutex<HashMap<(u64, u64), WindowHandle>>> {
    static REGISTRY: OnceLock<Arc<Mutex<HashMap<(u64, u64), WindowHandle>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

fn handle_key(handle: u64) -> (u64, u64) {
    crate::native::runtime_handle_key(handle)
}

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

fn validate_dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 || width > MAX_WINDOW_DIMENSION || height > MAX_WINDOW_DIMENSION {
        return Err(format!(
            "window dimensions must be between 1 and {MAX_WINDOW_DIMENSION}"
        ));
    }
    Ok(())
}

fn validate_title(title: &str) -> Result<(), String> {
    if title.len() > MAX_WINDOW_TITLE_BYTES {
        return Err(format!(
            "window title exceeds byte limit {MAX_WINDOW_TITLE_BYTES}"
        ));
    }
    Ok(())
}

fn validate_event(event: &WindowEvent) -> bool {
    match event {
        WindowEvent::KeyDown { key } | WindowEvent::KeyUp { key } => {
            key.len() <= MAX_WINDOW_KEY_BYTES
        }
        WindowEvent::Resized { width, height } => validate_dimensions(*width, *height).is_ok(),
        _ => true,
    }
}

pub fn create(title: &str, width: u32, height: u32) -> Result<u64, String> {
    validate_title(title)?;
    validate_dimensions(width, height)?;
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(registry());
    let active = registry
        .keys()
        .filter(|(owner, _)| *owner == runtime_id)
        .count();
    if active >= MAX_WINDOW_HANDLES {
        return Err(format!(
            "window handle quota exceeded (limit {MAX_WINDOW_HANDLES})"
        ));
    }
    let id = NEXT_WINDOW_ID
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| {
            (id <= i64::MAX as u64).then(|| id + 1)
        })
        .map_err(|_| "window handle space exhausted".to_string())?;
    let config = WindowConfig {
        title: title.to_string(),
        width,
        height,
        ..Default::default()
    };
    registry.insert(
        (runtime_id, id),
        WindowHandle {
            id,
            config,
            is_open: true,
            events: Vec::new(),
        },
    );
    Ok(id)
}

pub fn is_open(id: u64) -> bool {
    crate::native::lock_recover(registry())
        .get(&handle_key(id))
        .is_some_and(|window| window.is_open)
}

pub fn close(id: u64) -> bool {
    crate::native::lock_recover(registry())
        .remove(&handle_key(id))
        .is_some_and(|window| window.is_open)
}

pub fn set_title(id: u64, title: &str) -> bool {
    if validate_title(title).is_err() {
        return false;
    }
    let mut registry = crate::native::lock_recover(registry());
    if let Some(window) = registry.get_mut(&handle_key(id)) {
        window.config.title = title.to_string();
        true
    } else {
        false
    }
}

pub fn resize(id: u64, width: u32, height: u32) -> bool {
    if validate_dimensions(width, height).is_err() {
        return false;
    }
    let mut registry = crate::native::lock_recover(registry());
    if let Some(window) = registry.get_mut(&handle_key(id)) {
        if !window.is_open || window.events.len() >= MAX_WINDOW_EVENTS {
            return false;
        }
        window.config.width = width;
        window.config.height = height;
        window.events.push(WindowEvent::Resized { width, height });
        true
    } else {
        false
    }
}

pub fn push_event(id: u64, event: WindowEvent) -> bool {
    if !validate_event(&event) {
        return false;
    }
    let mut registry = crate::native::lock_recover(registry());
    if let Some(window) = registry.get_mut(&handle_key(id)) {
        if window.is_open && window.events.len() < MAX_WINDOW_EVENTS {
            window.events.push(event);
            return true;
        }
    }
    false
}

pub fn poll_events(id: u64) -> Vec<String> {
    let mut registry = crate::native::lock_recover(registry());
    registry
        .get_mut(&handle_key(id))
        .map(|window| {
            window
                .events
                .drain(..)
                .map(|event| format_event(&event))
                .collect()
        })
        .unwrap_or_default()
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
        let win_id = create("Test Window", 1024, 768).unwrap();
        assert!(is_open(win_id));

        assert!(set_title(win_id, "Updated Title"));
        assert!(resize(win_id, 1280, 720));

        assert!(push_event(
            win_id,
            WindowEvent::KeyDown {
                key: "Escape".to_string()
            }
        ));
        assert!(push_event(
            win_id,
            WindowEvent::MouseMove { x: 100, y: 200 }
        ));

        let events = poll_events(win_id);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], "Resized(1280, 720)");
        assert_eq!(events[1], "KeyDown(Escape)");
        assert_eq!(events[2], "MouseMove(100, 200)");

        assert!(close(win_id));
        assert!(!is_open(win_id));
    }

    #[test]
    fn handles_titles_dimensions_and_event_queues_are_bounded() {
        let runtime_id = 8_300_003;
        crate::native::with_runtime_context(runtime_id, || {
            assert!(create("oversized", 0, 1).is_err());
            assert!(create(&"x".repeat(MAX_WINDOW_TITLE_BYTES + 1), 1, 1).is_err());
            let mut handles = (0..MAX_WINDOW_HANDLES)
                .map(|_| create("bounded", 1, 1).unwrap())
                .collect::<Vec<_>>();
            assert!(create("overflow", 1, 1)
                .unwrap_err()
                .contains("handle quota"));
            assert!(!set_title(
                handles[0],
                &"x".repeat(MAX_WINDOW_TITLE_BYTES + 1)
            ));
            for index in 0..MAX_WINDOW_EVENTS {
                assert!(push_event(
                    handles[0],
                    WindowEvent::MouseMove {
                        x: index as i32,
                        y: 0,
                    }
                ));
            }
            assert!(!push_event(handles[0], WindowEvent::FocusGained));
            assert_eq!(poll_events(handles[0]).len(), MAX_WINDOW_EVENTS);
            assert!(push_event(handles[0], WindowEvent::FocusGained));
            assert!(close(handles.pop().unwrap()));
            handles.push(create("recovered", 1, 1).unwrap());
        });
        assert_eq!(cleanup_runtime(runtime_id), MAX_WINDOW_HANDLES);
    }
}
