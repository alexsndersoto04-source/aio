//! Monotonic timing, deadlines, and Unix wall-clock helpers.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub fn unix_seconds() -> Result<u64, std::time::SystemTimeError> { Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()) }
pub fn unix_millis() -> Result<u128, std::time::SystemTimeError> { Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()) }
pub fn sleep(duration: Duration) { std::thread::sleep(duration); }
pub fn milliseconds(value: u64) -> Duration { Duration::from_millis(value) }
pub fn seconds(value: f64) -> Option<Duration> { if value.is_finite() && value >= 0.0 { Some(Duration::from_secs_f64(value)) } else { None } }

#[derive(Debug, Clone)] pub struct Stopwatch { started: Instant, lap: Instant }
impl Stopwatch {
    pub fn start() -> Self { let now = Instant::now(); Self { started: now, lap: now } }
    pub fn elapsed(&self) -> Duration { self.started.elapsed() }
    pub fn lap(&mut self) -> Duration { let elapsed = self.lap.elapsed(); self.lap = Instant::now(); elapsed }
    pub fn reset(&mut self) { let now = Instant::now(); self.started = now; self.lap = now; }
}

#[derive(Debug, Clone)] pub struct Deadline(Instant);
impl Deadline { pub fn after(duration: Duration) -> Self { Self(Instant::now() + duration) } pub fn expired(&self) -> bool { Instant::now() >= self.0 } pub fn remaining(&self) -> Duration { self.0.saturating_duration_since(Instant::now()) } }

#[cfg(test)] mod tests { use super::*; #[test] fn validates_seconds() { assert!(seconds(1.5).is_some()); assert!(seconds(-1.0).is_none()); assert!(seconds(f64::NAN).is_none()); } }
