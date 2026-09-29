//! Real Gzip / Deflate / Zstandard compression (`std::compress::*`).
//!
//! Backed by `flate2` (miniz-oxide, pure Rust) and `zstd` (compiles cleanly
//! on Termux AArch64). Every function operates on `Vec<u8>` so `.titan`
//! code can pipe bytes straight from `std::io::read`, `std::hash::*`, or
//! `std::encoding::*`.

use std::io::Write;

use std::io::{Error as IoError, ErrorKind};

use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::{Compression, Crc};
use miniz_oxide::inflate::core::{decompress, inflate_flags, DecompressorOxide};
use miniz_oxide::inflate::TINFLStatus;
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
    if value < 0 || value > max {
        return Err(CompressError::Level(value));
    }
    Ok(Compression::new(value as u32))
}


// ---------------- Decoding core ----------------
//
// Decoding used to go through flate2's `read::*Decoder` + `read_to_end`.
// That path had two silent-corruption defects:
//   * a truncated raw deflate / zlib stream returned the partial output as
//     success (flate2 maps miniz's "needs more input" to a plain EOF);
//   * a back-reference pointing before the start of the output was accepted
//     and filled with zeros from the (zeroed) 32 KiB wrapping dictionary.
// Both are now real errors: miniz_oxide is driven in non-wrapping mode over
// the whole input (which checks distances against the bytes produced so far)
// and "ran out of input" is reported as `unexpected end of file`.
// Everything else keeps flate2's exact behaviour and messages: any invalid
// data (including a zlib preset dictionary or an Adler-32 mismatch) is
// "corrupt deflate stream", and bytes after the end of the stream are ignored.

fn corrupt_deflate() -> CompressError {
    CompressError::Io(IoError::new(
        ErrorKind::InvalidInput,
        "corrupt deflate stream",
    ))
}

fn unexpected_eof() -> CompressError {
    CompressError::Io(ErrorKind::UnexpectedEof.into())
}

/// Inflates one stream from the start of `input`. Returns the output and the
/// number of input bytes that belong to the stream.
fn inflate_stream(input: &[u8], zlib: bool) -> Result<(Vec<u8>, usize), CompressError> {
    let mut flags = inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    if zlib {
        flags |= inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
            | inflate_flags::TINFL_FLAG_COMPUTE_ADLER32;
    }
    let mut out: Vec<u8> = vec![0; input.len().saturating_mul(2).max(64)];
    let mut decomp = Box::<DecompressorOxide>::default();
    let mut in_pos = 0usize;
    let mut out_pos = 0usize;
    loop {
        let (status, in_used, out_used) =
            decompress(&mut decomp, &input[in_pos..], &mut out, out_pos, flags);
        in_pos += in_used;
        out_pos += out_used;
        match status {
            TINFLStatus::Done => {
                out.truncate(out_pos);
                return Ok((out, in_pos));
            }
            TINFLStatus::HasMoreOutput => {
                let new_len = out.len().saturating_mul(2);
                out.resize(new_len, 0);
            }
            TINFLStatus::FailedCannotMakeProgress | TINFLStatus::NeedsMoreInput => {
                return Err(unexpected_eof());
            }
            _ => return Err(corrupt_deflate()),
        }
    }
}

const GZ_FHCRC: u8 = 1 << 1;
const GZ_FEXTRA: u8 = 1 << 2;
const GZ_FNAME: u8 = 1 << 3;
const GZ_FCOMMENT: u8 = 1 << 4;
const GZ_FRESERVED: u8 = 1 << 5 | 1 << 6 | 1 << 7;
const GZ_MAX_HEADER_BUF: usize = 65535;

fn bad_gzip_header() -> CompressError {
    CompressError::Io(IoError::new(ErrorKind::InvalidInput, "invalid gzip header"))
}

fn corrupt_gzip() -> CompressError {
    CompressError::Io(IoError::new(
        ErrorKind::InvalidInput,
        "corrupt gzip stream does not have a matching checksum",
    ))
}

fn take<'a>(data: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8], CompressError> {
    if data.len() - *pos < n {
        return Err(unexpected_eof());
    }
    let slice = &data[*pos..*pos + n];
    *pos += n;
    Ok(slice)
}

fn take_to_nul<'a>(data: &'a [u8], pos: &mut usize) -> Result<&'a [u8], CompressError> {
    let start = *pos;
    loop {
        match data.get(*pos) {
            None => return Err(unexpected_eof()),
            Some(0) => {
                *pos += 1;
                return Ok(&data[start..*pos]);
            }
            Some(_) if *pos - start == GZ_MAX_HEADER_BUF => {
                return Err(CompressError::Io(IoError::new(
                    ErrorKind::InvalidInput,
                    "gzip header field too long",
                )));
            }
            Some(_) => *pos += 1,
        }
    }
}

/// Same header rules as flate2's `GzHeaderParser` (first member only).
fn gzip_header_len(data: &[u8]) -> Result<usize, CompressError> {
    let mut pos = 0usize;
    let fixed = take(data, &mut pos, 10)?;
    if fixed[0] != 0x1f || fixed[1] != 0x8b || fixed[2] != 8 {
        return Err(bad_gzip_header());
    }
    let flags = fixed[3];
    if flags & GZ_FRESERVED != 0 {
        return Err(bad_gzip_header());
    }
    if flags & GZ_FEXTRA != 0 {
        let xlen = take(data, &mut pos, 2)?;
        let xlen = u16::from_le_bytes([xlen[0], xlen[1]]) as usize;
        take(data, &mut pos, xlen)?;
    }
    if flags & GZ_FNAME != 0 {
        take_to_nul(data, &mut pos)?;
    }
    if flags & GZ_FCOMMENT != 0 {
        take_to_nul(data, &mut pos)?;
    }
    if flags & GZ_FHCRC != 0 {
        let mut crc = Crc::new();
        crc.update(&data[..pos]);
        let stored = take(data, &mut pos, 2)?;
        if u16::from_le_bytes([stored[0], stored[1]]) != crc.sum() as u16 {
            return Err(corrupt_gzip());
        }
    }
    Ok(pos)
}

// ---------------- Gzip ----------------

pub fn gzip_encode(data: &[u8], compression_level: i32) -> Result<Vec<u8>, CompressError> {
    let mut encoder = GzEncoder::new(Vec::new(), level(compression_level, 9)?);
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn gzip_decode(data: &[u8]) -> Result<Vec<u8>, CompressError> {
    let header = gzip_header_len(data)?;
    let (out, used) = inflate_stream(&data[header..], false)?;
    let mut pos = header + used;
    let trailer = take(data, &mut pos, 8)?;
    let stored_crc = u32::from_le_bytes([trailer[0], trailer[1], trailer[2], trailer[3]]);
    let stored_len = u32::from_le_bytes([trailer[4], trailer[5], trailer[6], trailer[7]]);
    let mut crc = Crc::new();
    crc.update(&out);
    if stored_crc != crc.sum() || stored_len != crc.amount() {
        return Err(corrupt_gzip());
    }
    Ok(out)
}

// ---------------- Zlib (RFC 1950) ----------------

pub fn zlib_encode(data: &[u8], compression_level: i32) -> Result<Vec<u8>, CompressError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), level(compression_level, 9)?);
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn zlib_decode(data: &[u8]) -> Result<Vec<u8>, CompressError> {
    Ok(inflate_stream(data, true)?.0)
}

// ---------------- Raw Deflate (RFC 1951) ----------------

pub fn deflate_encode(data: &[u8], compression_level: i32) -> Result<Vec<u8>, CompressError> {
    let mut encoder = DeflateEncoder::new(Vec::new(), level(compression_level, 9)?);
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn deflate_decode(data: &[u8]) -> Result<Vec<u8>, CompressError> {
    Ok(inflate_stream(data, false)?.0)
}

// ---------------- Zstandard ----------------

pub fn zstd_encode(data: &[u8], compression_level: i32) -> Result<Vec<u8>, CompressError> {
    if !(1..=22).contains(&compression_level) {
        return Err(CompressError::Level(compression_level));
    }
    zstd::stream::encode_all(data, compression_level)
        .map_err(|error| CompressError::Zstd(error.to_string()))
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
        assert!(
            compressed.len() < SAMPLE.len(),
            "gzip should shrink repetitive text"
        );
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
    fn truncated_streams_are_errors() {
        let deflate = deflate_encode(SAMPLE, 6).unwrap();
        let zlib = zlib_encode(SAMPLE, 6).unwrap();
        let gzip = gzip_encode(SAMPLE, 6).unwrap();
        for cut in 0..deflate.len() {
            assert!(deflate_decode(&deflate[..cut]).is_err(), "deflate cut {cut}");
        }
        for cut in 0..zlib.len() {
            assert!(zlib_decode(&zlib[..cut]).is_err(), "zlib cut {cut}");
        }
        for cut in 0..gzip.len() {
            assert!(gzip_decode(&gzip[..cut]).is_err(), "gzip cut {cut}");
        }
        let msg = deflate_decode(b"xyz").unwrap_err().to_string();
        assert_eq!(msg, "compression I/O error: unexpected end of file");
    }

    #[test]
    fn distance_before_start_is_an_error() {
        // Fixed-Huffman block: literal 'a', then a match of length 3 at
        // distance 2 (only 1 byte has been produced), then end of block.
        let stream = [0x4b, 0x04, 0x42, 0x00];
        let msg = deflate_decode(&stream).unwrap_err().to_string();
        assert_eq!(msg, "compression I/O error: corrupt deflate stream");
    }

    #[test]
    fn trailing_bytes_are_ignored() {
        let mut gzip = gzip_encode(SAMPLE, 6).unwrap();
        gzip.extend_from_slice(b"garbage");
        assert_eq!(gzip_decode(&gzip).unwrap(), SAMPLE);
        let mut zlib = zlib_encode(SAMPLE, 6).unwrap();
        zlib.extend_from_slice(b"garbage");
        assert_eq!(zlib_decode(&zlib).unwrap(), SAMPLE);
    }

    #[test]
    fn rejects_bad_levels() {
        assert!(gzip_encode(b"x", 99).is_err());
        assert!(zstd_encode(b"x", 0).is_err());
        assert!(zstd_encode(b"x", 23).is_err());
    }
}
