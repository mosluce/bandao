//! `/orgs/me/api-tokens` — admin-only CRUD for Org-scoped machine API
//! tokens. See `openspec/specs/org-api-tokens/spec.md`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use crate::auth::api_token;
use crate::auth::extractor::RequireAdmin;
use crate::domain::{ApiTokenScope, ApiTokenStatus, OrgApiToken};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const NAME_MIN: usize = 1;
const NAME_MAX: usize = 60;

#[derive(Debug, Serialize)]
pub struct ApiTokenDto {
    pub id: String,
    pub name: String,
    pub scopes: Vec<ApiTokenScope>,
    pub status: ApiTokenStatus,
    pub token_prefix: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotated_at: Option<String>,
}

impl ApiTokenDto {
    fn from_token(t: &OrgApiToken) -> Self {
        Self {
            id: t.id.to_hex(),
            name: t.name.clone(),
            scopes: t.scopes.clone(),
            status: t.status,
            token_prefix: t.token_prefix.clone(),
            created_at: t.created_at.try_to_rfc3339_string().unwrap_or_default(),
            last_used_at: t.last_used_at.and_then(|d| d.try_to_rfc3339_string().ok()),
            rotated_at: t.rotated_at.and_then(|d| d.try_to_rfc3339_string().ok()),
        }
    }
}

/// Creation/rotation response. `secret` is the plaintext token — present
/// exactly once, never returned by any other endpoint.
#[derive(Debug, Serialize)]
pub struct ApiTokenSecretResponse {
    pub token: ApiTokenDto,
    pub secret: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiTokenRequest {
    pub name: String,
    pub scopes: Vec<ApiTokenScope>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiTokenStatusRequest {
    pub status: ApiTokenStatus,
}

/// `GET /orgs/me/api-tokens` — admin-only, scoped to `current_org`. Never
/// includes `token_hash` or the plaintext secret.
pub async fn list(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
) -> ApiResult<Json<Vec<ApiTokenDto>>> {
    let tokens = state.db.org_api_tokens.list_by_org(active.org_id).await?;
    let mut out: Vec<ApiTokenDto> = tokens.iter().map(ApiTokenDto::from_token).collect();
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(out))
}

/// `POST /orgs/me/api-tokens` — admin-only. Requires a non-empty `name` and
/// at least one known `scope`. Returns the plaintext secret exactly once.
pub async fn create(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Json(req): Json<CreateApiTokenRequest>,
) -> ApiResult<(StatusCode, Json<ApiTokenSecretResponse>)> {
    let name = req.name.trim();
    validate_name(name)?;
    if req.scopes.is_empty() {
        return Err(ApiError::Validation(
            "scopes must include at least one value".to_string(),
        ));
    }

    let (plaintext, hash, prefix) = api_token::generate();
    let token = state
        .db
        .org_api_tokens
        .insert(
            active.org_id,
            name,
            &hash,
            &prefix,
            req.scopes,
            active.ctx.user_id,
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiTokenSecretResponse {
            token: ApiTokenDto::from_token(&token),
            secret: plaintext,
        }),
    ))
}

/// `POST /orgs/me/api-tokens/:id/rotate` — admin-only, org-scoped. Keeps
/// `name`/`scopes`, replaces the secret, returns the new plaintext once.
pub async fn rotate(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id): Path<String>,
) -> ApiResult<Json<ApiTokenSecretResponse>> {
    let target_id = parse_id(&id)?;
    // Confirm it exists in this Org before rotating — cross-Org collapses to
    // NOT_FOUND, same pattern as `/app-users/:id`.
    load_in_org(&state, active.org_id, target_id).await?;

    let (plaintext, hash, prefix) = api_token::generate();
    let token = state
        .db
        .org_api_tokens
        .rotate(target_id, active.org_id, &hash, &prefix)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(ApiTokenSecretResponse {
        token: ApiTokenDto::from_token(&token),
        secret: plaintext,
    }))
}

/// `PATCH /orgs/me/api-tokens/:id` — admin-only, org-scoped. Body
/// `{ "status": "active" | "disabled" }`.
pub async fn update_status(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id): Path<String>,
    Json(req): Json<UpdateApiTokenStatusRequest>,
) -> ApiResult<Json<ApiTokenDto>> {
    let target_id = parse_id(&id)?;
    load_in_org(&state, active.org_id, target_id).await?;

    let token = state
        .db
        .org_api_tokens
        .update_status(target_id, active.org_id, req.status)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(ApiTokenDto::from_token(&token)))
}

/// `DELETE /orgs/me/api-tokens/:id` — admin-only, org-scoped. Irreversible.
pub async fn delete(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let target_id = parse_id(&id)?;
    let deleted = state
        .db
        .org_api_tokens
        .delete(target_id, active.org_id)
        .await?;
    if deleted == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

fn parse_id(id: &str) -> ApiResult<ObjectId> {
    ObjectId::parse_str(id).map_err(|_| ApiError::NotFound)
}

async fn load_in_org(
    state: &AppState,
    current_org_id: ObjectId,
    target_id: ObjectId,
) -> ApiResult<OrgApiToken> {
    state
        .db
        .org_api_tokens
        .find_by_id_and_org(target_id, current_org_id)
        .await?
        .ok_or(ApiError::NotFound)
}

fn validate_name(value: &str) -> ApiResult<()> {
    let len = value.chars().count();
    if !(NAME_MIN..=NAME_MAX).contains(&len) {
        return Err(ApiError::Validation(format!(
            "name length must be {NAME_MIN}..={NAME_MAX}"
        )));
    }
    Ok(())
}
