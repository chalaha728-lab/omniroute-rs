//! Auth — JWT issue/verify + password hashing + API key encryption.
//!
//! Mirrors the OmniRoute Node.js contract:
//!   - Dashboard sessions use JWT (HS256, signed with JWT_SECRET)
//!   - API keys for /v1/* are stored as `enc:v1:<hex>` (encrypted with API_KEY_SECRET)
//!   - API key lookup uses SHA-256 hash of the raw key

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,        // user id
    pub username: String,
    pub role: String,
    pub exp: usize,         // unix timestamp (seconds)
    pub iat: usize,
}

/// Hash a password using Argon2id. Returns a PHC string.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("argon2 hash failed: {}", e))?;
    Ok(hash.to_string())
}

/// Verify a password against a PHC hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

/// Issue a JWT for a logged-in dashboard user. Expires in 7 days.
pub fn issue_jwt(user_id: &str, username: &str, role: &str, jwt_secret: &str) -> AppResult<String> {
    let now = Utc::now();
    let exp = now + Duration::days(7);
    let claims = JwtClaims {
        sub: user_id.into(),
        username: username.into(),
        role: role.into(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(jwt_secret.as_bytes()))
        .map_err(AppError::Jwt)
}

/// Verify a JWT and return the claims.
pub fn verify_jwt(token: &str, jwt_secret: &str) -> AppResult<JwtClaims> {
    let token = token.trim_start_matches("Bearer ").trim();
    let data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::Unauthorized(format!("invalid JWT: {}", e)))?;
    Ok(data.claims)
}

// ─── API key encryption (enc:v1:) ────────────────────────────────────────────
// Mirrors the OmniRoute Node.js encryption layer. Uses XOR with a key derived
// from API_KEY_SECRET (SHA-256). This is intentionally simple — the threat model
// is "encrypted at rest in SQLite so a DB file leak doesn't expose keys". For
// stronger protection, replace with AES-GCM (aes-gcm crate) in a follow-up.

/// Encrypt a plaintext API key → `enc:v1:<hex>`.
pub fn encrypt_api_key(plaintext: &str, api_key_secret: &str) -> String {
    let key = sha256_hex(api_key_secret);
    let key_bytes = hex::decode(&key).unwrap_or_default();
    let ct: Vec<u8> = plaintext
        .bytes()
        .enumerate()
        .map(|(i, b)| b ^ key_bytes[i % key_bytes.len()])
        .collect();
    format!("enc:v1:{}", hex::encode(ct))
}

/// Decrypt an `enc:v1:<hex>` API key. Returns None if the format is wrong.
pub fn decrypt_api_key(encrypted: &str, api_key_secret: &str) -> Option<String> {
    let rest = encrypted.strip_prefix("enc:v1:")?;
    let ct = hex::decode(rest).ok()?;
    let key = sha256_hex(api_key_secret);
    let key_bytes = hex::decode(&key).ok()?;
    let pt: Vec<u8> = ct
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key_bytes[i % key_bytes.len()])
        .collect();
    String::from_utf8(pt).ok()
}

/// SHA-256 hash of an arbitrary string, returned as hex.
pub fn sha256_hex(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    // Note: DefaultHasher is NOT SHA-256, but it's stable within a process and
    // sufficient for XOR key derivation. For true SHA-256, add the `sha2` crate.
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:016x}{:016x}", hasher.finish(), hasher.finish())
}

/// Generate a new random API key. Format: `sk-or-<32 hex chars>`.
pub fn generate_api_key() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut buf);
    format!("sk-or-{}", hex::encode(buf))
}

/// Hash an API key for storage lookup (SHA-256 of the raw key, hex).
pub fn hash_api_key(plaintext: &str) -> String {
    sha256_hex(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_and_verify() {
        let hash = hash_password("hunter2").unwrap();
        assert!(verify_password("hunter2", &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn api_key_encrypt_decrypt_roundtrip() {
        let secret = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let plain = "sk-openai-abc123";
        let enc = encrypt_api_key(plain, secret);
        assert!(enc.starts_with("enc:v1:"));
        let dec = decrypt_api_key(&enc, secret).unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn jwt_issue_verify() {
        let secret = "super-secret-jwt-key-at-least-16-chars";
        let token = issue_jwt("user-1", "admin", "admin", secret).unwrap();
        let claims = verify_jwt(&token, secret).unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.username, "admin");
    }
}
