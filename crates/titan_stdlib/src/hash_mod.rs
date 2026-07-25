//! Cryptographic hashes and HMAC (`std::hash::*`).
//!
//! Backed by the RustCrypto ecosystem: `sha2`, `sha3`, `blake3`, `hmac`.
//! Output is always the lowercase hex-encoded digest as a `String` (except
//! `*_bytes` helpers which return raw `Vec<u8>`), so `.titan` code doesn't
//! have to worry about encoding.

use hmac::{Hmac, Mac};
use sha2::{Digest as _, Sha256, Sha384, Sha512};
use sha3::{Sha3_256, Sha3_512};

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes { use std::fmt::Write as _; let _ = write!(out, "{byte:02x}"); }
    out
}

pub fn sha256(data: &[u8]) -> String { hex(&Sha256::digest(data)) }
pub fn sha384(data: &[u8]) -> String { hex(&Sha384::digest(data)) }
pub fn sha512(data: &[u8]) -> String { hex(&Sha512::digest(data)) }
pub fn sha3_256(data: &[u8]) -> String { hex(&Sha3_256::digest(data)) }
pub fn sha3_512(data: &[u8]) -> String { hex(&Sha3_512::digest(data)) }
pub fn blake3(data: &[u8]) -> String { blake3::hash(data).to_hex().to_string() }

pub fn sha256_bytes(data: &[u8]) -> Vec<u8> { Sha256::digest(data).to_vec() }
pub fn sha512_bytes(data: &[u8]) -> Vec<u8> { Sha512::digest(data).to_vec() }
pub fn blake3_bytes(data: &[u8]) -> Vec<u8> { blake3::hash(data).as_bytes().to_vec() }

/// HMAC-SHA256 with an arbitrary key. Returns hex digest.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    hex(&mac.finalize().into_bytes())
}

/// HMAC-SHA512 with an arbitrary key. Returns hex digest.
pub fn hmac_sha512(key: &[u8], data: &[u8]) -> String {
    let mut mac = <Hmac<Sha512> as Mac>::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    hex(&mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vectors from FIPS 180-4 / RFC 6234 / BLAKE3 test suite.
    #[test]
    fn known_vectors() {
        assert_eq!(sha256(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(sha512(b"abc"),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f");
        assert_eq!(sha3_256(b"abc"), "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532");
        assert_eq!(blake3(b""),      "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262");
        assert_eq!(blake3(b"abc"),   "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85");
    }

    // RFC 4231 test vector for HMAC-SHA256.
    #[test]
    fn hmac_sha256_rfc4231() {
        let key = [0x0bu8; 20];
        assert_eq!(
            hmac_sha256(&key, b"Hi There"),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn bytes_helpers_match_hex() {
        let raw = sha256_bytes(b"abc");
        assert_eq!(hex(&raw), sha256(b"abc"));
        let b = blake3_bytes(b"abc");
        assert_eq!(b.len(), 32);
    }
}
