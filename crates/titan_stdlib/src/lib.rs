//! Titan Standard Library.

pub mod bytes;
pub mod cache;
pub mod checksum;
pub mod collections;
pub mod csv;
pub mod encoding;
pub mod io;
pub mod json;
pub mod math;
pub mod net;
pub mod native;
pub mod path;
pub mod process;
pub mod stats;
pub mod sync;
pub mod testing;
pub mod text;
pub mod time;

pub fn print(value: &str) { println!("{}", value); }
pub fn eprint(value: &str) { eprintln!("{}", value); }
pub fn args() -> Vec<String> { std::env::args().collect() }
pub fn env(key: &str) -> Option<String> { std::env::var(key).ok() }
pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64()).unwrap_or(0.0)
}
pub fn sleep(seconds: f64) {
    std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
}