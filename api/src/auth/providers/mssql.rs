//! MSSQL-backed external authentication.
//!
//! NOTE: stub. The real implementation (tiberius connection, parameterized
//! `@account` / `@password` binding, column resolution, shadow-user upsert) is
//! pending two decisions flagged in the change's design.md — the symmetric-
//! encryption key source for `password_encrypted`, and accepting the `tiberius`
//! dependency weight. Until then this provider fails closed with
//! `Unavailable`, so `external_db` Orgs get `EXTERNAL_AUTH_UNAVAILABLE` rather
//! than a silent wrong answer.

use std::sync::Arc;

use async_trait::async_trait;
use bson::oid::ObjectId;

use super::{AppAuthProvider, AuthProviderError};
use crate::config::Config;
use crate::db::app_users::AppUserRepository;
use crate::domain::{AppUser, ExternalAuthConfig};

#[allow(dead_code)] // fields consumed by the real implementation (group 3)
pub struct MssqlProvider {
    app_users: AppUserRepository,
    org_id: ObjectId,
    config: ExternalAuthConfig,
    server_config: Arc<Config>,
}

impl MssqlProvider {
    pub fn new(
        app_users: AppUserRepository,
        org_id: ObjectId,
        config: ExternalAuthConfig,
        server_config: Arc<Config>,
    ) -> Self {
        Self {
            app_users,
            org_id,
            config,
            server_config,
        }
    }
}

#[async_trait]
impl AppAuthProvider for MssqlProvider {
    async fn authenticate(
        &self,
        _account: &str,
        _password: &str,
    ) -> Result<AppUser, AuthProviderError> {
        Err(AuthProviderError::Unavailable(
            "mssql external auth provider is not yet implemented".to_string(),
        ))
    }
}
