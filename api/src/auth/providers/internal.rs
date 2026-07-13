//! Built-in authentication: Mongo `app_users` lookup + argon2 verification.
//! This is the behavior the login handler had inline before providers existed;
//! moving it behind the trait keeps the two auth sources on one code path.

use std::sync::Arc;

use async_trait::async_trait;
use bson::DateTime;
use bson::oid::ObjectId;

use super::{AppAuthProvider, AuthProviderError};
use crate::auth::password;
use crate::config::Config;
use crate::db::app_users::AppUserRepository;
use crate::domain::AppUser;

pub struct InternalProvider {
    app_users: AppUserRepository,
    org_id: ObjectId,
    config: Arc<Config>,
}

impl InternalProvider {
    pub fn new(app_users: AppUserRepository, org_id: ObjectId, config: Arc<Config>) -> Self {
        Self {
            app_users,
            org_id,
            config,
        }
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

        // Locked accounts are rejected without checking the password, both to
        // avoid the wasted bcrypt work and so repeated attempts against a
        // locked account never extend the lock window.
        if user
            .locked_until
            .is_some_and(|until| until > DateTime::now())
        {
            return Err(AuthProviderError::InvalidCredentials);
        }

        let hash = user
            .password_hash
            .as_deref()
            .ok_or(AuthProviderError::InvalidCredentials)?;
        if !password::verify(password_input, hash)
            .map_err(|e| AuthProviderError::Unavailable(e.to_string()))?
        {
            let attempts = self
                .app_users
                .record_failed_attempt(user.id)
                .await
                .map_err(|e| AuthProviderError::Unavailable(e.to_string()))?;
            if attempts >= self.config.login_lockout_threshold {
                let until = DateTime::from_millis(
                    DateTime::now().timestamp_millis()
                        + self.config.login_lockout_duration.as_millis() as i64,
                );
                self.app_users
                    .set_locked_until(user.id, until)
                    .await
                    .map_err(|e| AuthProviderError::Unavailable(e.to_string()))?;
            }
            return Err(AuthProviderError::InvalidCredentials);
        }
        self.app_users
            .reset_lockout(user.id)
            .await
            .map_err(|e| AuthProviderError::Unavailable(e.to_string()))?;

        Ok(user)
    }
}
