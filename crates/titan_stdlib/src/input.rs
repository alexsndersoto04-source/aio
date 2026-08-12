//! Hardware Input State Management (`std::input`).
//! Tracks real-time keyboard state, mouse coordinates, mouse buttons, and multi-touch points for games and GUI.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

struct InputState {
    keys_down: HashSet<String>,
    mouse_x: i32,
    mouse_y: i32,
    mouse_buttons: HashSet<u8>,
    touch_points: HashMap<u32, (i32, i32)>,
}

fn states() -> &'static Mutex<HashMap<u64, Arc<Mutex<InputState>>>> {
    static STATES: OnceLock<Mutex<HashMap<u64, Arc<Mutex<InputState>>>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn state() -> Arc<Mutex<InputState>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut states = crate::native::lock_recover(states());
    Arc::clone(states.entry(runtime_id).or_insert_with(|| {
        Arc::new(Mutex::new(InputState {
            keys_down: HashSet::new(),
            mouse_x: 0,
            mouse_y: 0,
            mouse_buttons: HashSet::new(),
            touch_points: HashMap::new(),
        }))
    }))
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(crate::native::lock_recover(states()).remove(&runtime_id).is_some())
}

pub fn set_key_state(key: &str, pressed: bool) -> bool {
    if let Ok(mut st) = state().lock() {
        if pressed {
            st.keys_down.insert(key.to_string());
        } else {
            st.keys_down.remove(key);
        }
        return true;
    }
    false
}

pub fn is_key_pressed(key: &str) -> bool {
    if let Ok(st) = state().lock() {
        return st.keys_down.contains(key);
    }
    false
}

pub fn set_mouse_pos(x: i32, y: i32) -> bool {
    if let Ok(mut st) = state().lock() {
        st.mouse_x = x;
        st.mouse_y = y;
        return true;
    }
    false
}

pub fn mouse_pos() -> (i32, i32) {
    if let Ok(st) = state().lock() {
        return (st.mouse_x, st.mouse_y);
    }
    (0, 0)
}

pub fn set_mouse_button(button: u8, pressed: bool) -> bool {
    if let Ok(mut st) = state().lock() {
        if pressed {
            st.mouse_buttons.insert(button);
        } else {
            st.mouse_buttons.remove(&button);
        }
        return true;
    }
    false
}

pub fn is_mouse_button_pressed(button: u8) -> bool {
    if let Ok(st) = state().lock() {
        return st.mouse_buttons.contains(&button);
    }
    false
}

pub fn set_touch_point(index: u32, x: i32, y: i32, active: bool) -> bool {
    if let Ok(mut st) = state().lock() {
        if active {
            st.touch_points.insert(index, (x, y));
        } else {
            st.touch_points.remove(&index);
        }
        return true;
    }
    false
}

pub fn touch_pos(index: u32) -> (i32, i32, bool) {
    if let Ok(st) = state().lock() {
        if let Some(&(x, y)) = st.touch_points.get(&index) {
            return (x, y, true);
        }
    }
    (0, 0, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::MutexGuard;

    /// One global InputState + parallel test threads = serialize every test
    /// behind this lock. Keys/buttons/touch indexes also stay unique per
    /// test so even a poisoned lock cannot create cross-test interference.
    fn test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn test_input_state_tracking() {
        let _guard = test_lock();
        assert!(set_key_state("Space", true));
        assert!(is_key_pressed("Space"));
        assert!(!is_key_pressed("Enter"));
        assert!(set_key_state("Space", false));
        assert!(!is_key_pressed("Space"));

        assert!(set_mouse_pos(450, 320));
        assert_eq!(mouse_pos(), (450, 320));

        assert!(set_mouse_button(0, true));
        assert!(is_mouse_button_pressed(0));
        assert!(!is_mouse_button_pressed(1));

        assert!(set_touch_point(1, 120, 240, true));
        assert_eq!(touch_pos(1), (120, 240, true));
        assert_eq!(touch_pos(99), (0, 0, false));
    }

    // ---- Keyboard ------------------------------------------------------

    #[test]
    fn key_press_release_cycle_is_exact() {
        let _guard = test_lock();
        assert!(!is_key_pressed("F1_A"));
        assert!(set_key_state("F1_A", true));
        assert!(is_key_pressed("F1_A"));
        // pressing twice is idempotent — still pressed, still true.
        assert!(set_key_state("F1_A", true));
        assert!(is_key_pressed("F1_A"));
        assert!(set_key_state("F1_A", false));
        assert!(!is_key_pressed("F1_A"));
        // releasing an already-released key is a valid no-op.
        assert!(set_key_state("F1_A", false));
        assert!(!is_key_pressed("F1_A"));
    }

    #[test]
    fn multiple_keys_stay_down_independently() {
        let _guard = test_lock();
        assert!(set_key_state("F2_A", true));
        assert!(set_key_state("F2_B", true));
        assert!(set_key_state("F2_C", true));
        assert!(set_key_state("F2_B", false));
        assert!(is_key_pressed("F2_A"));
        assert!(!is_key_pressed("F2_B"));
        assert!(is_key_pressed("F2_C"));
        assert!(set_key_state("F2_A", false));
        assert!(set_key_state("F2_C", false));
    }

    #[test]
    fn never_touched_key_reports_not_pressed() {
        let _guard = test_lock();
        assert!(!is_key_pressed("NO_SUCH_KEY_XYZ_42"));
    }

    #[test]
    fn key_names_are_case_sensitive_and_unicode_safe() {
        let _guard = test_lock();
        assert!(set_key_state("F3_Lower", true));
        assert!(is_key_pressed("F3_Lower"));
        assert!(
            !is_key_pressed("f3_lower"),
            "case must matter, like real keyboard layouts"
        );
        assert!(set_key_state("Ñandú", true));
        assert!(is_key_pressed("Ñandú"));
        assert!(set_key_state("F3_Lower", false));
        assert!(set_key_state("Ñandú", false));
    }

    // ---- Mouse ----------------------------------------------------------

    #[test]
    fn mouse_tracks_negative_and_overwritten_positions() {
        let _guard = test_lock();
        assert!(set_mouse_pos(-15, -42));
        assert_eq!(mouse_pos(), (-15, -42));
        assert!(set_mouse_pos(640, 480));
        assert_eq!(mouse_pos(), (640, 480), "latest write wins");
        assert!(set_mouse_pos(0, 0));
    }

    #[test]
    fn mouse_buttons_track_independently() {
        let _guard = test_lock();
        assert!(set_mouse_button(7, true));
        assert!(set_mouse_button(8, true));
        assert!(set_mouse_button(7, false));
        assert!(!is_mouse_button_pressed(7));
        assert!(is_mouse_button_pressed(8));
        assert!(!is_mouse_button_pressed(9), "never touched");
        assert!(set_mouse_button(8, false));
    }

    // ---- Multi-touch ----------------------------------------------------

    #[test]
    fn touch_tracks_multiple_points_at_once() {
        let _guard = test_lock();
        assert!(set_touch_point(81, 100, 200, true));
        assert!(set_touch_point(82, 300, 400, true));
        assert_eq!(touch_pos(81), (100, 200, true));
        assert_eq!(touch_pos(82), (300, 400, true));
        assert!(set_touch_point(81, 0, 0, false));
        assert!(set_touch_point(82, 0, 0, false));
    }

    #[test]
    fn touch_updates_coordinates_while_staying_active() {
        let _guard = test_lock();
        assert!(set_touch_point(83, 10, 10, true));
        assert!(set_touch_point(83, 55, 77, true));
        assert_eq!(touch_pos(83), (55, 77, true), "dragging updates in place");
        assert!(set_touch_point(83, 0, 0, false));
        assert_eq!(touch_pos(83), (0, 0, false));
    }

    #[test]
    fn touch_index_zero_and_extreme_indexes_work() {
        let _guard = test_lock();
        assert!(set_touch_point(0, 1, 2, true));
        assert_eq!(touch_pos(0), (1, 2, true));
        assert!(set_touch_point(0, 0, 0, false));
        assert_eq!(
            touch_pos(u32::MAX),
            (0, 0, false),
            "untouched extreme index"
        );
    }

    // ---- Cross-domain isolation -----------------------------------------

    #[test]
    fn keys_buttons_and_touch_do_not_leak_into_each_other() {
        let _guard = test_lock();
        assert!(set_key_state("F4_KEY", true));
        assert!(set_mouse_button(251, true));
        assert!(set_touch_point(84, 9, 9, true));
        assert!(!is_mouse_button_pressed(252));
        assert!(!is_key_pressed("F4_BUTTON"));
        assert_eq!(touch_pos(85), (0, 0, false));
        // cleanup own footprint
        assert!(set_key_state("F4_KEY", false));
        assert!(set_mouse_button(251, false));
        assert!(set_touch_point(84, 0, 0, false));
    }
}
