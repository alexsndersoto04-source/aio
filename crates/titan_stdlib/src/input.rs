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

fn state() -> &'static Arc<Mutex<InputState>> {
    static STATE: OnceLock<Arc<Mutex<InputState>>> = OnceLock::new();
    STATE.get_or_init(|| {
        Arc::new(Mutex::new(InputState {
            keys_down: HashSet::new(),
            mouse_x: 0,
            mouse_y: 0,
            mouse_buttons: HashSet::new(),
            touch_points: HashMap::new(),
        }))
    })
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

    #[test]
    fn test_input_state_tracking() {
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
}
