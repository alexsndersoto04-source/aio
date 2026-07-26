//! Password hashing (`std::password::*`) done right.
//!
//! Two industry-standard KDFs:
//!   * **Argon2id** — memory-hard, winner of the Password Hashing Competition.
//!     Recommended default for new systems.
//!   * **bcrypt** — battle-tested classic; use when interoperating with older
//!     databases that already store bcrypt hashes.
//!
//! Both `hash_*` helpers return a **PHC-formatted string** that carries the
//! algorithm parameters and salt inside, so `verify_*` can be called with
//! nothing more than the hash string and the candidate password.
//!
//! **Never** store a plain SHA-256 or MD5 of a password — those are for file
//! integrity, not for authentication. Use these functions for user credentials.

use argon2::{
    // Use the OsRng re-exported by `password_hash` so we're compatible with
    // its bundled `rand_core 0.6`, no matter what `rand` version other
    // dependencies pull in.
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("password hash error: {0}")]
    Hash(String),
    #[error("bcrypt error: {0}")]
    Bcrypt(String),
}

fn arg_err(error: impl std::fmt::Display) -> PasswordError { PasswordError::Hash(error.to_string()) }
fn bcrypt_err(error: impl std::fmt::Display) -> PasswordError { PasswordError::Bcrypt(error.to_string()) }

// ---------------- Argon2id (recommended default) ----------------

/// Hashes a password with Argon2id and safe defaults (m=19 MiB, t=2, p=1).
pub fn hash_argon2(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let params = Params::new(19_456, 2, 1, None).map_err(arg_err)?;
    let argon = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    Ok(argon.hash_password(password.as_bytes(), &salt).map_err(arg_err)?.to_string())
}

/// Verifies a password against a PHC-formatted Argon2 hash.
pub fn verify_argon2(hash: &str, password: &str) -> Result<bool, PasswordError> {
    let parsed = PasswordHash::new(hash).map_err(arg_err)?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

// ---------------- bcrypt (compatibility) ----------------

/// Hashes a password with bcrypt at the given cost (10 = sane default; use 12 if the
/// hardware can afford it).
pub fn hash_bcrypt(password: &str, cost: u32) -> Result<String, PasswordError> {
    bcrypt::hash(password, cost).map_err(bcrypt_err)
}

/// Verifies a password against a bcrypt hash.
pub fn verify_bcrypt(hash: &str, password: &str) -> Result<bool, PasswordError> {
    bcrypt::verify(password, hash).map_err(bcrypt_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2_hash_and_verify_round_trip() {
        let hash = hash_argon2("mi clave secreta 123").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_argon2(&hash, "mi clave secreta 123").unwrap());
        assert!(!verify_argon2(&hash, "clave incorrecta").unwrap());
    }

    #[test]
    fn argon2_produces_different_hashes_thanks_to_salt() {
        let a = hash_argon2("mismo password").unwrap();
        let b = hash_argon2("mismo password").unwrap();
        assert_ne!(a, b, "salted hashes must differ across calls");
        assert!(verify_argon2(&a, "mismo password").unwrap());
        assert!(verify_argon2(&b, "mismo password").unwrap());
    }

    #[test]
    fn bcrypt_hash_and_verify_round_trip() {
        // Use a low cost (4) to keep the test fast; bcrypt refuses cost < 4.
        let hash = hash_bcrypt("Pass w0rd!", 4).unwrap();
        assert!(hash.starts_with("$2"));
        assert!(verify_bcrypt(&hash, "Pass w0rd!").unwrap());
        assert!(!verify_bcrypt(&hash, "otra cosa").unwrap());
    }

    #[test]
    fn verify_reports_error_on_garbage_hash() {
        assert!(verify_argon2("no es un hash", "x").is_err());
        assert!(verify_bcrypt("no es un hash", "x").is_err());
    }
}
