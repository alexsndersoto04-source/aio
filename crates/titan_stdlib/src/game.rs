use std::sync::{Mutex, OnceLock};
use std::time::Instant;

struct GameState {
    initialized: bool,
    title: String,
    width: i64,
    height: i64,
    last_frame: Option<Instant>,
    frame_count: u64,
    fps: i64,
}

impl GameState {
    fn new() -> Self {
        Self {
            initialized: false,
            title: String::new(),
            width: 800,
            height: 600,
            last_frame: None,
            frame_count: 0,
            fps: 60,
        }
    }
}

static GAME_STATE: OnceLock<Mutex<GameState>> = OnceLock::new();

fn get_game_state() -> &'static Mutex<GameState> {
    GAME_STATE.get_or_init(|| Mutex::new(GameState::new()))
}

pub fn init(title: &str, width: i64, height: i64) -> bool {
    if let Ok(mut state) = get_game_state().lock() {
        state.title = title.to_string();
        state.width = width;
        state.height = height;
        state.last_frame = Some(Instant::now());
        state.frame_count = 0;
        state.initialized = true;
        true
    } else {
        false
    }
}

pub fn step() -> f64 {
    if let Ok(mut state) = get_game_state().lock() {
        if !state.initialized {
            return 0.0;
        }
        let now = Instant::now();
        let delta = if let Some(last) = state.last_frame {
            let dt = now.duration_since(last).as_secs_f64();
            if dt > 0.0 {
                state.fps = (1.0 / dt).round() as i64;
            }
            dt
        } else {
            0.016_666_666_666_666_666
        };
        state.last_frame = Some(now);
        state.frame_count = state.frame_count.saturating_add(1);
        delta
    } else {
        0.0
    }
}

pub fn fps() -> i64 {
    if let Ok(state) = get_game_state().lock() {
        if state.initialized {
            state.fps
        } else {
            0
        }
    } else {
        0
    }
}

/// Detects 2D Axis-Aligned Bounding Box (AABB) collision between two objects.
/// `pos1` and `pos2` are (x, y), while `size1` and `size2` are (width, height).
pub fn check_collision(pos1: (f64, f64), size1: (f64, f64), pos2: (f64, f64), size2: (f64, f64)) -> bool {
    pos1.0 < pos2.0 + size2.0 && pos1.0 + size1.0 > pos2.0 && pos1.1 < pos2.1 + size2.1 && pos1.1 + size1.1 > pos2.1
}

pub fn shutdown() -> bool {
    if let Ok(mut state) = get_game_state().lock() {
        state.initialized = false;
        state.last_frame = None;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_loop_and_collision() {
        assert!(init("Titan Game", 1024, 768));
        std::thread::sleep(std::time::Duration::from_millis(15));
        let dt = step();
        assert!(dt > 0.0);
        assert!(check_collision((0.0, 0.0), (10.0, 10.0), (5.0, 5.0), (10.0, 10.0)));
        assert!(!check_collision((0.0, 0.0), (10.0, 10.0), (20.0, 20.0), (10.0, 10.0)));
        assert!(shutdown());
    }
}
