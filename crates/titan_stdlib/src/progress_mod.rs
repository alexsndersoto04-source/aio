//! Progress bars and spinners (`std::progress::*`) powered by `indicatif`.
//!
//! Real animated progress in the terminal. Nothing simulated. Handles are
//! managed as `i64` IDs in a process-wide registry so `.titan` code can
//! create/update/finish bars by number.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

struct Registry { bars: HashMap<(u64, i64), ProgressBar>, next_id: i64 }

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry { bars: HashMap::new(), next_id: 1 }))
}

fn handle_key(handle: i64) -> (u64, i64) { crate::native::runtime_handle_key(handle) }

fn insert(bar: ProgressBar) -> i64 {
    let mut reg = registry().lock().expect("progress registry poisoned");
    let id = reg.next_id;
    reg.next_id += 1;
    reg.bars.insert(handle_key(id), bar);
    id
}

fn with_bar<F, R>(id: i64, action: F) -> Option<R> where F: FnOnce(&ProgressBar) -> R {
    let reg = registry().lock().ok()?;
    reg.bars.get(&handle_key(id)).map(action)
}

// ---------------- Public API ------------------------------------------

/// Create a determinate progress bar of `total` steps and return its id.
pub fn bar_new(total: u64) -> i64 {
    let bar = ProgressBar::new(total);
    bar.set_style(
        ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> "),
    );
    insert(bar)
}

/// Create an indeterminate spinner and return its id.
pub fn spinner_new() -> i64 {
    let bar = ProgressBar::new_spinner();
    bar.enable_steady_tick(Duration::from_millis(100));
    bar.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
    );
    insert(bar)
}

pub fn set_message(id: i64, message: &str) {
    with_bar(id, |bar| bar.set_message(message.to_string()));
}

pub fn set_position(id: i64, position: u64) {
    with_bar(id, |bar| bar.set_position(position));
}

pub fn increment(id: i64, delta: u64) {
    with_bar(id, |bar| bar.inc(delta));
}

/// Mark the bar as finished (keeps the final line visible) and drop the handle.
pub fn finish(id: i64, message: &str) {
    if let Some(bar) = registry().lock().ok().and_then(|mut r| r.bars.remove(&handle_key(id))) {
        if message.is_empty() {
            bar.finish();
        } else {
            bar.finish_with_message(message.to_string());
        }
    }
}

/// Erase the bar's line and drop the handle (no residue on the terminal).
pub fn abandon(id: i64) {
    if let Some(bar) = registry().lock().ok().and_then(|mut r| r.bars.remove(&handle_key(id))) {
        bar.finish_and_clear();
    }
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let bars = {
        let mut reg = crate::native::lock_recover(registry());
        let keys: Vec<_> = reg.bars.keys()
            .filter(|(owner, _)| *owner == runtime_id)
            .copied()
            .collect();
        keys.into_iter().filter_map(|key| reg.bars.remove(&key)).collect::<Vec<_>>()
    };
    let released = bars.len();
    for bar in bars { bar.finish_and_clear(); }
    released
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_hands_out_unique_ids() {
        let a = bar_new(10);
        let b = bar_new(10);
        assert_ne!(a, b);
        abandon(a);
        abandon(b);
    }

    #[test]
    fn set_and_finish_do_not_panic_on_unknown_id() {
        // Operating on an ID that was never created must be a no-op.
        set_message(999_999, "hi");
        increment(999_999, 1);
        finish(999_999, "done");
        abandon(999_999);
    }

    #[test]
    fn full_life_cycle() {
        let id = bar_new(3);
        set_message(id, "trabajando");
        increment(id, 1);
        set_position(id, 3);
        finish(id, "listo");
        // ID has been removed.
        assert!(with_bar(id, |_| ()).is_none());
    }
}
