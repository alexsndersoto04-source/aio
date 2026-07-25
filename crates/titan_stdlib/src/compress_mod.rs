//! Real Gzip / Deflate / Zstandard compression (`std::compress::*`).
//!
//! Backed by `flate2` (miniz-oxide, pure Rust) and `zstd` (compiles cleanly
//! on Termux AArch64). Every function operates on `Vec<u8>` so `.titan`
//! code can pipe bytes straight from `std::io::read`, `std::hash::*`, or
//! `std::encoding::*`.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompressError {
    #[error("compression I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Zstd error: {0}")]
    Zstd(String),
    #[error("compression level must be between 0 and 9 (or 1..=22 for zstd), got {0}")]
    Level(i32),
}

fn level(value: i32, max: i32) -> Result<Compression, CompressError> {
    if value < 0 || value > max { return Err(CompressError::Level(value)); }
    Ok(Compression::new(value as u32))
}

// ---------------- Gzip ----------------

pub fn gzip_encode(data: &[u8], compression_level: i32) -> Result<Vec<u8>, CompressError> {
    let mut encoder = GzEncoder::new(Vec::new(), level(compression_level, 9)?);
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn gzip_decode(data: &[u8]) -> Result<Vec<u8>, CompressError> {
    let mut decoder = GzDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}

// ---------------- Zlib (RFC 1950) ----------------

pub fn zlib_encode(data: &[u8], compression_level: i32) -> Result<Vec<u8>, CompressError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), level(compression_level, 9)?);
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn zlib_decode(data: &[u8]) -> Result<Vec<u8>, CompressError> {
    let mut decoder = ZlibDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}

// ---------------- Raw Deflate (RFC 1951) ----------------

pub fn deflate_encode(data: &[u8], compression_level: i32) -> Result<Vec<u8>, CompressError> {
    let mut encoder = DeflateEncoder::new(Vec::new(), level(compression_level, 9)?);
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn deflate_decode(data: &[u8]) -> Result<Vec<u8>, CompressError> {
    let mut decoder = DeflateDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}

// ---------------- Zstandard ----------------

pub fn zstd_encode(data: &[u8], compression_level: i32) -> Result<Vec<u8>, CompressError> {
    if !(1..=22).contains(&compression_level) { return Err(CompressError::Level(compression_level)); }
    zstd::stream::encode_all(data, compression_level).map_err(|error| CompressError::Zstd(error.to_string()))
}

pub fn zstd_decode(data: &[u8]) -> Result<Vec<u8>, CompressError> {
    zstd::stream::decode_all(data).map_err(|error| CompressError::Zstd(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"Rust + TITAN compression demo. Rust + TITAN compression demo. Rust + TITAN compression demo.";

    #[test]
    fn gzip_round_trip_and_compresses() {
        let compressed = gzip_encode(SAMPLE, 6).unwrap();
        assert!(compressed.len() < SAMPLE.len(), "gzip should shrink repetitive text");
        assert_eq!(gzip_decode(&compressed).unwrap(), SAMPLE);
    }

    #[test]
    fn zlib_round_trip() {
        let compressed = zlib_encode(SAMPLE, 9).unwrap();
        assert_eq!(zlib_decode(&compressed).unwrap(), SAMPLE);
    }

    #[test]
    fn deflate_round_trip() {
        let compressed = deflate_encode(SAMPLE, 3).unwrap();
        assert_eq!(deflate_decode(&compressed).unwrap(), SAMPLE);
    }

    #[test]
    fn zstd_round_trip() {
        let compressed = zstd_encode(SAMPLE, 3).unwrap();
        assert_eq!(zstd_decode(&compressed).unwrap(), SAMPLE);
    }

    #[test]
    fn rejects_bad_levels() {
        assert!(gzip_encode(b"x", 99).is_err());
        assert!(zstd_encode(b"x", 0).is_err());
        assert!(zstd_encode(b"x", 23).is_err());
    }
}
