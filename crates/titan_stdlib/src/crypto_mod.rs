//! Authenticated symmetric encryption (`std::crypto::*`).
//!
//! Two modern AEAD ciphers, both backed by the RustCrypto ecosystem:
//!   * **ChaCha20-Poly1305** — fast and secure everywhere; the default choice
//!     when you don't have hardware AES. 32-byte key, 12-byte nonce.
//!   * **AES-256-GCM** — industry standard; 32-byte key, 12-byte nonce.
//!
//! Both provide *authenticated* encryption: any tampering with the ciphertext,
//! the nonce or the optional associated data (`aad`) is detected on decrypt.
//!
//! Convenience helpers `chacha20_seal` / `aes_gcm_seal` prepend the 12-byte
//! nonce to the ciphertext so `.titan` code can persist a single blob and
//! decrypt it later without keeping a separate nonce around.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng as AeadOsRng, Payload};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::ChaCha20Poly1305;
use rand::RngCore;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid key length: expected {expected} bytes, got {got}")]
    KeyLength { expected: usize, got: usize },
    #[error("invalid nonce length: expected 12 bytes, got {0}")]
    NonceLength(usize),
    #[error("cipher failed (wrong key/nonce or tampered ciphertext)")]
    Cipher,
    #[error("sealed message is truncated (needs at least 12 bytes for the nonce)")]
    Truncated,
}

fn take_key<const N: usize>(key: &[u8]) -> Result<[u8; N], CryptoError> {
    if key.len() != N { return Err(CryptoError::KeyLength { expected: N, got: key.len() }); }
    let mut out = [0u8; N];
    out.copy_from_slice(key);
    Ok(out)
}

fn take_nonce(nonce: &[u8]) -> Result<[u8; 12], CryptoError> {
    if nonce.len() != 12 { return Err(CryptoError::NonceLength(nonce.len())); }
    let mut out = [0u8; 12];
    out.copy_from_slice(nonce);
    Ok(out)
}

/// Generates a cryptographically secure random 32-byte key.
pub fn generate_key_32() -> Vec<u8> {
    let mut key = vec![0u8; 32];
    rand::rng().fill_bytes(&mut key);
    key
}

/// Generates a fresh 12-byte nonce. **Never reuse a (key, nonce) pair.**
pub fn generate_nonce() -> Vec<u8> {
    let mut nonce = vec![0u8; 12];
    rand::rng().fill_bytes(&mut nonce);
    nonce
}

// ---------------- ChaCha20-Poly1305 ----------------

pub fn chacha20_encrypt(key: &[u8], nonce: &[u8], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = take_key::<32>(key)?;
    let nonce_bytes = take_nonce(nonce)?;
    let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes).expect("32 bytes");
    cipher.encrypt(&nonce_bytes.into(), Payload { msg: plaintext, aad }).map_err(|_| CryptoError::Cipher)
}

pub fn chacha20_decrypt(key: &[u8], nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = take_key::<32>(key)?;
    let nonce_bytes = take_nonce(nonce)?;
    let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes).expect("32 bytes");
    cipher.decrypt(&nonce_bytes.into(), Payload { msg: ciphertext, aad }).map_err(|_| CryptoError::Cipher)
}

/// Encrypts `plaintext` and returns `nonce || ciphertext` (a single blob).
///
/// Uses the AEAD-crate's own `OsRng` (backed by `rand_core 0.6`) to generate
/// the nonce so we don't collide with `rand 0.9` used elsewhere in the crate.
pub fn chacha20_seal(key: &[u8], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = take_key::<32>(key)?;
    let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes).expect("32 bytes");
    let nonce = ChaCha20Poly1305::generate_nonce(&mut AeadOsRng);
    let ciphertext = cipher.encrypt(&nonce, Payload { msg: plaintext, aad }).map_err(|_| CryptoError::Cipher)?;
    let mut out = Vec::with_capacity(nonce.len() + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Reverses `chacha20_seal`.
pub fn chacha20_open(key: &[u8], sealed: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if sealed.len() < 12 { return Err(CryptoError::Truncated); }
    let (nonce, ciphertext) = sealed.split_at(12);
    chacha20_decrypt(key, nonce, ciphertext, aad)
}

// ---------------- AES-256-GCM ----------------

pub fn aes_gcm_encrypt(key: &[u8], nonce: &[u8], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = take_key::<32>(key)?;
    let nonce_bytes = take_nonce(nonce)?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("32 bytes");
    cipher.encrypt(&nonce_bytes.into(), Payload { msg: plaintext, aad }).map_err(|_| CryptoError::Cipher)
}

pub fn aes_gcm_decrypt(key: &[u8], nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = take_key::<32>(key)?;
    let nonce_bytes = take_nonce(nonce)?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("32 bytes");
    cipher.decrypt(&nonce_bytes.into(), Payload { msg: ciphertext, aad }).map_err(|_| CryptoError::Cipher)
}

pub fn aes_gcm_seal(key: &[u8], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = take_key::<32>(key)?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("32 bytes");
    let nonce = Aes256Gcm::generate_nonce(&mut AeadOsRng);
    let ciphertext = cipher.encrypt(&nonce, Payload { msg: plaintext, aad }).map_err(|_| CryptoError::Cipher)?;
    let mut out = Vec::with_capacity(nonce.len() + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn aes_gcm_open(key: &[u8], sealed: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if sealed.len() < 12 { return Err(CryptoError::Truncated); }
    let (nonce, ciphertext) = sealed.split_at(12);
    aes_gcm_decrypt(key, nonce, ciphertext, aad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_helpers_have_correct_length() {
        assert_eq!(generate_key_32().len(), 32);
        assert_eq!(generate_nonce().len(), 12);
    }

    #[test]
    fn chacha20_seal_round_trip() {
        let key = generate_key_32();
        let msg = b"mensaje ultra secreto";
        let aad = b"meta";
        let sealed = chacha20_seal(&key, msg, aad).unwrap();
        assert_ne!(&sealed[12..], msg, "ciphertext must differ from plaintext");
        let opened = chacha20_open(&key, &sealed, aad).unwrap();
        assert_eq!(opened, msg);
    }

    #[test]
    fn aes_gcm_seal_round_trip() {
        let key = generate_key_32();
        let msg = b"otro mensaje";
        let sealed = aes_gcm_seal(&key, msg, b"").unwrap();
        assert_eq!(aes_gcm_open(&key, &sealed, b"").unwrap(), msg);
    }

    #[test]
    fn tampering_is_detected() {
        let key = generate_key_32();
        let mut sealed = chacha20_seal(&key, b"pago 100 USD", b"").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01; // flip one bit
        assert!(chacha20_open(&key, &sealed, b"").is_err(), "tampered ciphertext must be rejected");
    }

    #[test]
    fn wrong_aad_is_rejected() {
        let key = generate_key_32();
        let sealed = chacha20_seal(&key, b"hola", b"correcto").unwrap();
        assert!(chacha20_open(&key, &sealed, b"otro").is_err());
    }

    #[test]
    fn rejects_short_key_and_nonce() {
        assert!(matches!(chacha20_encrypt(&[0u8; 16], &[0u8; 12], b"x", b""), Err(CryptoError::KeyLength { .. })));
        assert!(matches!(chacha20_encrypt(&[0u8; 32], &[0u8; 8], b"x", b""),  Err(CryptoError::NonceLength(_))));
    }
}
