//! App-user authentication providers.
//!
//! Every App login flows through an [`AppAuthProvider`] selected by the Org's
//! `auth_source` (see [`provider_for`]). Providers own credential verification
//! and resolve the caller to a local [`AppUser`] row that anchors the session,
//! check-in events, and location pings — so the login handler is identical
//! regardless of where credentials actually live. `internal` uses the built-in
//! Mongo + password-hash flow; `external_db` delegates to a driver-specific
//! provider (MSSQL is the only one implemented).

pub mod internal;
pub mod mssql;

use async_trait::async_trait;

use crate::domain::{AppUser, Org, OrgAuthSource};
use crate::state::AppState;

/// Outcome of a provider failing to authenticate.
#[derive(Debug)]
pub enum AuthProviderError {
    /// The credentials did not match (unknown account, wrong password, or the
    /// resolved local user is disabled). Collapses to `INVALID_CREDENTIALS` at
    /// the handler so callers cannot distinguish the sub-cases.
    InvalidCredentials,
    /// Verification could not be completed for a non-credential reason —
    /// connection failure, query error, missing/malformed config, unsupported
    /// driver. Carries a diagnostic for admin-facing surfaces (test-login); the
    /// end-user login path collapses it to `EXTERNAL_AUTH_UNAVAILABLE` without
    /// leaking the detail.
    Unavailable(String),
}

/// Resolves credentials to a local [`AppUser`] whose `_id` the caller uses to
/// issue the session.
#[async_trait]
pub trait AppAuthProvider: Send + Sync {
    async fn authenticate(
        &self,
        account: &str,
        password: &str,
    ) -> Result<AppUser, AuthProviderError>;
}

/// Build the provider for `org` based on its `auth_source`. Returns
/// `Unavailable` when an `external_db` Org has missing/malformed config or an
/// unsupported driver — the login handler maps that to
/// `EXTERNAL_AUTH_UNAVAILABLE`.
pub fn provider_for(
    state: &AppState,
    org: &Org,
) -> Result<Box<dyn AppAuthProvider>, AuthProviderError> {
    match org.auth_source() {
        OrgAuthSource::Internal => Ok(Box::new(internal::InternalProvider::new(
            state.db.app_users.clone(),
            org.id,
        ))),
        OrgAuthSource::ExternalDb => {
            let cfg = org.external_auth().ok_or_else(|| {
                AuthProviderError::Unavailable("external_auth config is missing".to_string())
            })?;
            match cfg.driver.as_str() {
                "mssql" => Ok(Box::new(mssql::MssqlProvider::new(
                    state.db.app_users.clone(),
                    org.id,
                    cfg,
                    state.config.clone(),
                ))),
                other => Err(AuthProviderError::Unavailable(format!(
                    "unsupported external auth driver: {other}"
                ))),
            }
        }
    }
}
