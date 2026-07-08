//! Symmetric encryption for secrets that must be recoverable in plaintext —
//! currently the external-auth database connection password. Unlike user
//! passwords (one-way argon2), we need the original value back to open the
//! external DB connection, so this uses reversible AEAD (XChaCha20-Poly1305).
//!
//! The key comes from `BANDAO_SECRET_KEY` (base64 of 32 bytes); deployments
//! that never use external auth can omit it. Ciphertext is encoded as
//! `base64(nonce[24] || ciphertext+tag)`.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;

use crate::error::{ApiError, ApiResult};

const NONCE_LEN: usize = 24;

#[derive(Clone)]
pub struct SecretBox {
    cipher: XChaCha20Poly1305,
}

impl SecretBox {
    pub fn from_key_bytes(key: &[u8; 32]) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(key.into()),
        }
    }

    /// Encrypt `plaintext` → `base64(nonce || ciphertext)`. A fresh random
    /// nonce is generated per call, so encrypting the same value twice yields
    /// different ciphertext.
    pub fn encrypt(&self, plaintext: &str) -> ApiResult<String> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| ApiError::Internal)?;
        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(B64.encode(out))
    }

    /// Reverse of [`encrypt`]. Returns `Internal` on any decode/auth failure
    /// (wrong key, tampered ciphertext, malformed encoding) — callers must not
    /// leak which.
    pub fn decrypt(&self, encoded: &str) -> ApiResult<String> {
        let raw = B64.decode(encoded).map_err(|_| ApiError::Internal)?;
        if raw.len() <= NONCE_LEN {
            return Err(ApiError::Internal);
        }
        let (nonce_bytes, ciphertext) = raw.split_at(NONCE_LEN);
        let nonce = XNonce::from_slice(nonce_bytes);
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| ApiError::Internal)?;
        String::from_utf8(plaintext).map_err(|_| ApiError::Internal)
    }
}

/// Decode a `BANDAO_SECRET_KEY` value (base64 of exactly 32 bytes) into a key
/// array. Returns `None` for the wrong length so config loading can report a
/// precise error.
pub fn decode_key(b64: &str) -> Option<[u8; 32]> {
    let bytes = B64.decode(b64).ok()?;
    <[u8; 32]>::try_from(bytes.as_slice()).ok()
}
