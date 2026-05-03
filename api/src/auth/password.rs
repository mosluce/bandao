use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};

use crate::error::{ApiError, ApiResult};

pub fn hash(plain: &str) -> ApiResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hasher = Argon2::default();
    let phc = hasher
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|err| {
            tracing::error!(?err, "argon2 hash failed");
            ApiError::Password
        })?;
    Ok(phc.to_string())
}

pub fn verify(plain: &str, hash: &str) -> ApiResult<bool> {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(err) => {
            tracing::error!(?err, "stored password hash unparsable");
            return Err(ApiError::Password);
        }
    };
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let hashed = hash("hunter2").unwrap();
        assert!(verify("hunter2", &hashed).unwrap());
        assert!(!verify("wrong", &hashed).unwrap());
    }
}
