//! Native Mobile & Android Lifecycle Management (`std::mobile`).
//! Manages app states (`Running`, `Paused`, `Stopped`) and lifecycle events (`onStart`, `onResume`, `onPause`, `onStop`, `onDestroy`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

const MAX_MOBILE_EVENTS: usize = 1_024;
const MAX_MOBILE_EVENT_BYTES: usize = 256;

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

fn service_registry() -> &'static Mutex<HashMap<u64, Arc<Mutex<MobileService>>>> {
    static SERVICES: OnceLock<Mutex<HashMap<u64, Arc<Mutex<MobileService>>>>> = OnceLock::new();
    SERVICES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn service() -> Arc<Mutex<MobileService>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(service_registry());
    Arc::clone(registry.entry(runtime_id).or_insert_with(|| {
        Arc::new(Mutex::new(MobileService {
            state: AppState::Running,
            event_history: Vec::new(),
        }))
    }))
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(
        crate::native::lock_recover(service_registry())
            .remove(&runtime_id)
            .is_some(),
    )
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
    if event.is_empty() || event.len() > MAX_MOBILE_EVENT_BYTES {
        return false;
    }
    if let Ok(mut srv) = service().lock() {
        if srv.event_history.len() >= MAX_MOBILE_EVENTS {
            return false;
        }
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
    use std::sync::MutexGuard;

    /// Single global service + parallel test threads = serialize, and drain
    /// any leftover history so each test starts from a clean queue.
    fn test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn test_mobile_lifecycle_transitions() {
        let _guard = test_lock();
        let _ = poll_events(); // hygienic drain
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

    #[test]
    fn full_android_chain_walks_every_state() {
        let _guard = test_lock();
        let _ = poll_events();
        assert!(trigger_event("onStart"));
        assert_eq!(get_state(), "Running");
        assert!(trigger_event("onPause"));
        assert_eq!(get_state(), "Paused");
        assert!(trigger_event("onStop"));
        assert_eq!(get_state(), "Stopped");
        assert!(trigger_event("onResume"));
        assert_eq!(
            get_state(),
            "Running",
            "resume from stopped, like tapping the icon again"
        );
    }

    #[test]
    fn resume_after_destroy_recreates_the_app() {
        let _guard = test_lock();
        let _ = poll_events();
        assert!(trigger_event("onDestroy"));
        assert_eq!(get_state(), "Destroyed");
        // Android DOES recreate activities after destroy (user reopens app).
        assert!(trigger_event("onResume"));
        assert_eq!(get_state(), "Running");
    }

    #[test]
    fn unknown_events_are_recorded_but_do_not_move_the_state() {
        let _guard = test_lock();
        let _ = poll_events();
        assert!(trigger_event("onPause")); // known anchor state
        let before = get_state();
        assert!(
            trigger_event("onWiggle"),
            "extensibility: accept, don't crash"
        );
        assert!(trigger_event("onLowMemory"));
        assert_eq!(
            get_state(),
            before,
            "unknown events must not mutate the state machine"
        );
        let events = poll_events();
        assert_eq!(
            events,
            vec![
                "onPause".to_string(),
                "onWiggle".to_string(),
                "onLowMemory".to_string()
            ]
        );
    }

    #[test]
    fn poll_events_drains_the_queue() {
        let _guard = test_lock();
        let _ = poll_events();
        assert!(trigger_event("onStart"));
        assert!(trigger_event("onPause"));
        let first = poll_events();
        assert_eq!(first.len(), 2);
        let second = poll_events();
        assert!(second.is_empty(), "a drained poll must come back empty");
    }

    #[test]
    fn poll_events_preserves_exact_arrival_order() {
        let _guard = test_lock();
        let _ = poll_events();
        for e in ["onStart", "onPause", "onStop", "onResume", "onDestroy"] {
            assert!(trigger_event(e));
        }
        let events = poll_events();
        assert_eq!(
            events,
            vec![
                "onStart".to_string(),
                "onPause".to_string(),
                "onStop".to_string(),
                "onResume".to_string(),
                "onDestroy".to_string(),
            ],
        );
    }

    #[test]
    fn state_names_match_android_vocabulary() {
        let _guard = test_lock();
        let _ = poll_events();
        assert!(trigger_event("onStart"));
        assert_eq!(get_state(), "Running");
        assert!(trigger_event("onPause"));
        assert_eq!(get_state(), "Paused");
        assert!(trigger_event("onStop"));
        assert_eq!(get_state(), "Stopped");
        assert!(trigger_event("onDestroy"));
        assert_eq!(get_state(), "Destroyed");
    }
    #[test]
    fn lifecycle_event_queue_is_bounded_and_draining_recovers_it() {
        let runtime_id = 85_005;
        crate::native::with_runtime_context(runtime_id, || {
            for _ in 0..MAX_MOBILE_EVENTS {
                assert!(trigger_event("onPause"));
            }
            assert!(!trigger_event("onPause"));
            assert!(!trigger_event(&"x".repeat(MAX_MOBILE_EVENT_BYTES + 1)));
            assert_eq!(poll_events().len(), MAX_MOBILE_EVENTS);
            assert!(trigger_event("onResume"));
            assert_eq!(poll_events(), vec!["onResume".to_string()]);
        });
        assert_eq!(cleanup_runtime(runtime_id), 1);
    }
}
