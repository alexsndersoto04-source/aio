//! Stable metadata for functions callable from Titan bytecode.
//!
//! Implementations live in `titan_vm`, where host values can be converted to
//! VM values. Keeping names/signatures here gives type checking and codegen one
//! authoritative registry without introducing a crate dependency cycle.

use std::cell::Cell;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, MutexGuard, RwLock, RwLockWriteGuard};

thread_local! {
    static CURRENT_RUNTIME_ID: Cell<u64> = const { Cell::new(0) };
}

struct RuntimeContextReset<'a> {
    current: &'a Cell<u64>,
    previous: u64,
}

impl Drop for RuntimeContextReset<'_> {
    fn drop(&mut self) {
        self.current.set(self.previous);
    }
}

/// Runs one native invocation in the ownership domain of a VM runtime.
///
/// This is public only because `titan_vm` and `titan_stdlib` are separate
/// crates in the same binary. Native resource registries use the current ID
/// to prevent opaque integer handles from crossing between independent VMs.
#[doc(hidden)]
pub fn with_runtime_context<R>(runtime_id: u64, operation: impl FnOnce() -> R) -> R {
    CURRENT_RUNTIME_ID.with(|current| {
        let reset = RuntimeContextReset {
            current,
            previous: current.replace(runtime_id),
        };
        let result = operation();
        drop(reset);
        result
    })
}

pub(crate) fn current_runtime_id() -> u64 {
    CURRENT_RUNTIME_ID.with(Cell::get)
}

pub(crate) fn runtime_handle_key<T>(handle: T) -> (u64, T) {
    (current_runtime_id(), handle)
}

pub(crate) fn lock_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn write_recover<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn remove_runtime_entries<K: Eq + Hash, V>(
    entries: &mut HashMap<(u64, K), V>,
    runtime_id: u64,
) -> usize {
    let before = entries.len();
    entries.retain(|(owner, _), _| *owner != runtime_id);
    before - entries.len()
}

/// Releases process-wide native handles after the last VM/task in an
/// ownership domain has gone away. Each module removes only entries carrying
/// `runtime_id`; handles belonging to other live VMs are left untouched.
#[doc(hidden)]
pub fn cleanup_runtime_resources(runtime_id: u64) -> usize {
    let mut released = crate::window::cleanup_runtime(runtime_id);
    released += crate::audio::cleanup_runtime(runtime_id);
    released += crate::game::cleanup_runtime(runtime_id);
    released += crate::gui::cleanup_runtime(runtime_id);
    released += crate::input::cleanup_runtime(runtime_id);
    released += crate::clipboard::cleanup_runtime(runtime_id);
    released += crate::mobile::cleanup_runtime(runtime_id);
    released += crate::metrics::cleanup_runtime(runtime_id);
    released += crate::freestanding::cleanup_runtime(runtime_id);
    released += crate::freestanding_cpu::cleanup_runtime(runtime_id);
    released += crate::freestanding_memory::cleanup_runtime(runtime_id);
    released += crate::freestanding_mmio::cleanup_runtime(runtime_id);
    #[cfg(all(feature = "signals_mod", unix))]
    {
        released += crate::signals_mod::cleanup_runtime(runtime_id);
    }

    #[cfg(feature = "kv_mod")]
    {
        released += crate::kv_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "fswatch_mod")]
    {
        released += crate::fswatch_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "redis_mod")]
    {
        released += crate::redis_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "image_mod")]
    {
        released += crate::image_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "tokenize_mod")]
    {
        released += crate::tokenize_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "onnx_mod")]
    {
        released += crate::onnx_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "pdf_mod")]
    {
        released += crate::pdf_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "progress_mod")]
    {
        released += crate::progress_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "router_mod")]
    {
        released += crate::router_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "server_mod")]
    {
        released += crate::server_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "process_mod")]
    {
        released += crate::process_mod::cleanup_runtime(runtime_id);
    }
    #[cfg(feature = "collections_mod")]
    {
        released += crate::collections_mod::cleanup_runtime(runtime_id);
    }

    released
}

/// Native windows are deliberately thread-local because OS window objects are
/// not `Send`. Every VM (root or task) cleans the windows it created on its own
/// thread when that VM is dropped.
#[doc(hidden)]
pub fn cleanup_thread_local_runtime_resources(runtime_id: u64) -> usize {
    #[cfg(all(feature = "window_live", not(target_os = "android")))]
    {
        return crate::window_live::cleanup_runtime(runtime_id);
    }
    #[cfg(not(all(feature = "window_live", not(target_os = "android"))))]
    {
        let _ = runtime_id;
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeType {
    Any,
    Int,
    Float,
    Bool,
    String,
    Bytes,
    Array,
    Map,
    Nil,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    None,
    Filesystem,
    Process,
    Network,
    Environment,
    UserInterface,
    FilesystemUserInterface,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeSignature {
    pub name: &'static str,
    pub params: &'static [NativeType],
    pub result: NativeType,
    pub capability: Capability,
}

macro_rules! native {
    ($name:literal, [$($param:ident),*], $result:ident) => { NativeSignature { name: $name, params: &[$(NativeType::$param),*], result: NativeType::$result, capability: Capability::None } };
    ($name:literal, [$($param:ident),*], $result:ident, $capability:ident) => { NativeSignature { name: $name, params: &[$(NativeType::$param),*], result: NativeType::$result, capability: Capability::$capability } };
}

pub static NATIVES: &[NativeSignature] = &[
    native!("std::text::length", [String], Int),
    native!("std::text::reverse", [String], String),
    native!("std::text::uppercase", [String], String),
    native!("std::text::lowercase", [String], String),
    native!("std::text::trim", [String], String),
    native!("std::text::capitalize", [String], String),
    native!("std::text::escape_html", [String], String),
    native!("std::text::slugify", [String], String),
    native!("std::text::levenshtein", [String, String], Int),
    native!("std::text::equals", [String, String], Bool),
    native!("std::text::hash64", [String], Int),
    native!("std::text::contains", [String, String], Bool),
    native!("std::text::starts_with", [String, String], Bool),
    native!("std::text::ends_with", [String, String], Bool),
    native!("std::text::replace", [String, String, String], String),
    native!("std::text::truncate", [String, Int, String], String),
    // Phase 32: parseo de numeros desde string + substring por chars.
    // parse_int / parse_float retornan Option (nil si el parseo falla).
    // substring(str, start, end) devuelve los chars en [start, end),
    // clampeando a la longitud real.
    native!("std::text::parse_int", [String], Any),
    native!("std::text::parse_float", [String], Any),
    native!("std::text::substring", [String, Int, Int], String),
    native!("std::text::words", [String], Array),
    native!("std::text::lines", [String], Array),
    native!("std::encoding::hex_encode", [Bytes], String),
    native!("std::encoding::hex_decode", [String], Bytes),
    native!("std::encoding::base64_encode", [Bytes], String),
    native!("std::encoding::base64_decode", [String], Bytes),
    native!("std::encoding::percent_encode", [String], String),
    native!("std::encoding::percent_decode", [String], String),
    native!("std::encoding::utf8_encode", [String], Bytes),
    native!("std::encoding::utf8_decode", [Bytes], String),
    native!("std::checksum::fnv1a64", [Bytes], Int),
    native!("std::checksum::crc32", [Bytes], Int),
    native!("std::checksum::constant_time_eq", [Bytes, Bytes], Bool),
    native!("std::bytes::from_array", [Array], Bytes),
    native!("std::bytes::to_array", [Bytes], Array),
    native!("std::bytes::length", [Bytes], Int),
    native!("std::bytes::concat", [Bytes, Bytes], Bytes),
    native!("std::bytes::slice", [Bytes, Int, Int], Bytes),
    native!("std::bytes::read_u32_le", [Bytes, Int], Int),
    native!("std::bytes::write_u32_le", [Int], Bytes),
    native!("std::http::parse_request", [Bytes], Any),
    native!("std::http::build_response", [Int, Map, Bytes, Bool], Bytes),
    native!("std::http::reason_phrase", [Int], String),
    native!("std::http::route_match", [String, String], Any),
    native!("std::http::parse_query", [String, Int], Map),
    native!("std::http::security_headers", [Map], Map),
    native!("std::http::cors", [Map, String, String], Map),
    native!("std::http::request_id", [Map], Map),
    native!("std::http::rate_limit", [String, Int, Int], Bool),
    native!("std::http::json_response", [Int, Any], Map),
    native!("std::http::error_response", [Int, String], Map),
    native!(
        "std::http::request",
        [String, String, Map, Bytes, Int, Int, Int],
        Map,
        Network
    ),
    native!(
        "std::http::parse_multipart",
        [String, Bytes, Int, Int],
        Array
    ),
    native!("std::metrics::counter_add", [String, Int], Int),
    native!("std::metrics::counter_get", [String], Int),
    native!("std::metrics::gauge_set", [String, Float], Nil),
    native!("std::metrics::gauge_get", [String], Float),
    native!("std::metrics::histogram_record", [String, Float], Nil),
    native!("std::metrics::snapshot", [], Map),
    native!("std::metrics::prometheus_export", [], String),
    native!("std::metrics::reset", [], Nil),
    native!("std::ws::accept_key", [String], String),
    native!("std::ws::upgrade_response", [String, String], Bytes),
    native!("std::ws::validate_upgrade", [Map, String], Bytes),
    native!("std::ws::validate_accept", [Bytes, String], Bool),
    native!("std::ws::encode", [Int, Bytes, Bool], Bytes),
    native!("std::ws::parse", [Bytes, Bool, Int], Any),
    native!("std::csv::parse", [String], Array),
    native!("std::csv::serialize", [Array], String),
    native!("std::json::parse", [String], Any),
    native!("std::json::stringify", [Any], String),
    native!("std::json::pretty", [Any], String),
    native!("std::json::pointer", [Any, String], Any),
    native!("std::json::merge", [Any, Any], Any),
    native!("std::json::flatten", [Any], Array),
    native!("std::array::set", [Array, Int, Any], Array),
    native!("std::array::push", [Array, Any], Array),
    native!("std::array::pop", [Array], Array),
    native!("std::array::slice", [Array, Int, Int], Array),
    native!("std::array::concat", [Array, Array], Array),
    // filled(n, value): array of `n` copies of `value`. Handy for building
    // sized float buffers for ONNX / plotters without a hand-rolled loop.
    native!("std::array::filled", [Int, Any], Array),
    // range(start, end): [start, start+1, ..., end-1] as ints. Complements
    // the `for i in start..end` syntax when you need the values as data.
    native!("std::array::range", [Int, Int], Array),
    native!("std::wasm::heap_used", [], Int),
    native!("std::wasm::heap_capacity", [], Int),
    native!("std::wasm::heap_limit", [], Int),
    native!("std::wasm::heap_set_limit", [Int], Bool),
    native!("std::wasm::heap_checkpoint", [], Int),
    native!("std::wasm::heap_restore", [Int], Bool),
    native!("std::wasm::heap_allocations", [], Int),
    native!("std::wasm::heap_allocated_bytes", [], Int),
    native!("std::wasm::heap_restores", [], Int),
    native!("std::wasm::heap_reclaimed_bytes", [], Int),
    native!("std::wasm::heap_peak_used", [], Int),
    native!("std::wasm::heap_reset_counters", [], Bool),
    native!("std::wasm::heap_scope_begin", [], Int),
    native!("std::wasm::heap_scope_end", [Int], Bool),
    native!("std::collections::length", [Any], Int),
    native!("std::collections::contains", [Array, Any], Bool),
    native!("std::collections::reverse", [Array], Array),
    native!("std::collections::deduplicate", [Array], Array),
    native!("std::collections::join", [Array, String], String),
    native!("std::collections::chunk", [Array, Int], Array),
    native!("std::map::new", [], Map),
    native!("std::map::length", [Map], Int),
    native!("std::map::insert_new", [Map, String, Any], Map),
    native!("std::map::keys", [Map], Array),
    native!("std::map::values", [Map], Array),
    native!("std::map::contains", [Map, String], Bool),
    native!("std::map::get", [Map, String], Any),
    native!("std::map::insert", [Map, String, Any], Map),
    native!("std::map::remove", [Map, String], Map),
    native!("std::math::sqrt", [Float], Float),
    native!("std::math::pow", [Float, Float], Float),
    native!("std::math::sin", [Float], Float),
    native!("std::math::cos", [Float], Float),
    native!("std::math::tan", [Float], Float),
    native!("std::math::ln", [Float], Float),
    native!("std::math::abs", [Float], Float),
    native!("std::math::floor", [Float], Float),
    native!("std::math::ceil", [Float], Float),
    native!("std::math::round", [Float], Float),
    // Additions in v0.13.0: exp / log / int<->float conversions.
    native!("std::math::exp", [Float], Float),
    native!("std::math::log", [Float, Float], Float),
    native!("std::math::to_float", [Int], Float),
    native!("std::math::to_int", [Float], Int),
    native!("std::stats::mean", [Array], Float),
    native!("std::stats::median", [Array], Float),
    native!("std::stats::quantile", [Array, Float], Float),
    native!("std::stats::variance", [Array], Float),
    native!("std::stats::stddev", [Array], Float),
    native!("std::time::unix_seconds", [], Int),
    native!("std::time::unix_millis", [], Int),
    native!("std::time::sleep_ms", [Int], Nil),
    native!("std::path::join", [String, String], String),
    native!("std::path::normalize", [String], String),
    native!("std::path::parent", [String], String),
    native!("std::path::file_name", [String], String),
    native!("std::path::stem", [String], String),
    native!("std::path::extension", [String], String),
    native!("std::path::absolute", [String], String, Filesystem),
    native!("std::path::canonical", [String], String, Filesystem),
    native!("std::fs::read_text", [String], String, Filesystem),
    native!("std::fs::read_bytes", [String], Bytes, Filesystem),
    native!("std::fs::write_text", [String, String], Nil, Filesystem),
    native!("std::fs::write_bytes", [String, Bytes], Nil, Filesystem),
    native!("std::fs::atomic_write", [String, Bytes], Nil, Filesystem),
    native!("std::fs::append", [String, Bytes], Nil, Filesystem),
    native!("std::fs::exists", [String], Bool, Filesystem),
    native!("std::fs::is_file", [String], Bool, Filesystem),
    native!("std::fs::is_dir", [String], Bool, Filesystem),
    native!("std::fs::create_dir", [String], Nil, Filesystem),
    native!("std::fs::remove_file", [String], Nil, Filesystem),
    native!("std::fs::remove_dir", [String], Nil, Filesystem),
    native!("std::fs::list_dir", [String], Array, Filesystem),
    native!("std::fs::file_size", [String], Int, Filesystem),
    native!("std::fs::copy", [String, String], Int, Filesystem),
    native!("std::fs::rename", [String, String], Nil, Filesystem),
    // Phase 34 redefines std::process::run as run(command): the single canonical
    // signature lives in the "Phase 34: std::process" section below. Keeping the
    // legacy run(program, args) entry here shadowed it (lookup() returns the
    // first NATIVES match) and broke `run("cmd")` calls with
    // "function expected 2 arguments, found 1".
    native!(
        "std::process::run_timeout",
        [String, Array, Int],
        Map,
        Process
    ),
    native!("std::env::get", [String], String, Environment),
    native!("std::env::args", [], Array, Environment),
    native!("std::env::current_dir", [], String, Environment),
    native!("std::net::http_get", [String], Map, Network),
    native!("std::web::query_exists", [String], Bool),
    native!("std::web::set_text", [String, String], Nil),
    native!("std::web::set_html", [String, String], Nil),
    native!("std::web::set_attribute", [String, String, String], Nil),
    native!("std::web::add_class", [String, String], Nil),
    native!("std::web::remove_class", [String, String], Nil),
    native!("std::web::focus", [String], Nil),
    native!("std::web::set_title", [String], Nil),
    native!("std::web::listen", [String, String, String], Int),
    native!("std::web::unlisten", [Int], Bool),
    native!("std::web::event_type", [], String),
    native!("std::web::event_value", [], String),
    native!("std::web::event_key", [], String),
    native!("std::web::event_target_id", [], String),
    native!("std::web::event_checked", [], Bool),
    native!("std::web::event_x", [], Int),
    native!("std::web::event_y", [], Int),
    native!("std::web::fetch", [String, Int, Int, String], Int),
    native!("std::web::fetch_cancel", [Int], Bool),
    native!("std::web::fetch_ok", [], Bool),
    native!("std::web::fetch_status", [], Int),
    native!("std::web::fetch_body", [], String),
    native!("std::web::fetch_url", [], String),
    native!("std::web::fetch_error", [], String),
    native!("std::web::fetch_headers", [], String),
    native!(
        "std::web::request",
        [String, String, String, String, Int, Int, String],
        Int
    ),
    native!(
        "std::web::ws_connect",
        [String, String, Int, String, String, String, String],
        Int
    ),
    native!("std::web::ws_send", [Int, String], Bool),
    native!("std::web::ws_close", [Int, Int, String], Bool),
    native!("std::web::ws_id", [], Int),
    native!("std::web::ws_message", [], String),
    native!("std::web::ws_protocol", [], String),
    native!("std::web::ws_close_code", [], Int),
    native!("std::web::ws_close_reason", [], String),
    native!("std::web::ws_was_clean", [], Bool),
    native!("std::web::ws_error", [], String),
    native!("std::web::canvas_resize", [String, Int, Int], Nil),
    native!("std::web::canvas_clear", [String, String], Nil),
    native!(
        "std::web::canvas_fill_rect",
        [String, Int, Int, Int, Int, String],
        Nil
    ),
    native!(
        "std::web::canvas_stroke_rect",
        [String, Int, Int, Int, Int, String, Int],
        Nil
    ),
    native!(
        "std::web::canvas_line",
        [String, Int, Int, Int, Int, String, Int],
        Nil
    ),
    native!(
        "std::web::canvas_text",
        [String, String, Int, Int, String, String],
        Nil
    ),
    native!("std::web::animation_start", [String], Int),
    native!("std::web::animation_cancel", [Int], Bool),
    native!("std::web::frame_id", [], Int),
    native!("std::web::frame_time_ms", [], Int),
    native!("std::web::frame_delta_ms", [], Int),
    native!("std::web::frame_count", [], Int),
    native!("std::web::webgl_supported", [String], Bool),
    native!(
        "std::web::webgl_create",
        [String, String, String, String, String],
        Int
    ),
    native!("std::web::webgl_uniform_f32", [Int, String, Int, Int], Bool),
    native!("std::web::webgl_draw", [Int, String], Bool),
    native!("std::web::webgl_delete", [Int], Bool),
    native!(
        "std::window::create",
        [String, Int, Int],
        Int,
        UserInterface
    ),
    native!("std::window::is_open", [Int], Bool, UserInterface),
    native!("std::window::close", [Int], Bool, UserInterface),
    native!("std::window::set_title", [Int, String], Bool, UserInterface),
    native!("std::window::resize", [Int, Int, Int], Bool, UserInterface),
    native!("std::window::poll_events", [Int], Array, UserInterface),
    // Fase 2 — ventana viva (minifb). -1 si no hay display (headless).
    native!(
        "std::window::live_open",
        [String, Int, Int],
        Int,
        UserInterface
    ),
    native!("std::window::live_is_open", [Int], Bool, UserInterface),
    native!("std::window::live_close", [Int], Bool, UserInterface),
    native!(
        "std::window::live_set_title",
        [Int, String],
        Bool,
        UserInterface
    ),
    native!("std::window::live_pump", [Int, Int], Int, UserInterface),
    native!("std::window::live_poll_events", [Int], Array, UserInterface),
    native!("std::input::is_key_pressed", [String], Bool, UserInterface),
    native!("std::input::mouse_pos", [], Array, UserInterface),
    native!(
        "std::input::is_mouse_button_pressed",
        [Int],
        Bool,
        UserInterface
    ),
    native!("std::input::touch_pos", [Int], Array, UserInterface),
    // Setters expuestos en Fase 1: el host (o el propio script) alimenta el
    // estado; el backend de ventanas (Fase 2) cableara eventos reales aqui.
    native!(
        "std::input::set_key_state",
        [String, Bool],
        Bool,
        UserInterface
    ),
    native!("std::input::set_mouse_pos", [Int, Int], Bool, UserInterface),
    native!(
        "std::input::set_mouse_button",
        [Int, Bool],
        Bool,
        UserInterface
    ),
    native!(
        "std::input::set_touch_point",
        [Int, Int, Int, Bool],
        Bool,
        UserInterface
    ),
    native!("std::clipboard::get_text", [], String, UserInterface),
    native!("std::clipboard::set_text", [String], Bool, UserInterface),
    native!("std::notify::send", [String, String], Bool, UserInterface),
    native!("std::mobile::state", [], String, UserInterface),
    native!("std::mobile::trigger", [String], Bool, UserInterface),
    native!("std::mobile::poll_events", [], Array, UserInterface),
    native!("std::game::init", [String, Int, Int], Bool, UserInterface),
    native!("std::game::step", [], Float, UserInterface),
    native!("std::game::fps", [], Int, UserInterface),
    native!(
        "std::game::check_collision",
        [Float, Float, Float, Float, Float, Float, Float, Float],
        Bool
    ),
    native!("std::game::shutdown", [], Bool, UserInterface),
    // NOTE: the legacy in-memory audio module (buffers, playback flags,
    // volume) was retired in 0.7.0 and replaced by std::audio::* below,
    // which is the real one (WAV I/O via `hound` + playback through
    // termux-media-player). Names kept only as `sim_*` to avoid silently
    // breaking pre-0.7.0 programs that might have used them for tests.
    native!("std::audio::sim_init", [], Bool),
    native!("std::audio::sim_load_wave", [Float, Int], Int),
    native!("std::audio::sim_sample_count", [Int], Int),
    native!("std::audio::sim_play", [Int, Bool], Bool),
    native!("std::audio::sim_set_volume", [Int, Float], Bool),
    native!("std::audio::sim_stop", [Int], Bool),
    native!("std::gui::init", [], Bool, UserInterface),
    native!(
        "std::gui::create_container",
        [String, Int, Int],
        Int,
        UserInterface
    ),
    native!(
        "std::gui::add_button",
        [Int, String, Int, Int, Int, Int],
        Int,
        UserInterface
    ),
    native!(
        "std::gui::add_label",
        [Int, String, Int, Int],
        Int,
        UserInterface
    ),
    native!("std::gui::set_text", [Int, String], Bool, UserInterface),
    native!("std::gui::get_text", [Int], String, UserInterface),
    native!("std::gui::trigger_click", [Int], Bool, UserInterface),
    native!("std::gui::is_clicked", [Int], Bool, UserInterface),
    native!("std::gui::child_count", [Int], Int, UserInterface),
    native!("std::gui::shutdown", [], Bool, UserInterface),
    // Fase 2: rasterizador por software — pixeles RGBA reales del arbol.
    native!("std::gui::render", [Int], Any, UserInterface),
    native!("std::image::from_rgba", [Int, Int, Bytes], Int),
    native!("std::freestanding::init", [String], Bool),
    native!("std::freestanding::validate_target_spec", [String], Bool),
    native!(
        "std::freestanding::generate_linker_script",
        [String, Int, Int],
        String
    ),
    native!(
        "std::freestanding::generate_startup_asm",
        [String, String],
        String
    ),
    native!("std::freestanding::get_active_target", [], String),
    native!("std::freestanding::shutdown", [], Bool),
    native!(
        "std::freestanding_memory::init_frame_allocator",
        [Int, Int],
        Bool
    ),
    native!("std::freestanding_memory::allocate_frame", [], Int),
    native!("std::freestanding_memory::deallocate_frame", [Int], Bool),
    native!("std::freestanding_memory::map_page", [Int, Int, Int], Bool),
    native!("std::freestanding_memory::translate_page", [Int], Int),
    native!("std::freestanding_memory::free_frames_count", [], Int),
    native!("std::freestanding_memory::shutdown", [], Bool),
    native!("std::freestanding_cpu::init_exception_table", [Int], Bool),
    native!(
        "std::freestanding_cpu::register_exception_handler",
        [Int, Int],
        Bool
    ),
    native!(
        "std::freestanding_cpu::dispatch_exception",
        [Int, Int, Int],
        Int
    ),
    native!(
        "std::freestanding_cpu::register_syscall_handler",
        [Int, Int],
        Bool
    ),
    native!(
        "std::freestanding_cpu::invoke_syscall",
        [Int, Int, Int, Int],
        Int
    ),
    native!("std::freestanding_cpu::get_last_fault_addr", [], Int),
    native!("std::freestanding_cpu::shutdown", [], Bool),
    native!("std::freestanding_mmio::init_mmio_region", [Int, Int], Bool),
    native!("std::freestanding_mmio::read_mmio_u32", [Int], Int),
    native!("std::freestanding_mmio::write_mmio_u32", [Int, Int], Bool),
    native!("std::freestanding_mmio::serial_init", [Int, Int], Bool),
    native!("std::freestanding_mmio::serial_write_str", [String], Int),
    native!("std::freestanding_mmio::serial_get_buffer", [], String),
    native!("std::freestanding_mmio::shutdown", [], Bool),
    native!("std::testing::assert", [Bool, String], Nil),
    native!("std::testing::assert_eq", [Any, Any, String], Nil),
    // --- Phase 18: exception-safe closure call ---
    // std::try::catch(fn, args...) runs the closure `fn` with the given
    // `args...` (0 or more), captures ANY runtime error and returns a
    // Result::Ok(value) or Result::Err(String message). Compiled specially
    // by titan_codegen into the TryCall opcode (not dispatched through the
    // native table), but declared here so the typechecker knows the name.
    native!("std::try::catch", [Any], Any),
    // --- Phase 1: regex ---
    native!("std::regex::is_match", [String, String], Bool),
    native!("std::regex::find", [String, String], String),
    native!("std::regex::find_all", [String, String], Array),
    native!("std::regex::captures", [String, String], Array),
    native!("std::regex::replace_all", [String, String, String], String),
    native!("std::regex::split", [String, String], Array),
    native!("std::regex::is_valid", [String], Bool),
    // --- Phase 1: uuid ---
    native!("std::uuid::v4", [], String),
    native!("std::uuid::v7", [], String),
    native!("std::uuid::is_valid", [String], Bool),
    native!("std::uuid::normalize", [String], String),
    native!("std::uuid::nil", [], String),
    // --- Phase 1: hash ---
    native!("std::hash::sha256", [Bytes], String),
    native!("std::hash::sha384", [Bytes], String),
    native!("std::hash::sha512", [Bytes], String),
    native!("std::hash::sha3_256", [Bytes], String),
    native!("std::hash::sha3_512", [Bytes], String),
    native!("std::hash::blake3", [Bytes], String),
    native!("std::hash::sha256_bytes", [Bytes], Bytes),
    native!("std::hash::sha512_bytes", [Bytes], Bytes),
    native!("std::hash::blake3_bytes", [Bytes], Bytes),
    native!("std::hash::hmac_sha256", [Bytes, Bytes], String),
    native!("std::hash::hmac_sha512", [Bytes, Bytes], String),
    // --- Phase 1: random ---
    native!("std::random::int", [], Int),
    native!("std::random::range", [Int, Int], Int),
    native!("std::random::float", [], Float),
    native!("std::random::bool", [], Bool),
    native!("std::random::bytes", [Int], Bytes),
    native!("std::random::seeded_int", [Int, Int, Int], Int),
    native!("std::random::seeded_float", [Int], Float),
    native!("std::random::seeded_bytes", [Int, Int], Bytes),
    // --- Phase 1: datetime ---
    native!("std::datetime::now", [], Int),
    native!("std::datetime::now_iso", [], String),
    native!("std::datetime::format", [Int, String], String),
    native!("std::datetime::to_rfc3339", [Int], String),
    native!("std::datetime::to_rfc2822", [Int], String),
    native!("std::datetime::parse_rfc3339", [String], Int),
    native!("std::datetime::parse", [String, String], Int),
    native!(
        "std::datetime::utc_ymd_hms",
        [Int, Int, Int, Int, Int, Int],
        Int
    ),
    native!("std::datetime::add_seconds", [Int, Int], Int),
    native!("std::datetime::add_days", [Int, Int], Int),
    native!("std::datetime::diff_seconds", [Int, Int], Int),
    native!("std::datetime::year", [Int], Int),
    native!("std::datetime::month", [Int], Int),
    native!("std::datetime::day", [Int], Int),
    native!("std::datetime::hour", [Int], Int),
    native!("std::datetime::minute", [Int], Int),
    native!("std::datetime::second", [Int], Int),
    native!("std::datetime::weekday", [Int], Int),
    native!("std::datetime::format_offset", [Int, String, Int], String),
    // --- Phase 1: url ---
    native!("std::url::is_valid", [String], Bool),
    native!("std::url::scheme", [String], String),
    native!("std::url::host", [String], String),
    native!("std::url::port", [String], Int),
    native!("std::url::path", [String], String),
    native!("std::url::query", [String], String),
    native!("std::url::fragment", [String], String),
    native!("std::url::parse_query", [String], Map),
    native!("std::url::build_query", [Array], String),
    native!("std::url::join", [String, String], String),
    // --- Phase 2: compress ---
    native!("std::compress::gzip_encode", [Bytes, Int], Bytes),
    native!("std::compress::gzip_decode", [Bytes], Bytes),
    native!("std::compress::zlib_encode", [Bytes, Int], Bytes),
    native!("std::compress::zlib_decode", [Bytes], Bytes),
    native!("std::compress::deflate_encode", [Bytes, Int], Bytes),
    native!("std::compress::deflate_decode", [Bytes], Bytes),
    native!("std::compress::zstd_encode", [Bytes, Int], Bytes),
    native!("std::compress::zstd_decode", [Bytes], Bytes),
    // --- Phase 2: archive ---
    native!("std::archive::tar_pack", [Array], Bytes),
    native!("std::archive::tar_unpack", [Bytes], Array),
    native!("std::archive::zip_pack", [Array], Bytes),
    native!("std::archive::zip_unpack", [Bytes], Array),
    native!("std::archive::zip_list", [Bytes], Array),
    // --- Phase 2: yaml ---
    native!("std::yaml::parse", [String], Any),
    native!("std::yaml::stringify", [Any], String),
    native!("std::yaml::parse_multi", [String], Array),
    // --- Phase 2: xml ---
    native!("std::xml::parse", [String], Any),
    native!("std::xml::stringify", [Any], String),
    native!("std::xml::escape_text", [String], String),
    native!("std::xml::escape_attr", [String], String),
    // --- Phase 3: http_full (HTTPS client via ureq + rustls) ---
    // Signature: (method, url, headers_map, body_bytes, options_map) -> response_map
    // options_map keys: basic_user, basic_pass, bearer, user_agent, timeout_ms, max_redirects
    native!(
        "std::http_full::request",
        [String, String, Map, Bytes, Map],
        Map,
        Network
    ),
    native!("std::http_full::get_json", [String, Map, Map], Any, Network),
    native!(
        "std::http_full::post_json",
        [String, Any, Map, Map],
        Any,
        Network
    ),
    native!(
        "std::http_full::post_form",
        [String, Array, Map, Map],
        Map,
        Network
    ),
    // --- Phase 3: dns ---
    native!("std::dns::resolve", [String], Array, Network),
    native!("std::dns::resolve_ipv4", [String], Array, Network),
    native!("std::dns::resolve_ipv6", [String], Array, Network),
    native!("std::dns::resolve_mx", [String], Array, Network),
    native!("std::dns::resolve_txt", [String], Array, Network),
    native!("std::dns::resolve_cname", [String], Array, Network),
    native!("std::dns::reverse", [String], Array, Network),
    // --- Phase 3: email (SMTP+TLS via lettre + rustls) ---
    native!(
        "std::email::send_simple",
        [String, Int, String, String, String, String, String, String],
        String,
        Network
    ),
    native!(
        "std::email::send_html",
        [String, Int, String, String, String, String, String, String, String],
        String,
        Network
    ),
    native!(
        "std::email::send_with_attachment",
        [String, Int, String, String, String, String, String, String, String, String, Bytes],
        String,
        Network
    ),
    // --- Phase 4: crypto (AEAD) ---
    native!("std::crypto::generate_key_32", [], Bytes),
    native!("std::crypto::generate_nonce", [], Bytes),
    native!(
        "std::crypto::chacha20_encrypt",
        [Bytes, Bytes, Bytes, Bytes],
        Bytes
    ),
    native!(
        "std::crypto::chacha20_decrypt",
        [Bytes, Bytes, Bytes, Bytes],
        Bytes
    ),
    native!("std::crypto::chacha20_seal", [Bytes, Bytes, Bytes], Bytes),
    native!("std::crypto::chacha20_open", [Bytes, Bytes, Bytes], Bytes),
    native!(
        "std::crypto::aes_gcm_encrypt",
        [Bytes, Bytes, Bytes, Bytes],
        Bytes
    ),
    native!(
        "std::crypto::aes_gcm_decrypt",
        [Bytes, Bytes, Bytes, Bytes],
        Bytes
    ),
    native!("std::crypto::aes_gcm_seal", [Bytes, Bytes, Bytes], Bytes),
    native!("std::crypto::aes_gcm_open", [Bytes, Bytes, Bytes], Bytes),
    // --- Phase 4: password (Argon2id + bcrypt) ---
    native!("std::password::hash_argon2", [String], String),
    native!("std::password::verify_argon2", [String, String], Bool),
    native!("std::password::hash_bcrypt", [String, Int], String),
    native!("std::password::verify_bcrypt", [String, String], Bool),
    // --- Phase 4: JWT (HS256 + RS256) ---
    native!("std::jwt::sign_hs256", [Any, Bytes], String),
    native!(
        "std::jwt::verify_hs256",
        [String, Bytes, String, String],
        Any
    ),
    native!("std::jwt::sign_rs256", [Any, Bytes], String),
    native!(
        "std::jwt::verify_rs256",
        [String, Bytes, String, String],
        Any
    ),
    native!("std::jwt::peek_header", [String], Any),
    // --- Phase 5: Termux / Android integration ---
    // Every helper spawns the matching `termux-*` CLI. Needs Termux:API + `pkg install termux-api`.
    // Marked `Process` so `titan run --sandbox` blocks them consistently with `std::process::*`.
    native!("std::termux::is_available", [], Bool, Environment),
    native!("std::termux::battery_status", [], Any, Process),
    native!("std::termux::wifi_info", [], Any, Process),
    native!("std::termux::telephony_info", [], Any, Process),
    native!("std::termux::location", [String, String], Any, Process),
    native!("std::termux::sensor_list", [], Array, Process),
    native!("std::termux::sensor_read", [String], Any, Process),
    native!("std::termux::clipboard_get", [], String, Process),
    native!("std::termux::clipboard_set", [String], Nil, Process),
    native!("std::termux::vibrate", [Int, Bool], Nil, Process),
    native!("std::termux::torch", [Bool], Nil, Process),
    native!("std::termux::toast", [String], Nil, Process),
    native!("std::termux::notify", [String, String, Int], Nil, Process),
    native!("std::termux::notify_remove", [Int], Nil, Process),
    native!("std::termux::tts_speak", [String], Nil, Process),
    native!("std::termux::sms_list", [Int], Any, Process),
    native!("std::termux::sms_send", [String, String], Nil, Process),
    native!("std::termux::contacts", [], Any, Process),
    native!("std::termux::camera_info", [], Any, Process),
    native!("std::termux::camera_photo", [String, String], Nil, Process),
    native!("std::termux::brightness", [Int], Nil, Process),
    native!("std::termux::dialog", [String, String], Any, Process),
    native!("std::termux::share", [String], Nil, Process),
    // --- Phase 6: Terminal (crossterm) ---
    native!(
        "std::term::print_colored",
        [String, String],
        Nil,
        UserInterface
    ),
    native!(
        "std::term::print_styled",
        [String, String, String],
        Nil,
        UserInterface
    ),
    native!(
        "std::term::print_attr",
        [String, String],
        Nil,
        UserInterface
    ),
    native!("std::term::clear_screen", [], Nil, UserInterface),
    native!("std::term::clear_line", [], Nil, UserInterface),
    native!("std::term::move_to", [Int, Int], Nil, UserInterface),
    native!("std::term::hide_cursor", [], Nil, UserInterface),
    native!("std::term::show_cursor", [], Nil, UserInterface),
    native!("std::term::size", [], Array, UserInterface),
    native!("std::term::flush", [], Nil, UserInterface),
    native!("std::term::enter_alt_screen", [], Nil, UserInterface),
    native!("std::term::leave_alt_screen", [], Nil, UserInterface),
    native!("std::term::enable_raw", [], Nil, UserInterface),
    native!("std::term::disable_raw", [], Nil, UserInterface),
    native!("std::term::read_key", [Int], String, UserInterface),
    // --- Phase 6: Readline (rustyline) ---
    native!("std::readline::prompt", [String], String, UserInterface),
    native!(
        "std::readline::prompt_with_history",
        [String],
        String,
        UserInterface
    ),
    native!(
        "std::readline::prompt_persistent",
        [String, String],
        String,
        FilesystemUserInterface
    ),
    native!(
        "std::readline::prompt_secret",
        [String],
        String,
        UserInterface
    ),
    // --- Phase 6: Progress (indicatif) ---
    // Progress bars write to the user's terminal, so every operation is UI-gated.
    native!("std::progress::bar_new", [Int], Int, UserInterface),
    native!("std::progress::spinner_new", [], Int, UserInterface),
    native!(
        "std::progress::set_message",
        [Int, String],
        Nil,
        UserInterface
    ),
    native!(
        "std::progress::set_position",
        [Int, Int],
        Nil,
        UserInterface
    ),
    native!(
        "std::progress::increment",
        [Int, Int],
        Nil,
        UserInterface
    ),
    native!(
        "std::progress::finish",
        [Int, String],
        Nil,
        UserInterface
    ),
    native!("std::progress::abandon", [Int], Nil, UserInterface),
    // --- Phase 7: Images (image crate) ---
    // Load / save are gated with Filesystem so `--sandbox` blocks them.
    native!("std::image::load", [String], Int, Filesystem),
    native!("std::image::load_bytes", [Bytes], Int),
    native!("std::image::save", [Int, String], Nil, Filesystem),
    native!("std::image::encode", [Int, String], Bytes),
    native!("std::image::width", [Int], Int),
    native!("std::image::height", [Int], Int),
    native!("std::image::color_type", [Int], String),
    native!("std::image::resize", [Int, Int, Int, String], Int),
    native!("std::image::resize_exact", [Int, Int, Int, String], Int),
    native!("std::image::thumbnail", [Int, Int, Int], Int),
    native!("std::image::crop", [Int, Int, Int, Int, Int], Int),
    native!("std::image::grayscale", [Int], Int),
    native!("std::image::blur", [Int, Float], Int),
    native!("std::image::brighten", [Int, Int], Int),
    native!("std::image::rotate90", [Int], Int),
    native!("std::image::rotate180", [Int], Int),
    native!("std::image::rotate270", [Int], Int),
    native!("std::image::flip_horizontal", [Int], Int),
    native!("std::image::flip_vertical", [Int], Int),
    native!("std::image::close", [Int], Nil),
    // --- Phase 7: QR codes (qrcode crate) ---
    native!(
        "std::qrcode::to_ascii",
        [String, String, String, String],
        String
    ),
    native!("std::qrcode::to_unicode", [String, String], String),
    native!("std::qrcode::to_svg", [String, String, Int], Bytes),
    native!("std::qrcode::to_png", [String, String, Int], Bytes),
    native!(
        "std::qrcode::save_png",
        [String, String, Int, String],
        Nil,
        Filesystem
    ),
    // --- Phase 8: System info (sysinfo) ---
    native!("std::procfs::hostname", [], String, Environment),
    native!("std::procfs::kernel", [], String, Environment),
    native!("std::procfs::os_name", [], String, Environment),
    native!("std::procfs::os_version", [], String, Environment),
    native!("std::procfs::uptime", [], Int, Environment),
    native!("std::procfs::cpu_usage", [], Float, Environment),
    native!("std::procfs::cpu_count", [], Int, Environment),
    native!("std::procfs::cpus", [], Array, Environment),
    native!("std::procfs::total_memory", [], Int, Environment),
    native!("std::procfs::used_memory", [], Int, Environment),
    native!("std::procfs::available_memory", [], Int, Environment),
    native!("std::procfs::total_swap", [], Int, Environment),
    native!("std::procfs::used_swap", [], Int, Environment),
    native!("std::procfs::load_average", [], Map, Environment),
    native!("std::procfs::process_count", [], Int, Environment),
    native!("std::procfs::top_processes", [Int], Array, Environment),
    native!("std::procfs::disks", [], Array, Environment),
    native!("std::procfs::networks", [], Map, Environment),
    // --- Phase 8: File-system watcher (notify) ---
    native!(
        "std::fswatch::watch_once",
        [String, Int, Bool],
        String,
        Filesystem
    ),
    native!("std::fswatch::open", [String, Bool], Int, Filesystem),
    native!(
        "std::fswatch::next_event",
        [Int, Int],
        String,
        Filesystem
    ),
    native!("std::fswatch::close", [Int], Nil, Filesystem),
    // --- Phase 8: Unix signals (signal-hook) ---
    native!("std::signals::install", [String], Nil, Process),
    native!("std::signals::pending", [String], Int, Process),
    native!("std::signals::wait_any", [Int], String, Process),
    // --- Phase 9: Audio (hound + termux-media-player) ---
    // Pure-Rust WAV I/O and synthesis.
    native!("std::audio::read_wav", [String], Map, Filesystem),
    native!("std::audio::read_wav_bytes", [Bytes], Map),
    native!(
        "std::audio::write_wav",
        [String, Array, Int, Int],
        Nil,
        Filesystem
    ),
    native!("std::audio::encode_wav", [Array, Int, Int], Bytes),
    native!("std::audio::sine_wave", [Float, Int, Int, Float], Array),
    native!("std::audio::square_wave", [Float, Int, Int, Float], Array),
    native!("std::audio::saw_wave", [Float, Int, Int, Float], Array),
    native!("std::audio::white_noise", [Int, Int, Float], Array),
    // Playback / recording via termux-api. Marked Process so `--sandbox` blocks them.
    native!(
        "std::audio::is_termux_media_available",
        [],
        Bool,
        Environment
    ),
    native!("std::audio::play", [String], String, Process),
    native!("std::audio::pause", [], String, Process),
    native!("std::audio::resume", [], String, Process),
    native!("std::audio::stop", [], String, Process),
    native!("std::audio::info", [], String, Process),
    native!("std::audio::record_start", [String, Int], String, Process),
    native!("std::audio::record_stop", [], String, Process),
    native!("std::audio::record_info", [], String, Process),
    // --- Phase 10: NoSQL — embedded key-value (sled) ---
    // Path I/O -> Filesystem capability. Results that may be missing use
    // Any (returns Bytes or Nil).
    native!("std::kv::open", [String], Int, Filesystem),
    native!("std::kv::close", [Int], Nil, Filesystem),
    native!("std::kv::flush", [Int], Int, Filesystem),
    native!("std::kv::insert", [Int, Bytes, Bytes], Any, Filesystem),
    native!("std::kv::get", [Int, Bytes], Any, Filesystem),
    native!("std::kv::remove", [Int, Bytes], Any, Filesystem),
    native!("std::kv::contains", [Int, Bytes], Bool, Filesystem),
    native!("std::kv::len", [Int], Int, Filesystem),
    native!("std::kv::clear", [Int], Nil, Filesystem),
    native!("std::kv::keys", [Int], Array, Filesystem),
    // compare_and_swap: pass empty bytes for None (expected / new).
    native!(
        "std::kv::compare_and_swap",
        [Int, Bytes, Bytes, Bytes],
        Bool,
        Filesystem
    ),
    native!("std::kv::open_tree", [Int, String], Int, Filesystem),
    native!(
        "std::kv::tree_insert",
        [Int, Bytes, Bytes],
        Any,
        Filesystem
    ),
    native!("std::kv::tree_get", [Int, Bytes], Any, Filesystem),
    native!(
        "std::kv::tree_remove",
        [Int, Bytes],
        Any,
        Filesystem
    ),
    native!("std::kv::tree_len", [Int], Int, Filesystem),
    native!("std::kv::tree_keys", [Int], Array, Filesystem),
    // --- Phase 10: Redis client ---
    // Network capability across the board.
    native!("std::redis::connect", [String], Int, Network),
    native!("std::redis::close", [Int], Nil, Network),
    native!("std::redis::ping", [Int], String, Network),
    native!("std::redis::set", [Int, String, String], Nil, Network),
    native!(
        "std::redis::set_ex",
        [Int, String, String, Int],
        Nil,
        Network
    ),
    native!("std::redis::get", [Int, String], Any, Network),
    native!("std::redis::del", [Int, String], Int, Network),
    native!("std::redis::exists", [Int, String], Bool, Network),
    native!("std::redis::expire", [Int, String, Int], Bool, Network),
    native!("std::redis::ttl", [Int, String], Int, Network),
    native!("std::redis::incr", [Int, String, Int], Int, Network),
    native!("std::redis::keys", [Int, String], Array, Network),
    native!("std::redis::lpush", [Int, String, String], Int, Network),
    native!("std::redis::rpush", [Int, String, String], Int, Network),
    native!(
        "std::redis::lrange",
        [Int, String, Int, Int],
        Array,
        Network
    ),
    native!("std::redis::llen", [Int, String], Int, Network),
    native!(
        "std::redis::hset",
        [Int, String, String, String],
        Nil,
        Network
    ),
    native!("std::redis::hget", [Int, String, String], Any, Network),
    native!("std::redis::hdel", [Int, String, String], Int, Network),
    native!("std::redis::hgetall", [Int, String], Array, Network),
    native!("std::redis::raw", [Int, String], String, Network),
    // --- Phase 11: bounded HTTP/1.1 + WebSocket server (std::net) ---
    // Any operation that can touch a socket requires Network.
    native!("std::server::start", [String], Int, Network),
    native!("std::server::local_addr", [Int], String, Network),
    native!("std::server::accept", [Int, Int], Int, Network),
    native!("std::server::stop", [Int], Nil, Network),
    native!("std::server::method", [Int], String, Network),
    native!("std::server::url", [Int], String, Network),
    native!("std::server::path", [Int], String, Network),
    native!("std::server::query", [Int], String, Network),
    native!("std::server::remote_addr", [Int], String, Network),
    native!("std::server::header", [Int, String], Any, Network),
    native!("std::server::headers", [Int], Map, Network),
    native!("std::server::body", [Int], Bytes, Network),
    native!("std::server::body_text", [Int], String, Network),
    native!("std::server::respond", [Int, Int, String], Nil, Network),
    native!(
        "std::server::respond_html",
        [Int, Int, String],
        Nil,
        Network
    ),
    native!(
        "std::server::respond_json",
        [Int, Int, String],
        Nil,
        Network
    ),
    native!(
        "std::server::respond_bytes",
        [Int, Int, String, Bytes],
        Nil,
        Network
    ),
    // respond_full: (req, status, content_type, headers-map, body-bytes)
    native!(
        "std::server::respond_full",
        [Int, Int, String, Map, Bytes],
        Nil,
        Network
    ),
    // WebSocket upgrade + I/O.
    native!("std::server::upgrade_websocket", [Int, Int], Int, Network),
    // ws_recv returns [kind, text, bytes] as an array of 3 elements.
    native!("std::server::ws_recv", [Int], Array, Network),
    native!("std::server::ws_send_text", [Int, String], Nil, Network),
    native!("std::server::ws_send_binary", [Int, Bytes], Nil, Network),
    native!("std::server::ws_close", [Int, Int, String], Nil, Network),
    // --- Phase 11: URL router (matchit, same one axum uses) ---
    native!("std::router::new", [], Int),
    native!("std::router::drop", [Int], Nil),
    native!("std::router::insert", [Int, String, String], Nil),
    // at() returns { "pattern": String, "params": Map<String,String> } or Nil.
    native!("std::router::at", [Int, String], Any),
    native!("std::router::matches", [Int, String], Bool),
    // --- Phase 14: SVG charts (plotters, pure Rust, no ttf/C-deps) ---
    // All write a standalone .svg to `path`. `xs`/`ys` are float arrays.
    native!(
        "std::plot::line",
        [String, String, String, String, Array, Array],
        Nil,
        Filesystem
    ),
    // multi_line: 3 parallel arrays — labels, list-of-xs-arrays, list-of-ys-arrays.
    native!(
        "std::plot::multi_line",
        [String, String, String, String, Array, Array, Array],
        Nil,
        Filesystem
    ),
    native!(
        "std::plot::bar",
        [String, String, String, Array, Array],
        Nil,
        Filesystem
    ),
    native!(
        "std::plot::scatter",
        [String, String, String, String, Array, Array],
        Nil,
        Filesystem
    ),
    native!(
        "std::plot::histogram",
        [String, String, String, Array, Int],
        Nil,
        Filesystem
    ),
    // --- Phase 12: HuggingFace tokenizers (pure Rust, no C++) ---
    // load() opens a HuggingFace tokenizer.json from disk; from_json()
    // takes the raw JSON string. Both return an opaque i64 handle.
    native!("std::tokenize::load", [String], Int, Filesystem),
    native!("std::tokenize::from_json", [String], Int),
    native!("std::tokenize::close", [Int], Nil),
    native!("std::tokenize::vocab_size", [Int], Int),
    // encode returns { ids, tokens, type_ids, attention_mask, special_tokens_mask } map.
    native!("std::tokenize::encode", [Int, String, Bool], Map),
    // encode_padded: encode + pad-to-max_length or truncate. For BERT-family
    // models compiled with a fixed [batch, seq_len] input shape.
    // args: (handle, text, max_length, pad_id, add_special_tokens) -> map.
    native!(
        "std::tokenize::encode_padded",
        [Int, String, Int, Int, Bool],
        Map
    ),
    // encode_batch takes an array of strings, returns an array of the same map shape.
    native!("std::tokenize::encode_batch", [Int, Array, Bool], Array),
    // decode: array of ids -> string.
    native!("std::tokenize::decode", [Int, Array, Bool], String),
    // Lookups: string <-> id (returns Any because either can be Nil when absent).
    native!("std::tokenize::token_to_id", [Int, String], Any),
    native!("std::tokenize::id_to_token", [Int, Int], Any),
    // --- Phase 12 part 2: ONNX inference via tract (pure-Rust) ---
    // load()      → optimizes and prepares a model without pinning shapes.
    // load_shape()→ pins the first input to the given shape before optimize
    //               (needed for models with dynamic axes).
    native!("std::onnx::load", [String], Int, Filesystem),
    native!("std::onnx::load_shape", [String, Array], Int, Filesystem),
    native!("std::onnx::close", [Int], Nil),
    native!("std::onnx::input_count", [Int], Int),
    native!("std::onnx::output_count", [Int], Int),
    // input_shape(handle, i) / output_shape(handle, i) → [Int] (may contain -1 for symbolic).
    native!("std::onnx::input_shape", [Int, Int], Array),
    native!("std::onnx::output_shape", [Int, Int], Array),
    // run_f32(handle, shape, data) → { values: [Float], shape: [Int] }
    native!("std::onnx::run_f32", [Int, Array, Array], Map),
    // run_ids(handle, shape, ids)  → same shape, for token-id inputs (BERT-family).
    native!("std::onnx::run_ids", [Int, Array, Array], Map),
    // BERT-style loaders (v0.13.0): pin (batch, seq_len) on 2 or 3 int64 inputs.
    native!("std::onnx::load_bert", [String, Int, Int], Int, Filesystem),
    native!("std::onnx::load_bert3", [String, Int, Int], Int, Filesystem),
    // run_bert(handle, shape, input_ids, attention_mask)
    native!("std::onnx::run_bert", [Int, Array, Array, Array], Map),
    // run_bert3(handle, shape, input_ids, attention_mask, token_type_ids)
    native!(
        "std::onnx::run_bert3",
        [Int, Array, Array, Array, Array],
        Map
    ),
    // run_bert_pooled: sentence-transformer style — runs the encoder
    // and mean-pools the token embeddings weighted by attention_mask.
    // args: (handle, batch, seq_len, input_ids, attention_mask)
    // returns { values: [Float len=batch*hidden], shape: [batch, hidden] }.
    native!(
        "std::onnx::run_bert_pooled",
        [Int, Int, Int, Array, Array],
        Map
    ),
    // --- Phase 12 pt.4: vector math (embeddings, semantic search) ---
    native!("std::vector::dot", [Array, Array], Float),
    native!("std::vector::norm", [Array], Float),
    native!("std::vector::cosine_similarity", [Array, Array], Float),
    native!("std::vector::normalize", [Array], Array),
    native!("std::vector::add", [Array, Array], Array),
    native!("std::vector::sub", [Array, Array], Array),
    native!("std::vector::scale", [Array, Float], Array),
    native!("std::vector::argmax", [Array], Int),
    // --- Phase 16: PDF generation (printpdf, pure Rust) ---
    // Documents are opaque i64 handles. Pages / layers by 0-based index.
    // All coords in millimetres, PDF-space (y grows upwards).
    native!("std::pdf::new", [String, Float, Float], Int),
    native!("std::pdf::add_page", [Int, Float, Float, String], Int),
    native!("std::pdf::page_count", [Int], Int),
    native!(
        "std::pdf::add_text",
        [Int, Int, Int, String, Float, Float, Float],
        Nil
    ),
    native!(
        "std::pdf::set_color",
        [Int, Int, Int, Float, Float, Float],
        Nil
    ),
    native!(
        "std::pdf::add_line",
        [Int, Int, Int, Float, Float, Float, Float, Float],
        Nil
    ),
    native!(
        "std::pdf::add_rect",
        [Int, Int, Int, Float, Float, Float, Float],
        Nil
    ),
    native!("std::pdf::save", [Int, String], Nil, Filesystem),
    native!("std::pdf::close", [Int], Nil),
    // --- Phase 13': Wi-Fi introspection (Termux:API) ---
    // Shell out to termux-wifi-*. Requires the termux-api package and
    // the Termux:API app installed on the device (same as Fase 5).
    // scan() → array of {ssid, bssid, rssi, frequency_mhz, timestamp,
    //          channel_bandwidth_mhz, center_frequency_mhz}
    native!("std::wifi::scan", [], Array, Network),
    // connection_info() → { ssid, bssid, ip, mac_address, ... } or Nil.
    native!("std::wifi::connection_info", [], Any, Network),
    // set_enabled(bool) — toggle the Wi-Fi radio.
    native!("std::wifi::set_enabled", [Bool], Nil, Network),
    // signal_bars(rssi_dbm) → 0..=4  (pure fn, no CLI, safe on any host).
    native!("std::wifi::signal_bars", [Int], Int),
    // --- Phase 1: dirs ---
    native!("std::dirs::home", [], String, Environment),
    native!("std::dirs::config", [], String, Environment),
    native!("std::dirs::cache", [], String, Environment),
    native!("std::dirs::data", [], String, Environment),
    native!("std::dirs::data_local", [], String, Environment),
    native!("std::dirs::state", [], String, Environment),
    native!("std::dirs::executable", [], String, Environment),
    native!("std::dirs::runtime", [], String, Environment),
    native!("std::dirs::preference", [], String, Environment),
    native!("std::dirs::desktop", [], String, Environment),
    native!("std::dirs::documents", [], String, Environment),
    native!("std::dirs::downloads", [], String, Environment),
    native!("std::dirs::pictures", [], String, Environment),
    native!("std::dirs::music", [], String, Environment),
    native!("std::dirs::videos", [], String, Environment),
    native!("std::dirs::public", [], String, Environment),
    native!("std::dirs::temp", [], String, Environment),
    native!("std::dirs::current", [], String, Environment),
    // ------------------- Phase 34: std::process -------------------
    native!("std::process::run", [String], Map, Process),
    native!(
        "std::process::run_with_input",
        [String, Bytes],
        Map,
        Process
    ),
    native!("std::process::shell", [String], Map, Process),
    native!("std::process::pipe", [Array], Map, Process),
    native!("std::process::spawn", [String], Int, Process),
    native!("std::process::spawn_wait", [Int], Map, Process),
    native!("std::process::spawn_poll", [Int], Any, Process),
    native!("std::process::spawn_kill", [Int], Nil, Process),
    native!("std::process::spawn_pid", [Int], Int, Process),
    native!("std::process::env_get", [String], Any, Environment),
    native!("std::process::env_set", [String, String], Nil, Environment),
    native!("std::process::env_unset", [String], Nil, Environment),
    native!("std::process::env_vars", [], Array, Environment),
    native!("std::process::working_dir", [], String, Filesystem),
    native!("std::process::set_working_dir", [String], Nil, Filesystem),
    native!("std::process::self_pid", [], Int),
    native!("std::process::hostname", [], String, Environment),
    native!("std::process::username", [], String, Environment),
    native!("std::process::args", [], Array, Environment),
    native!("std::process::send_signal", [Int, Int], Nil, Process),
    native!("std::process::exit", [Int], Nil, Process),
    // ------------------- Phase 34: std::collections -------------------
    // Set
    native!("std::collections::set_new", [], Int),
    native!("std::collections::set_from", [Array], Int),
    native!("std::collections::set_add", [Int, String], Bool),
    native!("std::collections::set_remove", [Int, String], Bool),
    native!("std::collections::set_contains", [Int, String], Bool),
    native!("std::collections::set_len", [Int], Int),
    native!("std::collections::set_to_array", [Int], Array),
    native!("std::collections::set_union", [Int, Int], Int),
    native!("std::collections::set_intersect", [Int, Int], Int),
    native!("std::collections::set_difference", [Int, Int], Int),
    native!("std::collections::set_is_subset", [Int, Int], Bool),
    native!("std::collections::set_drop", [Int], Bool),
    // Deque
    native!("std::collections::deque_new", [], Int),
    native!("std::collections::deque_push_front", [Int, String], Nil),
    native!("std::collections::deque_push_back", [Int, String], Nil),
    native!("std::collections::deque_pop_front", [Int], Any),
    native!("std::collections::deque_pop_back", [Int], Any),
    native!("std::collections::deque_len", [Int], Int),
    native!("std::collections::deque_to_array", [Int], Array),
    native!("std::collections::deque_drop", [Int], Bool),
    // PriorityQueue
    native!("std::collections::pq_new_max", [], Int),
    native!("std::collections::pq_new_min", [], Int),
    native!("std::collections::pq_push", [Int, String, Int], Nil),
    native!("std::collections::pq_pop", [Int], Any),
    native!("std::collections::pq_peek", [Int], Any),
    native!("std::collections::pq_len", [Int], Int),
    native!("std::collections::pq_drop", [Int], Bool),
    // OrderedMap
    native!("std::collections::omap_new", [], Int),
    native!("std::collections::omap_insert", [Int, String, Any], Nil),
    native!("std::collections::omap_get", [Int, String], Any),
    native!("std::collections::omap_remove", [Int, String], Bool),
    native!("std::collections::omap_keys", [Int], Array),
    native!("std::collections::omap_len", [Int], Int),
    native!("std::collections::omap_drop", [Int], Bool),
    // Counter
    native!("std::collections::counter_from", [Array], Int),
    native!("std::collections::counter_add", [Int, String, Int], Nil),
    native!("std::collections::counter_count", [Int, String], Int),
    native!("std::collections::counter_most_common", [Int, Int], Array),
    native!("std::collections::counter_total", [Int], Int),
    native!("std::collections::counter_drop", [Int], Bool),
    // Graph
    native!("std::collections::graph_new", [Bool], Int),
    native!("std::collections::graph_add_node", [Int, String], Nil),
    native!(
        "std::collections::graph_add_edge",
        [Int, String, String, Int],
        Nil
    ),
    native!("std::collections::graph_neighbors", [Int, String], Array),
    native!("std::collections::graph_bfs", [Int, String], Array),
    native!("std::collections::graph_dfs", [Int, String], Array),
    native!(
        "std::collections::graph_shortest_path",
        [Int, String, String],
        Array
    ),
    native!("std::collections::graph_topological_sort", [Int], Array),
    native!("std::collections::graph_has_cycle", [Int], Bool),
    native!("std::collections::graph_nodes", [Int], Array),
    native!("std::collections::graph_drop", [Int], Bool),
    // ------------------- Phase 34: std::datetime extendido -------------------
    // Componentes (year/month/day/hour/minute/second ya están declarados en la
    // sección de datetime_mod arriba con firmas idénticas — no repetir aquí:
    // lookup() devuelve la primera coincidencia y los duplicados solo ensucian).
    native!("std::datetime::day_of_week", [Int], Int),
    native!("std::datetime::day_of_year", [Int], Int),
    native!("std::datetime::week_of_year", [Int], Int),
    native!("std::datetime::quarter", [Int], Int),
    native!("std::datetime::is_leap_year", [Int], Bool),
    native!("std::datetime::days_in_month", [Int, Int], Int),
    // Aritmética (add_seconds y diff_seconds ya están en datetime_mod arriba)
    native!("std::datetime::add_minutes", [Int, Int], Int),
    native!("std::datetime::add_hours", [Int, Int], Int),
    native!("std::datetime::add_days_ext", [Int, Int], Int),
    native!("std::datetime::add_weeks", [Int, Int], Int),
    native!("std::datetime::add_months", [Int, Int], Int),
    native!("std::datetime::add_years", [Int, Int], Int),
    native!("std::datetime::diff_minutes", [Int, Int], Int),
    native!("std::datetime::diff_hours", [Int, Int], Int),
    native!("std::datetime::diff_days", [Int, Int], Int),
    // Comparaciones
    native!("std::datetime::is_before", [Int, Int], Bool),
    native!("std::datetime::is_after", [Int, Int], Bool),
    native!("std::datetime::is_same_day", [Int, Int], Bool),
    // Timezones
    native!("std::datetime::to_timezone", [Int, String], String),
    native!("std::datetime::timezone_offset_seconds", [Int, String], Int),
    native!("std::datetime::common_timezones", [], Array),
    // ISO / RFC
    native!("std::datetime::to_iso", [Int], String),
    native!("std::datetime::from_iso", [String], Int),
    // Humano
    native!("std::datetime::humanize", [Int, Int], String),
    native!("std::datetime::is_weekend", [Int], Bool),
    native!("std::datetime::business_days_between", [Int, Int], Int),
    native!("std::datetime::next_weekday", [Int, Int], Int),
    // Construcción / formato
    native!("std::datetime::from_ymd", [Int, Int, Int], Int),
    native!(
        "std::datetime::from_ymd_hms",
        [Int, Int, Int, Int, Int, Int],
        Int
    ),
    native!("std::datetime::range_ext", [Int, Int, Int], Array),
];

pub fn lookup(name: &str) -> Option<&'static NativeSignature> {
    NATIVES.iter().find(|signature| signature.name == name)
}
pub fn contains(name: &str) -> bool {
    lookup(name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_names_are_unique_and_qualified() {
        let mut names = HashSet::new();
        for signature in NATIVES {
            assert!(signature.name.starts_with("std::"));
            assert!(
                names.insert(signature.name),
                "duplicate native {}",
                signature.name
            );
        }
        assert!(NATIVES.len() >= 80);
    }

    #[test]
    fn effectful_native_capabilities_are_declared() {
        assert_eq!(
            lookup("std::http::request").unwrap().capability,
            Capability::Network
        );
        for signature in NATIVES
            .iter()
            .filter(|signature| signature.name.starts_with("std::server::"))
        {
            assert_eq!(
                signature.capability,
                Capability::Network,
                "{}",
                signature.name
            );
        }
        for (prefix, capability) in [
            ("std::redis::", Capability::Network),
            ("std::kv::", Capability::Filesystem),
            ("std::fswatch::", Capability::Filesystem),
            ("std::progress::", Capability::UserInterface),
        ] {
            for signature in NATIVES
                .iter()
                .filter(|signature| signature.name.starts_with(prefix))
            {
                assert_eq!(signature.capability, capability, "{}", signature.name);
            }
        }
        assert_eq!(
            lookup("std::readline::prompt_persistent")
                .unwrap()
                .capability,
            Capability::FilesystemUserInterface,
        );
        assert_eq!(
            lookup("std::clipboard::get_text").unwrap().capability,
            Capability::UserInterface
        );
        assert_eq!(
            lookup("std::game::check_collision").unwrap().capability,
            Capability::None
        );
    }

    #[test]
    fn runtime_context_is_scoped_and_restored() {
        assert_eq!(runtime_handle_key(9), (0, 9));
        with_runtime_context(41, || {
            assert_eq!(runtime_handle_key(9), (41, 9));
            with_runtime_context(42, || assert_eq!(runtime_handle_key(9), (42, 9)));
            assert_eq!(runtime_handle_key(9), (41, 9));
        });
        assert_eq!(runtime_handle_key(9), (0, 9));
    }
    #[test]
    fn mutable_service_state_is_isolated_and_selectively_cleaned_per_runtime() {
        let first = 80_001;
        let second = 80_002;
        let (audio_handle, root, button) = with_runtime_context(first, || {
            assert!(crate::audio::init());
            let audio_handle = crate::audio::load_wave(440.0, 10);
            assert!(crate::gui::init());
            let root = crate::gui::create_container("first", 320, 240);
            let button = crate::gui::add_button(root, "private", 0, 0, 10, 10);
            assert!(crate::gui::trigger_click(button));
            assert!(crate::input::set_key_state("PrivateKey", true));
            assert!(crate::input::set_mouse_pos(17, 29));
            assert!(crate::clipboard::set_text("first secret"));
            assert!(crate::clipboard::send_notification("first", "private"));
            assert!(crate::mobile::trigger_event("onPause"));
            assert!(crate::game::init("first game", 320, 240));
            crate::metrics::counter_add("private.counter", 9).unwrap();
            (audio_handle, root, button)
        });

        with_runtime_context(second, || {
            assert_eq!(crate::audio::sample_count(audio_handle), 0);
            assert_eq!(crate::gui::get_text(root), "");
            assert!(!crate::gui::is_clicked(button));
            assert!(!crate::input::is_key_pressed("PrivateKey"));
            assert_eq!(crate::input::mouse_pos(), (0, 0));
            assert_eq!(crate::clipboard::get_text(), "");
            assert!(crate::clipboard::poll_notifications().is_empty());
            assert_eq!(crate::mobile::get_state(), "Running");
            assert!(crate::mobile::poll_events().is_empty());
            assert_eq!(crate::game::step(), 0.0);
            assert_eq!(crate::metrics::counter_get("private.counter").unwrap(), 0);

            assert!(crate::audio::shutdown());
            assert!(crate::gui::shutdown());
            assert!(!crate::game::shutdown());
            assert!(crate::input::set_key_state("PrivateKey", false));
            assert!(crate::clipboard::set_text("second"));
            assert!(crate::mobile::trigger_event("onDestroy"));
            crate::metrics::counter_add("private.counter", 2).unwrap();
            crate::metrics::reset().unwrap();
        });
        assert_eq!(cleanup_runtime_resources(second), 6);

        with_runtime_context(first, || {
            assert!(crate::audio::sample_count(audio_handle) > 0);
            assert_eq!(crate::gui::get_text(root), "first");
            assert!(crate::gui::is_clicked(button));
            assert!(crate::input::is_key_pressed("PrivateKey"));
            assert_eq!(crate::input::mouse_pos(), (17, 29));
            assert_eq!(crate::clipboard::get_text(), "first secret");
            assert_eq!(
                crate::clipboard::poll_notifications(),
                vec![("first".to_string(), "private".to_string())]
            );
            assert_eq!(crate::mobile::get_state(), "Paused");
            assert_eq!(crate::mobile::poll_events(), vec!["onPause".to_string()]);
            assert!(crate::game::step() > 0.0);
            assert_eq!(crate::metrics::counter_get("private.counter").unwrap(), 9);
        });
        assert_eq!(cleanup_runtime_resources(first), 7);
    }

    #[test]
    fn freestanding_emulator_state_is_isolated_and_selectively_cleaned_per_runtime() {
        let first = 81_001;
        let second = 81_002;
        with_runtime_context(first, || {
            assert!(crate::freestanding::init("aarch64-unknown-none"));
            assert!(crate::freestanding_memory::init_frame_allocator(
                0x10_0000, 0x4000
            ));
            let frame = crate::freestanding_memory::allocate_frame();
            assert_eq!(frame, 0x10_0000);
            assert!(crate::freestanding_memory::map_page(0x40_0000, frame, 3));
            assert!(crate::freestanding_cpu::init_exception_table(0x8000_0000));
            assert!(crate::freestanding_cpu::register_exception_handler(
                0, 0x9000
            ));
            assert_ne!(crate::freestanding_cpu::dispatch_exception(0, 0x1234, 5), 0);
            assert!(crate::freestanding_mmio::init_mmio_region(
                0x3f00_0000,
                0x1000
            ));
            assert!(crate::freestanding_mmio::write_mmio_u32(
                0x3f00_0004,
                0xdead_beef
            ));
            assert!(crate::freestanding_mmio::serial_init(0x1000_0000, 115_200));
            assert_eq!(crate::freestanding_mmio::serial_write_str("private"), 7);
        });

        with_runtime_context(second, || {
            assert_eq!(crate::freestanding::get_active_target(), "");
            assert_eq!(crate::freestanding_memory::free_frames_count(), 0);
            assert_eq!(crate::freestanding_memory::translate_page(0x40_0000), 0);
            assert_eq!(crate::freestanding_cpu::get_last_fault_addr(), 0);
            assert_eq!(crate::freestanding_mmio::read_mmio_u32(0x3f00_0004), 0);
            assert_eq!(crate::freestanding_mmio::serial_get_buffer(), "");
            assert!(crate::freestanding::shutdown());
            assert!(crate::freestanding_memory::shutdown());
            assert!(crate::freestanding_cpu::shutdown());
            assert!(crate::freestanding_mmio::shutdown());
        });
        assert_eq!(cleanup_runtime_resources(second), 4);

        with_runtime_context(first, || {
            assert_eq!(
                crate::freestanding::get_active_target(),
                "aarch64-unknown-none"
            );
            assert_eq!(crate::freestanding_memory::free_frames_count(), 3);
            assert_eq!(
                crate::freestanding_memory::translate_page(0x40_0042),
                0x10_0042
            );
            assert_eq!(crate::freestanding_cpu::get_last_fault_addr(), 0x1234);
            assert_eq!(
                crate::freestanding_mmio::read_mmio_u32(0x3f00_0004),
                0xdead_beef
            );
            assert_eq!(crate::freestanding_mmio::serial_get_buffer(), "private");
        });
        assert_eq!(cleanup_runtime_resources(first), 4);
    }
}
