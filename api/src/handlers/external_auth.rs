//! Admin endpoints for configuring external-database App-user auth:
//! `PUT /orgs/me/external-auth` (set auth source + connection/query config) and
//! `POST /orgs/me/external-auth/test-login` (dry-run the full provider flow).
//! Both are admin-only and scoped to `current_org`.

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::auth::extractor::RequireAdmin;
use crate::auth::providers::{self, mssql::MssqlProvider};
use crate::domain::{EncryptMode, ExternalAuthConfig, OrgAuthSource};
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
    /// Transport encryption; absent → `Optional`.
    #[serde(default)]
    pub encrypt: EncryptMode,
    /// Trust an otherwise-invalid server cert; absent → `true`.
    #[serde(default = "default_trust_server_certificate")]
    pub trust_server_certificate: bool,
    /// Unparameterized "list everyone" query for `POST
    /// /orgs/me/external-auth/sync`. Absent → manual sync stays unavailable.
    #[serde(default)]
    pub list_query: Option<String>,
}

fn default_trust_server_certificate() -> bool {
    true
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
    Ok(Json(OrgDto::from_org_as_admin(&updated)))
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

#[derive(Debug, Serialize)]
pub struct SkippedRow {
    pub row_index: usize,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct SyncResponse {
    pub total_rows: usize,
    pub created: usize,
    pub updated: usize,
    pub skipped: Vec<SkippedRow>,
}

/// `POST /orgs/me/external-auth/sync` — admin-only, scoped to `current_org`,
/// available only when `current_org.auth_source == external_db`. Runs the
/// Org's stored `list_query` and upserts a shadow `AppUser` per resolved
/// row — see the `external-db-auth` spec's "Admin can manually sync the
/// external user roster" requirement for the exact create/update/skip
/// semantics. Does NOT touch `last_login_at` and does NOT modify or remove
/// any local user absent from the result.
pub async fn sync(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
) -> ApiResult<Json<SyncResponse>> {
    let org = state
        .db
        .orgs
        .find_by_id(active.org_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    if !matches!(org.auth_source(), OrgAuthSource::ExternalDb) {
        return Err(ApiError::ExternalAuthNotEnabled);
    }
    let cfg = org.external_auth().ok_or_else(|| {
        ApiError::Validation("external_auth configuration is required".to_string())
    })?;
    let list_query = cfg
        .list_query
        .clone()
        .filter(|q| !q.trim().is_empty())
        .ok_or_else(|| ApiError::Validation("list_query is not configured".to_string()))?;

    let provider = MssqlProvider::new(
        state.db.app_users.clone(),
        active.org_id,
        cfg,
        state.config.clone(),
    );
    let rows = provider.list_identities(&list_query).await.map_err(|e| {
        let msg = match e {
            providers::AuthProviderError::Unavailable(msg) => msg,
            // list_identities takes no credentials, so this arm shouldn't be
            // reachable — handled instead of panicking, in case that ever
            // changes.
            providers::AuthProviderError::InvalidCredentials => {
                "unexpected credential error during sync".to_string()
            }
        };
        ApiError::ExternalAuthSyncFailed(msg)
    })?;

    let mut created = 0usize;
    let mut updated = 0usize;
    let mut skipped = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let Some(external_key) = row.external_key.as_deref().filter(|k| !k.trim().is_empty())
        else {
            skipped.push(SkippedRow {
                row_index,
                reason: "key column is empty or null".to_string(),
            });
            continue;
        };
        let display_name = row.display_name.as_deref().unwrap_or(external_key);
        match state
            .db
            .app_users
            .sync_upsert_shadow(active.org_id, external_key, display_name)
            .await?
        {
            crate::db::SyncUpsertOutcome::Created => created += 1,
            crate::db::SyncUpsertOutcome::Updated => updated += 1,
        }
    }

    Ok(Json(SyncResponse {
        total_rows: rows.len(),
        created,
        updated,
        skipped,
    }))
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
    if let Some(list_query) = &input.list_query {
        providers::validate_list_query_settings(&input.driver, list_query)
            .map_err(ApiError::Validation)?;
    }

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
        encrypt: input.encrypt,
        trust_server_certificate: input.trust_server_certificate,
        list_query: input.list_query,
    })
}
