//! Binary-to-text encodings with strict validation.

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)] pub enum EncodingError {
    #[error("invalid hexadecimal length")] InvalidHexLength,
    #[error("invalid hexadecimal character at byte {0}")] InvalidHex(usize),
    #[error("invalid base64 data at byte {0}")] InvalidBase64(usize),
    #[error("invalid percent encoding at byte {0}")] InvalidPercent(usize),
    #[error("decoded data is not UTF-8")] InvalidUtf8,
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn hex_encode(bytes: &[u8]) -> String { const H: &[u8; 16] = b"0123456789abcdef"; let mut out = String::with_capacity(bytes.len() * 2); for b in bytes { out.push(H[(b >> 4) as usize] as char); out.push(H[(b & 15) as usize] as char); } out }
pub fn hex_decode(text: &str) -> Result<Vec<u8>, EncodingError> {
    if !text.len().is_multiple_of(2) { return Err(EncodingError::InvalidHexLength); }
    text.as_bytes().chunks_exact(2).enumerate().map(|(i, pair)| Ok((hex(pair[0]).ok_or(EncodingError::InvalidHex(i * 2))? << 4) | hex(pair[1]).ok_or(EncodingError::InvalidHex(i * 2 + 1))?)).collect()
}
fn hex(value: u8) -> Option<u8> { match value { b'0'..=b'9' => Some(value - b'0'), b'a'..=b'f' => Some(value - b'a' + 10), b'A'..=b'F' => Some(value - b'A' + 10), _ => None } }

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let bits = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(B64[((bits >> 18) & 63) as usize] as char); out.push(B64[((bits >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { B64[((bits >> 6) & 63) as usize] as char } else { '=' }); out.push(if chunk.len() > 2 { B64[(bits & 63) as usize] as char } else { '=' });
    }
    out
}
pub fn base64_decode(text: &str) -> Result<Vec<u8>, EncodingError> {
    let bytes = text.as_bytes(); if !bytes.len().is_multiple_of(4) { return Err(EncodingError::InvalidBase64(bytes.len())); }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (block, chunk) in bytes.chunks_exact(4).enumerate() {
        let last = block + 1 == bytes.len() / 4; let pad = usize::from(chunk[3] == b'=') + usize::from(chunk[2] == b'=');
        if chunk[0] == b'=' || chunk[1] == b'=' || (!last && pad > 0) || pad > 2 || (chunk[2] == b'=' && chunk[3] != b'=') { return Err(EncodingError::InvalidBase64(block * 4)); }
        let mut values = [0u8; 4]; for i in 0..4 { values[i] = if chunk[i] == b'=' { 0 } else { b64_value(chunk[i]).ok_or(EncodingError::InvalidBase64(block * 4 + i))? }; }
        if (pad == 2 && values[1] & 0x0f != 0) || (pad == 1 && values[2] & 0x03 != 0) { return Err(EncodingError::InvalidBase64(block * 4 + 1)); }
        let bits = (u32::from(values[0]) << 18) | (u32::from(values[1]) << 12) | (u32::from(values[2]) << 6) | u32::from(values[3]);
        out.push((bits >> 16) as u8);
        if pad < 2 { out.push((bits >> 8) as u8); }
        if pad == 0 { out.push(bits as u8); }
    }
    Ok(out)
}
fn b64_value(value: u8) -> Option<u8> { B64.iter().position(|v| *v == value).map(|i| i as u8) }

pub fn percent_encode(text: &str) -> String { let mut out = String::new(); for b in text.bytes() { if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { out.push(b as char); } else { out.push('%'); out.push_str(&format!("{b:02X}")); } } out }
pub fn percent_decode(text: &str) -> Result<String, EncodingError> {
    let bytes = text.as_bytes(); let mut out = Vec::new(); let mut i = 0;
    while i < bytes.len() { if bytes[i] == b'%' { if i + 2 >= bytes.len() { return Err(EncodingError::InvalidPercent(i)); } let a = hex(bytes[i + 1]).ok_or(EncodingError::InvalidPercent(i))?; let b = hex(bytes[i + 2]).ok_or(EncodingError::InvalidPercent(i))?; out.push((a << 4) | b); i += 3; } else { out.push(bytes[i]); i += 1; } }
    String::from_utf8(out).map_err(|_| EncodingError::InvalidUtf8)
}

#[cfg(test)] mod tests { use super::*; #[test] fn known_vectors() { assert_eq!(hex_encode(b"Titan"), "546974616e"); assert_eq!(hex_decode("546974616e").unwrap(), b"Titan"); assert_eq!(base64_encode(b"Titan"), "VGl0YW4="); assert_eq!(base64_decode("VGl0YW4=").unwrap(), b"Titan"); } #[test] fn rejects_non_canonical_base64() { for value in ["A===", "Zh==", "Zm9="] { assert!(base64_decode(value).is_err(), "{value}"); } } #[test] fn percent_round_trip() { let value = "año / 1"; assert_eq!(percent_decode(&percent_encode(value)).unwrap(), value); } }
