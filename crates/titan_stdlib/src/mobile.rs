//! Native Mobile & Android Lifecycle Management (`std::mobile`).
//! Manages app states (`Running`, `Paused`, `Stopped`) and lifecycle events (`onStart`, `onResume`, `onPause`, `onStop`, `onDestroy`).

use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Running,
    Paused,
    Stopped,
    Destroyed,
}

struct MobileService {
    state: AppState,
    event_history: Vec<String>,
}

fn service() -> &'static Arc<Mutex<MobileService>> {
    static SERVICE: OnceLock<Arc<Mutex<MobileService>>> = OnceLock::new();
    SERVICE.get_or_init(|| {
        Arc::new(Mutex::new(MobileService {
            state: AppState::Running,
            event_history: Vec::new(),
        }))
    })
}

pub fn get_state() -> String {
    if let Ok(srv) = service().lock() {
        return match srv.state {
            AppState::Running => "Running".to_string(),
            AppState::Paused => "Paused".to_string(),
            AppState::Stopped => "Stopped".to_string(),
            AppState::Destroyed => "Destroyed".to_string(),
        };
    }
    "Unknown".to_string()
}

pub fn trigger_event(event: &str) -> bool {
    if let Ok(mut srv) = service().lock() {
        srv.event_history.push(event.to_string());
        match event {
            "onStart" | "onResume" => srv.state = AppState::Running,
            "onPause" => srv.state = AppState::Paused,
            "onStop" => srv.state = AppState::Stopped,
            "onDestroy" => srv.state = AppState::Destroyed,
            _ => {}
        }
        return true;
    }
    false
}

pub fn poll_events() -> Vec<String> {
    if let Ok(mut srv) = service().lock() {
        return srv.event_history.drain(..).collect();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mobile_lifecycle_transitions() {
        assert!(trigger_event("onPause"));
        assert_eq!(get_state(), "Paused");
        assert!(trigger_event("onResume"));
        assert_eq!(get_state(), "Running");
        assert!(trigger_event("onDestroy"));
        assert_eq!(get_state(), "Destroyed");

        let events = poll_events();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], "onPause");
        assert_eq!(events[2], "onDestroy");
    }
}
