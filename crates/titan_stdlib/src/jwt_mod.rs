//! JSON Web Token creation and verification (`std::jwt::*`).
//!
//! Backed by the `jsonwebtoken` crate. Payloads are `serde_json::Value`,
//! which the VM already knows how to bridge to Titan maps/arrays/etc.
//!
//! Two algorithm families:
//!   * **HS256** — HMAC-SHA256 with a shared secret. Cheapest, symmetric.
//!   * **RS256** — RSA-SHA256 with a public/private PEM pair. Asymmetric;
//!     the token can be verified by anyone holding the public key.

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JwtError {
    #[error("JWT error: {0}")]
    Token(String),
    #[error("invalid PEM material: {0}")]
    Pem(String),
}

fn tok(error: impl std::fmt::Display) -> JwtError {
    JwtError::Token(error.to_string())
}
fn pem(error: impl std::fmt::Display) -> JwtError {
    JwtError::Pem(error.to_string())
}

// ---------------- HS256 ----------------

/// Encodes `claims` as a JWT signed with HMAC-SHA256 using `secret`.
pub fn sign_hs256(claims: &Value, secret: &[u8]) -> Result<String, JwtError> {
    encode(
        &Header::new(Algorithm::HS256),
        claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(tok)
}

/// Verifies an HS256 token and returns its decoded claims.
///
/// Sets `validate_exp` to `true` — an expired `exp` claim will be rejected
/// automatically. Pass `expected_audience`/`expected_issuer` = None to skip
/// those checks.
pub fn verify_hs256(
    token: &str,
    secret: &[u8],
    expected_audience: Option<&str>,
    expected_issuer: Option<&str>,
) -> Result<Value, JwtError> {
    let mut validation = Validation::new(Algorithm::HS256);
    if let Some(aud) = expected_audience {
        validation.set_audience(&[aud]);
    }
    if let Some(iss) = expected_issuer {
        validation.set_issuer(&[iss]);
    }
    let data =
        decode::<Value>(token, &DecodingKey::from_secret(secret), &validation).map_err(tok)?;
    Ok(data.claims)
}

// ---------------- RS256 ----------------

pub fn sign_rs256(claims: &Value, private_pem: &[u8]) -> Result<String, JwtError> {
    let key = EncodingKey::from_rsa_pem(private_pem).map_err(pem)?;
    encode(&Header::new(Algorithm::RS256), claims, &key).map_err(tok)
}

pub fn verify_rs256(
    token: &str,
    public_pem: &[u8],
    expected_audience: Option<&str>,
    expected_issuer: Option<&str>,
) -> Result<Value, JwtError> {
    let key = DecodingKey::from_rsa_pem(public_pem).map_err(pem)?;
    let mut validation = Validation::new(Algorithm::RS256);
    if let Some(aud) = expected_audience {
        validation.set_audience(&[aud]);
    }
    if let Some(iss) = expected_issuer {
        validation.set_issuer(&[iss]);
    }
    let data = decode::<Value>(token, &key, &validation).map_err(tok)?;
    Ok(data.claims)
}

/// Decodes the JWT header only (no signature check). Useful to peek at `alg`,
/// `kid`, `typ` before choosing a validator.
pub fn peek_header(token: &str) -> Result<Value, JwtError> {
    let header = jsonwebtoken::decode_header(token).map_err(tok)?;
    Ok(serde_json::to_value(header).map_err(tok)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hs256_round_trip() {
        let secret = b"super-secret-not-in-git";
        let claims = json!({
            "sub": "user-42",
            "name": "Juan",
            "exp": chrono_far_future(),
        });
        let token = sign_hs256(&claims, secret).unwrap();
        assert!(token.split('.').count() == 3, "JWT should have three parts");
        let decoded = verify_hs256(&token, secret, None, None).unwrap();
        assert_eq!(decoded["sub"], "user-42");
        assert_eq!(decoded["name"], "Juan");
    }

    #[test]
    fn hs256_rejects_wrong_secret() {
        let token = sign_hs256(
            &json!({ "sub": "x", "exp": chrono_far_future() }),
            b"correcto",
        )
        .unwrap();
        assert!(verify_hs256(&token, b"otro secreto", None, None).is_err());
    }

    #[test]
    fn hs256_rejects_expired_token() {
        // exp in 1970 -> definitely expired.
        let token = sign_hs256(&json!({ "sub": "x", "exp": 1i64 }), b"k").unwrap();
        assert!(verify_hs256(&token, b"k", None, None).is_err());
    }

    #[test]
    fn peek_header_reveals_alg() {
        let token = sign_hs256(&json!({ "exp": chrono_far_future() }), b"k").unwrap();
        let header = peek_header(&token).unwrap();
        assert_eq!(header["alg"], "HS256");
    }

    // Uses std::time so we don't require chrono here.
    fn chrono_far_future() -> i64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        now + 3600
    }
}
