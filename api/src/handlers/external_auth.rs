//! Admin endpoints for configuring external-database App-user auth:
//! `PUT /orgs/me/external-auth` (set auth source + connection/query config) and
//! `POST /orgs/me/external-auth/test-login` (dry-run the full provider flow).
//! Both are admin-only and scoped to `current_org`.

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::auth::extractor::RequireAdmin;
use crate::auth::providers::{self, mssql::MssqlProvider};
use crate::domain::{ExternalAuthConfig, OrgAuthSource};
use crate::error::{ApiError, ApiResult};
use crate::handlers::auth::OrgDto;
use crate::state::AppState;

/// Connection + query settings as submitted by an admin. `password` is
/// write-only: `Some` replaces the stored password; `None` keeps the existing
/// one (so editing other fields doesn't require re-entering the password).
#[derive(Debug, Deserialize)]
pub struct ExternalAuthInput {
    pub driver: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    pub query: String,
    pub key_col: String,
    pub display_col: String,
}

#[derive(Debug, Deserialize)]
pub struct ConfigureRequest {
    pub auth_source: OrgAuthSource,
    #[serde(default)]
    pub external_auth: Option<ExternalAuthInput>,
}

/// `PUT /orgs/me/external-auth` — set the Org's auth source and, when switching
/// to / editing `external_db`, its connection + query config. Switching to
/// `internal` keeps any stored config so switching back restores it.
pub async fn configure(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Json(req): Json<ConfigureRequest>,
) -> ApiResult<Json<OrgDto>> {
    let org = state
        .db
        .orgs
        .find_by_id(active.org_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let external_config: Option<ExternalAuthConfig> = match req.auth_source {
        OrgAuthSource::ExternalDb => match req.external_auth {
            // New / edited config: validate, encrypt the password (or keep the
            // stored one), persist.
            Some(input) => Some(build_config(&state, &org, input)?),
            // Switch to external using the already-stored config — require one
            // and re-validate it.
            None => {
                let existing = org.external_auth().ok_or_else(|| {
                    ApiError::Validation("external_auth configuration is required".to_string())
                })?;
                providers::validate_query_settings(
                    &existing.driver,
                    &existing.query,
                    &existing.key_col,
                    &existing.display_col,
                )
                .map_err(ApiError::Validation)?;
                None
            }
        },
        // Going internal: just flip the source, leave any stored config intact.
        OrgAuthSource::Internal => None,
    };

    let updated = state
        .db
        .orgs
        .set_auth_config(active.org_id, req.auth_source, external_config.as_ref())
        .await?;
    Ok(Json(OrgDto::from_org(&updated)))
}

#[derive(Debug, Deserialize)]
pub struct TestLoginRequest {
    pub external_auth: ExternalAuthInput,
    pub test_account: String,
    pub test_password: String,
}

/// Shape of the dry-run result. `connected` distinguishes a working
/// configuration that simply didn't match the test credentials (`matched:
/// false`) from a broken one (`connected: false` + `error`).
#[derive(Debug, Serialize)]
pub struct TestLoginResponse {
    pub connected: bool,
    pub matched: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `POST /orgs/me/external-auth/test-login` — run the full provider flow against
/// the submitted config with test credentials, returning the resolved identity
/// or a specific diagnostic. Creates NO session and NO shadow user; the test
/// password is never persisted or logged.
pub async fn test_login(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Json(req): Json<TestLoginRequest>,
) -> ApiResult<Json<TestLoginResponse>> {
    let org = state
        .db
        .orgs
        .find_by_id(active.org_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let cfg = build_config(&state, &org, req.external_auth)?;
    let provider = MssqlProvider::new(
        state.db.app_users.clone(),
        active.org_id,
        cfg,
        state.config.clone(),
    );

    // `resolve_identity` runs connect + query + column resolution WITHOUT
    // touching the database (no upsert, no session).
    match provider
        .resolve_identity(&req.test_account, &req.test_password)
        .await
    {
        Ok(Some(identity)) => Ok(Json(TestLoginResponse {
            connected: true,
            matched: true,
            external_key: Some(identity.external_key),
            display_name: Some(identity.display_name),
            error: None,
        })),
        Ok(None) => Ok(Json(TestLoginResponse {
            connected: true,
            matched: false,
            external_key: None,
            display_name: None,
            error: None,
        })),
        Err(providers::AuthProviderError::InvalidCredentials) => Ok(Json(TestLoginResponse {
            connected: true,
            matched: false,
            external_key: None,
            display_name: None,
            error: None,
        })),
        Err(providers::AuthProviderError::Unavailable(msg)) => Ok(Json(TestLoginResponse {
            connected: false,
            matched: false,
            external_key: None,
            display_name: None,
            error: Some(msg),
        })),
    }
}

/// Validate the submitted settings and resolve an [`ExternalAuthConfig`],
/// encrypting a freshly-supplied password or reusing the stored ciphertext.
fn build_config(
    state: &AppState,
    org: &crate::domain::Org,
    input: ExternalAuthInput,
) -> ApiResult<ExternalAuthConfig> {
    providers::validate_query_settings(
        &input.driver,
        &input.query,
        &input.key_col,
        &input.display_col,
    )
    .map_err(ApiError::Validation)?;

    let password_encrypted = match input.password {
        Some(pw) => state.config.secret_box()?.encrypt(&pw)?,
        None => org
            .external_auth()
            .map(|c| c.password_encrypted)
            .filter(|c| !c.is_empty())
            .ok_or_else(|| ApiError::Validation("connection password is required".to_string()))?,
    };

    Ok(ExternalAuthConfig {
        driver: input.driver,
        host: input.host,
        port: input.port,
        database: input.database,
        username: input.username,
        password_encrypted,
        query: input.query,
        key_col: input.key_col,
        display_col: input.display_col,
    })
}
