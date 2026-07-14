//! Stable metadata for functions callable from Titan bytecode.
//!
//! Implementations live in `titan_vm`, where host values can be converted to
//! VM values. Keeping names/signatures here gives type checking and codegen one
//! authoritative registry without introducing a crate dependency cycle.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeType { Any, Int, Float, Bool, String, Bytes, Array, Map, Nil }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability { None, Filesystem, Process, Network, Environment }

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
    native!("std::text::length", [String], Int), native!("std::text::reverse", [String], String),
    native!("std::text::uppercase", [String], String), native!("std::text::lowercase", [String], String),
    native!("std::text::trim", [String], String), native!("std::text::capitalize", [String], String),
    native!("std::text::escape_html", [String], String), native!("std::text::slugify", [String], String),
    native!("std::text::levenshtein", [String, String], Int), native!("std::text::contains", [String, String], Bool),
    native!("std::text::starts_with", [String, String], Bool), native!("std::text::ends_with", [String, String], Bool),
    native!("std::text::replace", [String, String, String], String), native!("std::text::truncate", [String, Int, String], String),
    native!("std::text::words", [String], Array), native!("std::text::lines", [String], Array),

    native!("std::encoding::hex_encode", [Bytes], String), native!("std::encoding::hex_decode", [String], Bytes),
    native!("std::encoding::base64_encode", [Bytes], String), native!("std::encoding::base64_decode", [String], Bytes),
    native!("std::encoding::percent_encode", [String], String), native!("std::encoding::percent_decode", [String], String),
    native!("std::encoding::utf8_encode", [String], Bytes), native!("std::encoding::utf8_decode", [Bytes], String),

    native!("std::checksum::fnv1a64", [Bytes], Int), native!("std::checksum::crc32", [Bytes], Int),
    native!("std::checksum::constant_time_eq", [Bytes, Bytes], Bool),
    native!("std::bytes::from_array", [Array], Bytes), native!("std::bytes::to_array", [Bytes], Array),
    native!("std::bytes::length", [Bytes], Int), native!("std::bytes::concat", [Bytes, Bytes], Bytes),
    native!("std::bytes::slice", [Bytes, Int, Int], Bytes), native!("std::bytes::read_u32_le", [Bytes, Int], Int),
    native!("std::bytes::write_u32_le", [Int], Bytes),

    native!("std::http::parse_request", [Bytes], Any), native!("std::http::build_response", [Int, Map, Bytes, Bool], Bytes),
    native!("std::http::reason_phrase", [Int], String),
    native!("std::http::route_match", [String, String], Any), native!("std::http::parse_query", [String, Int], Map),
    native!("std::http::security_headers", [Map], Map), native!("std::http::cors", [Map, String, String], Map),
    native!("std::http::request_id", [Map], Map), native!("std::http::rate_limit", [String, Int, Int], Bool),
    native!("std::http::json_response", [Int, Any], Map), native!("std::http::error_response", [Int, String], Map),
    native!("std::ws::accept_key", [String], String), native!("std::ws::upgrade_response", [String, String], Bytes),
    native!("std::ws::validate_upgrade", [Map, String], Bytes), native!("std::ws::validate_accept", [Bytes, String], Bool),
    native!("std::ws::encode", [Int, Bytes, Bool], Bytes), native!("std::ws::parse", [Bytes, Bool, Int], Any),
    native!("std::csv::parse", [String], Array), native!("std::csv::serialize", [Array], String),
    native!("std::json::parse", [String], Any), native!("std::json::stringify", [Any], String),
    native!("std::json::pretty", [Any], String), native!("std::json::pointer", [Any, String], Any),
    native!("std::json::merge", [Any, Any], Any), native!("std::json::flatten", [Any], Array),

    native!("std::collections::length", [Any], Int), native!("std::collections::contains", [Array, Any], Bool),
    native!("std::collections::reverse", [Array], Array), native!("std::collections::deduplicate", [Array], Array),
    native!("std::collections::join", [Array, String], String), native!("std::collections::chunk", [Array, Int], Array),
    native!("std::map::keys", [Map], Array), native!("std::map::values", [Map], Array),
    native!("std::map::contains", [Map, String], Bool), native!("std::map::get", [Map, String], Any),
    native!("std::map::insert", [Map, String, Any], Map), native!("std::map::remove", [Map, String], Map),

    native!("std::math::sqrt", [Float], Float), native!("std::math::pow", [Float, Float], Float),
    native!("std::math::sin", [Float], Float), native!("std::math::cos", [Float], Float),
    native!("std::math::tan", [Float], Float), native!("std::math::ln", [Float], Float),
    native!("std::math::abs", [Float], Float), native!("std::math::floor", [Float], Float),
    native!("std::math::ceil", [Float], Float), native!("std::math::round", [Float], Float),
    native!("std::stats::mean", [Array], Float), native!("std::stats::median", [Array], Float),
    native!("std::stats::quantile", [Array, Float], Float), native!("std::stats::variance", [Array], Float),
    native!("std::stats::stddev", [Array], Float),

    native!("std::time::unix_seconds", [], Int), native!("std::time::unix_millis", [], Int),
    native!("std::time::sleep_ms", [Int], Nil),

    native!("std::path::join", [String, String], String), native!("std::path::normalize", [String], String),
    native!("std::path::parent", [String], String), native!("std::path::file_name", [String], String),
    native!("std::path::stem", [String], String), native!("std::path::extension", [String], String),
    native!("std::path::absolute", [String], String, Filesystem), native!("std::path::canonical", [String], String, Filesystem),

    native!("std::fs::read_text", [String], String, Filesystem), native!("std::fs::read_bytes", [String], Bytes, Filesystem),
    native!("std::fs::write_text", [String, String], Nil, Filesystem), native!("std::fs::write_bytes", [String, Bytes], Nil, Filesystem),
    native!("std::fs::atomic_write", [String, Bytes], Nil, Filesystem), native!("std::fs::append", [String, Bytes], Nil, Filesystem),
    native!("std::fs::exists", [String], Bool, Filesystem), native!("std::fs::is_file", [String], Bool, Filesystem),
    native!("std::fs::is_dir", [String], Bool, Filesystem), native!("std::fs::create_dir", [String], Nil, Filesystem),
    native!("std::fs::remove_file", [String], Nil, Filesystem), native!("std::fs::remove_dir", [String], Nil, Filesystem),
    native!("std::fs::list_dir", [String], Array, Filesystem), native!("std::fs::file_size", [String], Int, Filesystem),
    native!("std::fs::copy", [String, String], Int, Filesystem), native!("std::fs::rename", [String, String], Nil, Filesystem),

    native!("std::process::run", [String, Array], Map, Process),
    native!("std::process::run_timeout", [String, Array, Int], Map, Process),
    native!("std::env::get", [String], String, Environment), native!("std::env::args", [], Array, Environment),
    native!("std::env::current_dir", [], String, Environment),
    native!("std::net::http_get", [String], Map, Network),
    native!("std::testing::assert", [Bool, String], Nil), native!("std::testing::assert_eq", [Any, Any, String], Nil),
];

pub fn lookup(name: &str) -> Option<&'static NativeSignature> { NATIVES.iter().find(|signature| signature.name == name) }
pub fn contains(name: &str) -> bool { lookup(name).is_some() }

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_names_are_unique_and_qualified() {
        let mut names = HashSet::new();
        for signature in NATIVES {
            assert!(signature.name.starts_with("std::"));
            assert!(names.insert(signature.name), "duplicate native {}", signature.name);
        }
        assert!(NATIVES.len() >= 80);
    }
}
