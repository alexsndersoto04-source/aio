//! Titan Standard Library.

pub mod game;
pub mod audio;
pub mod gui;
pub mod freestanding;
pub mod freestanding_memory;
pub mod freestanding_cpu;
pub mod freestanding_mmio;
pub mod bytes;
pub mod cache;
pub mod checksum;
pub mod collections;
pub mod csv;
pub mod encoding;
pub mod io;
pub mod http;
pub mod http_client;
pub mod json;
pub mod math;
pub mod metrics;
pub mod multipart;
pub mod net;
pub mod native;
pub mod path;
pub mod process;
pub mod stats;
pub mod sync;
pub mod testing;
pub mod text;
pub mod time;
pub mod websocket;
pub mod window;
pub mod input;
pub mod clipboard;
pub mod mobile;

// --- Phase 1: optional extras (each behind its own Cargo feature) ------
#[cfg(feature = "regex_mod")]    pub mod regex_mod;
#[cfg(feature = "uuid_mod")]     pub mod uuid_mod;
#[cfg(feature = "hash_mod")]     pub mod hash_mod;
#[cfg(feature = "random_mod")]   pub mod random_mod;
#[cfg(feature = "datetime_mod")] pub mod datetime_mod;
#[cfg(feature = "url_mod")]      pub mod url_mod;
#[cfg(feature = "dirs_mod")]     pub mod dirs_mod;

// --- Phase 2: formats & compression ------------------------------------
#[cfg(feature = "compress_mod")] pub mod compress_mod;
#[cfg(feature = "archive_mod")]  pub mod archive_mod;
#[cfg(feature = "yaml_mod")]     pub mod yaml_mod;
#[cfg(feature = "xml_mod")]      pub mod xml_mod;

// --- Phase 3: advanced networking --------------------------------------
#[cfg(feature = "http_full_mod")] pub mod http_full_mod;
#[cfg(feature = "dns_mod")]       pub mod dns_mod;
#[cfg(feature = "email_mod")]     pub mod email_mod;

// --- Phase 4: modern cryptography --------------------------------------
#[cfg(feature = "crypto_mod")]    pub mod crypto_mod;
#[cfg(feature = "password_mod")]  pub mod password_mod;
#[cfg(feature = "jwt_mod")]       pub mod jwt_mod;

// --- Phase 5: Termux / Android integration -----------------------------
#[cfg(feature = "termux_mod")]    pub mod termux_mod;

// --- Phase 6: Terminal & TUI -------------------------------------------
#[cfg(feature = "term_mod")]      pub mod term_mod;
#[cfg(feature = "readline_mod")]  pub mod readline_mod;
#[cfg(feature = "progress_mod")]  pub mod progress_mod;

// --- Phase 7: Images & QR codes ----------------------------------------
#[cfg(feature = "image_mod")]     pub mod image_mod;
#[cfg(feature = "qrcode_mod")]    pub mod qrcode_mod;

// --- Phase 8: System & OS ----------------------------------------------
#[cfg(feature = "procfs_mod")]    pub mod procfs_mod;
#[cfg(feature = "fswatch_mod")]   pub mod fswatch_mod;
#[cfg(feature = "signals_mod")]   pub mod signals_mod;

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
