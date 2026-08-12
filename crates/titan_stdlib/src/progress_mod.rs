//! Progress bars and spinners (`std::progress::*`) powered by `indicatif`.
//!
//! Real animated progress in the terminal. Nothing simulated. Handles are
//! managed as `i64` IDs in a process-wide registry so `.titan` code can
//! create/update/finish bars by number.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

const MAX_PROGRESS_HANDLES: usize = 64;
const MAX_PROGRESS_MESSAGE_BYTES: usize = 4 * 1024;

struct Registry {
    bars: HashMap<(u64, i64), ProgressBar>,
    next_id: i64,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        Mutex::new(Registry {
            bars: HashMap::new(),
            next_id: 1,
        })
    })
}

fn handle_key(handle: i64) -> (u64, i64) {
    crate::native::runtime_handle_key(handle)
}

fn insert(bar: ProgressBar) -> Result<i64, String> {
    let runtime_id = crate::native::current_runtime_id();
    let mut reg = crate::native::lock_recover(registry());
    let active = reg
        .bars
        .keys()
        .filter(|(owner, _)| *owner == runtime_id)
        .count();
    if active >= MAX_PROGRESS_HANDLES {
        return Err(format!(
            "progress handle quota exceeded (limit {MAX_PROGRESS_HANDLES})"
        ));
    }
    let id = reg.next_id;
    reg.next_id = id
        .checked_add(1)
        .ok_or_else(|| "progress handle space exhausted".to_string())?;
    reg.bars.insert((runtime_id, id), bar);
    Ok(id)
}

fn validate_message(message: &str) -> Result<(), String> {
    if message.len() > MAX_PROGRESS_MESSAGE_BYTES {
        return Err(format!(
            "progress message exceeds byte limit {MAX_PROGRESS_MESSAGE_BYTES}"
        ));
    }
    Ok(())
}

fn with_bar<F, R>(id: i64, action: F) -> Option<R>
where
    F: FnOnce(&ProgressBar) -> R,
{
    let bar = crate::native::lock_recover(registry())
        .bars
        .get(&handle_key(id))
        .cloned();
    bar.as_ref().map(action)
}

// ---------------- Public API ------------------------------------------

/// Create a determinate progress bar of `total` steps and return its id.
pub fn bar_new(total: u64) -> Result<i64, String> {
    let bar = ProgressBar::new(total);
    bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap()
        .progress_chars("=> "),
    );
    insert(bar)
}

/// Create an indeterminate spinner and return its id.
pub fn spinner_new() -> Result<i64, String> {
    let bar = ProgressBar::new_spinner();
    bar.enable_steady_tick(Duration::from_millis(100));
    bar.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
    );
    insert(bar)
}

pub fn set_message(id: i64, message: &str) -> Result<(), String> {
    validate_message(message)?;
    with_bar(id, |bar| bar.set_message(message.to_string()));
    Ok(())
}

pub fn set_position(id: i64, position: u64) {
    with_bar(id, |bar| bar.set_position(position));
}

pub fn increment(id: i64, delta: u64) {
    with_bar(id, |bar| bar.inc(delta));
}

/// Mark the bar as finished (keeps the final line visible) and drop the handle.
pub fn finish(id: i64, message: &str) -> Result<(), String> {
    validate_message(message)?;
    let bar = crate::native::lock_recover(registry())
        .bars
        .remove(&handle_key(id));
    if let Some(bar) = bar {
        if message.is_empty() {
            bar.finish();
        } else {
            bar.finish_with_message(message.to_string());
        }
    }
    Ok(())
}

/// Erase the bar's line and drop the handle (no residue on the terminal).
pub fn abandon(id: i64) {
    let bar = crate::native::lock_recover(registry())
        .bars
        .remove(&handle_key(id));
    if let Some(bar) = bar {
        bar.finish_and_clear();
    }
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    let bars = {
        let mut reg = crate::native::lock_recover(registry());
        let keys: Vec<_> = reg
            .bars
            .keys()
            .filter(|(owner, _)| *owner == runtime_id)
            .copied()
            .collect();
        keys.into_iter()
            .filter_map(|key| reg.bars.remove(&key))
            .collect::<Vec<_>>()
    };
    let released = bars.len();
    for bar in bars {
        bar.finish_and_clear();
    }
    released
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_hands_out_unique_ids() {
        let a = bar_new(10).unwrap();
        let b = bar_new(10).unwrap();
        assert_ne!(a, b);
        abandon(a);
        abandon(b);
    }

    #[test]
    fn set_and_finish_do_not_panic_on_unknown_id() {
        // Operating on an ID that was never created must be a no-op.
        set_message(999_999, "hi").unwrap();
        increment(999_999, 1);
        finish(999_999, "done").unwrap();
        abandon(999_999);
    }

    #[test]
    fn handle_and_message_quotas_reject_growth_and_recover() {
        let runtime_id = 8_300_001;
        crate::native::with_runtime_context(runtime_id, || {
            let mut handles = (0..MAX_PROGRESS_HANDLES)
                .map(|_| bar_new(1).unwrap())
                .collect::<Vec<_>>();
            assert!(bar_new(1).unwrap_err().contains("handle quota"));
            assert!(
                set_message(handles[0], &"x".repeat(MAX_PROGRESS_MESSAGE_BYTES + 1))
                    .unwrap_err()
                    .contains("message exceeds")
            );
            abandon(handles.pop().unwrap());
            handles.push(spinner_new().unwrap());
        });
        assert_eq!(cleanup_runtime(runtime_id), MAX_PROGRESS_HANDLES);
    }

    #[test]
    fn full_life_cycle() {
        let id = bar_new(3).unwrap();
        set_message(id, "trabajando").unwrap();
        increment(id, 1);
        set_position(id, 3);
        finish(id, "listo").unwrap();
        // ID has been removed.
        assert!(with_bar(id, |_| ()).is_none());
    }
}
