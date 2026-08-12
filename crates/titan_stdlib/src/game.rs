//! 2D game-loop engine (`std::game`): real frame timing (delta time and
//! measured FPS) plus AABB collision math. Rendering arrives with the
//! window backend; the timing/collision core is complete and headless.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
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

fn game_states() -> &'static Mutex<HashMap<u64, Arc<Mutex<GameState>>>> {
    static STATES: OnceLock<Mutex<HashMap<u64, Arc<Mutex<GameState>>>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_game_state() -> Arc<Mutex<GameState>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut states = crate::native::lock_recover(game_states());
    Arc::clone(
        states
            .entry(runtime_id)
            .or_insert_with(|| Arc::new(Mutex::new(GameState::new()))),
    )
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(
        crate::native::lock_recover(game_states())
            .remove(&runtime_id)
            .is_some(),
    )
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
pub fn check_collision(
    pos1: (f64, f64),
    size1: (f64, f64),
    pos2: (f64, f64),
    size2: (f64, f64),
) -> bool {
    pos1.0 < pos2.0 + size2.0
        && pos1.0 + size1.0 > pos2.0
        && pos1.1 < pos2.1 + size2.1
        && pos1.1 + size1.1 > pos2.1
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
    use std::sync::MutexGuard;

    /// Tests in this module mutate one global GameState while `cargo test`
    /// runs them on parallel threads — serialize them behind one lock so
    /// every test sees a deterministic state machine.
    fn test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn test_game_loop_and_collision() {
        let _guard = test_lock();
        assert!(init("Titan Game", 1024, 768));
        std::thread::sleep(std::time::Duration::from_millis(15));
        let dt = step();
        assert!(dt > 0.0);
        assert!(check_collision(
            (0.0, 0.0),
            (10.0, 10.0),
            (5.0, 5.0),
            (10.0, 10.0)
        ));
        assert!(!check_collision(
            (0.0, 0.0),
            (10.0, 10.0),
            (20.0, 20.0),
            (10.0, 10.0)
        ));
        assert!(shutdown());
    }

    // ---- Timing engine -------------------------------------------------

    #[test]
    fn step_reports_real_positive_delta_and_updates_fps() {
        let _guard = test_lock();
        assert!(init("Timing", 640, 480));
        std::thread::sleep(std::time::Duration::from_millis(5));
        let dt = step();
        // sleep() guarantees strictly positive elapsed time on every OS.
        assert!(dt > 0.0);
        assert!(
            dt < 1.0,
            "dt of a 5ms nap must stay well under a second, got {dt}"
        );
        // FPS is derived from the measured dt: sane bounds for any CI runner.
        let measured = fps();
        assert!(
            measured >= 1 && measured <= 1000,
            "fps out of sane range: {measured}"
        );
        assert!(shutdown());
    }

    #[test]
    fn fps_is_steady_when_sleeping_consistently() {
        let _guard = test_lock();
        assert!(init("Steady", 320, 240));
        std::thread::sleep(std::time::Duration::from_millis(20));
        let first = step();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let second = step();
        assert!(first > 0.0 && second > 0.0);
        // two identical naps cannot differ by an order of magnitude.
        let ratio = if first > second {
            first / second
        } else {
            second / first
        };
        assert!(ratio < 10.0, "dt jitter too wild: {first} vs {second}");
        assert!(shutdown());
    }

    #[test]
    fn step_and_fps_go_silent_after_shutdown() {
        let _guard = test_lock();
        assert!(init("Silent", 800, 600));
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(step() > 0.0);
        assert!(shutdown());
        assert_eq!(step(), 0.0, "a stopped engine must report dt = 0");
        assert_eq!(fps(), 0, "a stopped engine must report 0 fps");
    }

    #[test]
    fn init_resets_frame_accounting_so_second_run_starts_fresh() {
        let _guard = test_lock();
        assert!(init("First run", 800, 600));
        assert!(shutdown());
        assert!(init("Second run", 1920, 1080));
        // After re-init the very next step must not inherit stale frames.
        std::thread::sleep(std::time::Duration::from_millis(3));
        let dt = step();
        assert!(dt > 0.0 && dt < 1.0);
        assert!(shutdown());
    }

    // ---- AABB collision math (pure functions — no global state) --------

    #[test]
    fn collision_corner_overlap_detected() {
        assert!(check_collision(
            (0.0, 0.0),
            (10.0, 10.0),
            (9.0, 9.0),
            (10.0, 10.0)
        ));
    }

    #[test]
    fn collision_touching_edges_do_not_count() {
        // right edge of A exactly on left edge of B: strict inequality means
        // a kiss is not a crash — documented AABB behavior.
        assert!(!check_collision(
            (0.0, 0.0),
            (10.0, 10.0),
            (10.0, 0.0),
            (10.0, 10.0)
        ));
        assert!(!check_collision(
            (0.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (10.0, 10.0)
        ));
    }

    #[test]
    fn collision_horizontal_separation_misses() {
        assert!(!check_collision(
            (0.0, 0.0),
            (10.0, 10.0),
            (50.0, 0.0),
            (10.0, 10.0)
        ));
    }

    #[test]
    fn collision_vertical_separation_misses() {
        assert!(!check_collision(
            (0.0, 0.0),
            (10.0, 10.0),
            (0.0, 50.0),
            (10.0, 10.0)
        ));
    }

    #[test]
    fn collision_full_containment_detected() {
        assert!(check_collision(
            (0.0, 0.0),
            (100.0, 100.0),
            (25.0, 25.0),
            (5.0, 5.0)
        ));
        assert!(check_collision(
            (25.0, 25.0),
            (5.0, 5.0),
            (0.0, 0.0),
            (100.0, 100.0)
        ));
    }

    #[test]
    fn collision_zero_size_point_inside_box_counts() {
        // a 0x0 object at a point strictly inside the box registers a hit —
        // useful for bullets/pixels.
        assert!(check_collision(
            (5.0, 5.0),
            (0.0, 0.0),
            (0.0, 0.0),
            (10.0, 10.0)
        ));
        assert!(!check_collision(
            (50.0, 50.0),
            (0.0, 0.0),
            (0.0, 0.0),
            (10.0, 10.0)
        ));
    }

    #[test]
    fn collision_with_negative_coordinates() {
        assert!(check_collision(
            (-20.0, -20.0),
            (10.0, 10.0),
            (-15.0, -15.0),
            (10.0, 10.0)
        ));
        assert!(!check_collision(
            (-100.0, -100.0),
            (10.0, 10.0),
            (0.0, 0.0),
            (10.0, 10.0)
        ));
    }

    #[test]
    fn collision_is_symmetric_in_argument_order() {
        let a = ((0.0, 0.0), (10.0, 10.0));
        let b = ((6.0, 6.0), (10.0, 10.0));
        assert_eq!(
            check_collision(a.0, a.1, b.0, b.1),
            check_collision(b.0, b.1, a.0, a.1),
        );
    }

    #[test]
    fn collision_fractional_subpixel_overlap() {
        assert!(check_collision(
            (0.0, 0.0),
            (10.0, 10.0),
            (9.999, 9.999),
            (10.0, 10.0)
        ));
        assert!(!check_collision(
            (0.0, 0.0),
            (10.0, 10.0),
            (10.001, 10.001),
            (10.0, 10.0)
        ));
    }
}
