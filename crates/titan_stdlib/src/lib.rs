//! Titan Standard Library.

pub mod audio;
pub mod bytes;
pub mod cache;
pub mod checksum;
pub mod clipboard;
pub mod collections;
pub mod csv;
pub mod encoding;
pub mod freestanding;
pub mod freestanding_cpu;
pub mod freestanding_memory;
pub mod freestanding_mmio;
pub mod game;
pub mod gui;
pub mod gui_raster;
pub mod http;
pub mod http_client;
pub mod input;
pub mod io;
pub mod json;
pub mod math;
pub mod metrics;
pub mod mobile;
pub mod multipart;
pub mod native;
pub mod net;
pub mod path;
pub mod process;
pub mod stats;
pub mod sync;
pub mod testing;
pub mod text;
pub mod time;
pub mod websocket;
pub mod window;
#[cfg(all(feature = "window_live", not(target_os = "android")))]
pub mod window_live;

// --- Phase 1: optional extras (each behind its own Cargo feature) ------
#[cfg(feature = "datetime_mod")]
pub mod datetime_mod;
#[cfg(feature = "dirs_mod")]
pub mod dirs_mod;
#[cfg(feature = "hash_mod")]
pub mod hash_mod;
#[cfg(feature = "random_mod")]
pub mod random_mod;
#[cfg(feature = "regex_mod")]
pub mod regex_mod;
#[cfg(feature = "url_mod")]
pub mod url_mod;
#[cfg(feature = "uuid_mod")]
pub mod uuid_mod;

// --- Phase 2: formats & compression ------------------------------------
#[cfg(feature = "archive_mod")]
pub mod archive_mod;
#[cfg(feature = "compress_mod")]
pub mod compress_mod;
#[cfg(feature = "xml_mod")]
pub mod xml_mod;
#[cfg(feature = "yaml_mod")]
pub mod yaml_mod;

// --- Phase 3: advanced networking --------------------------------------
#[cfg(feature = "dns_mod")]
pub mod dns_mod;
#[cfg(feature = "email_mod")]
pub mod email_mod;
#[cfg(feature = "http_full_mod")]
pub mod http_full_mod;

// --- Phase 4: modern cryptography --------------------------------------
#[cfg(feature = "crypto_mod")]
pub mod crypto_mod;
#[cfg(feature = "jwt_mod")]
pub mod jwt_mod;
#[cfg(feature = "password_mod")]
pub mod password_mod;

// --- Phase 5: Termux / Android integration -----------------------------
#[cfg(feature = "termux_mod")]
pub mod termux_mod;

// --- Phase 6: Terminal & TUI -------------------------------------------
#[cfg(feature = "progress_mod")]
pub mod progress_mod;
#[cfg(feature = "readline_mod")]
pub mod readline_mod;
#[cfg(feature = "term_mod")]
pub mod term_mod;

// --- Phase 7: Images & QR codes ----------------------------------------
#[cfg(feature = "image_mod")]
pub mod image_mod;
#[cfg(feature = "qrcode_mod")]
pub mod qrcode_mod;

// --- Phase 8: System & OS ----------------------------------------------
#[cfg(feature = "fswatch_mod")]
pub mod fswatch_mod;
#[cfg(feature = "procfs_mod")]
pub mod procfs_mod;
#[cfg(all(feature = "signals_mod", unix))]
pub mod signals_mod;

// --- Phase 9: Audio (WAV synthesis / I/O + Termux media playback) ------
#[cfg(feature = "audio_mod")]
pub mod audio_mod;

// --- Phase 10: NoSQL — embedded key-value + Redis client ---------------
#[cfg(feature = "kv_mod")]
pub mod kv_mod;
#[cfg(feature = "redis_mod")]
pub mod redis_mod;

// --- Phase 11: HTTP server (tiny_http) + URL router (matchit) ----------
#[cfg(feature = "router_mod")]
pub mod router_mod;
#[cfg(feature = "server_mod")]
pub mod server_mod;

// --- Phase 14: SVG charts (plotters, no ttf/font-kit for Termux) -------
#[cfg(feature = "plot_mod")]
pub mod plot_mod;

// --- Phase 12: Local AI — HuggingFace tokenizers (pure-Rust build) -----
#[cfg(feature = "tokenize_mod")]
pub mod tokenize_mod;

// --- Phase 12 part 2: Local AI — ONNX inference (tract, pure-Rust) -----
#[cfg(feature = "onnx_mod")]
pub mod onnx_mod;

// --- Phase 13': Wi-Fi introspection via termux-wifi-* CLI --------------
#[cfg(feature = "wifi_mod")]
pub mod wifi_mod;

// --- Phase 12 pt.4: Vector math for embeddings + semantic search -------
#[cfg(feature = "vector_mod")]
pub mod vector_mod;

// --- Phase 16: PDF generation (printpdf, pure-Rust core) ---------------
#[cfg(feature = "pdf_mod")]
pub mod pdf_mod;

// --- Phase 34: process, collections avanzadas, datetime extendido ---
#[cfg(feature = "collections_mod")]
pub mod collections_mod;
#[cfg(feature = "datetime_ext_mod")]
pub mod datetime_ext_mod;
#[cfg(feature = "process_mod")]
pub mod process_mod;

pub fn print(value: &str) {
    println!("{}", value);
}
pub fn eprint(value: &str) {
    eprintln!("{}", value);
}
pub fn args() -> Vec<String> {
    std::env::args().collect()
}
pub fn env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}
pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
pub fn sleep(seconds: f64) {
    std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
}
