//! Built-in authentication: Mongo `app_users` lookup + argon2 verification.
//! This is the behavior the login handler had inline before providers existed;
//! moving it behind the trait keeps the two auth sources on one code path.

use async_trait::async_trait;
use bson::oid::ObjectId;

use super::{AppAuthProvider, AuthProviderError};
use crate::auth::password;
use crate::db::app_users::AppUserRepository;
use crate::domain::AppUser;

pub struct InternalProvider {
    app_users: AppUserRepository,
    org_id: ObjectId,
}

impl InternalProvider {
    pub fn new(app_users: AppUserRepository, org_id: ObjectId) -> Self {
        Self { app_users, org_id }
    }
}

#[async_trait]
impl AppAuthProvider for InternalProvider {
    async fn authenticate(
        &self,
        account: &str,
        password_input: &str,
    ) -> Result<AppUser, AuthProviderError> {
        // Case-insensitive lookup via the denormalized `username_lower`; trim to
        // be lenient about leading whitespace, matching the original handler.
        let username_key = account.trim().to_ascii_lowercase();
        let user = self
            .app_users
            .find_by_org_and_username_lower(self.org_id, &username_key)
            .await
            .map_err(|e| AuthProviderError::Unavailable(e.to_string()))?
            .ok_or(AuthProviderError::InvalidCredentials)?;

        let hash = user
            .password_hash
            .as_deref()
            .ok_or(AuthProviderError::InvalidCredentials)?;
        if !password::verify(password_input, hash)
            .map_err(|e| AuthProviderError::Unavailable(e.to_string()))?
        {
            return Err(AuthProviderError::InvalidCredentials);
        }

        Ok(user)
    }
}
