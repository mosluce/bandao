//! `org-join-requests` capability — admin approves new members instead of
//! letting `register mode=join` / `POST /me/memberships` create rows
//! directly. Submitter side: `POST /me/join-requests`,
//! `GET /me/join-requests`, `DELETE /me/join-requests/:id`. Admin side:
//! `GET /orgs/me/join-requests`, `POST .../approve`, `POST .../reject`.
//!
//! Submission shares an `submit_inner` that the legacy
//! `POST /me/memberships` and `register mode=join` both call so the three
//! entry paths produce identical state.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use crate::auth::extractor::{AuthContext, RequireAdmin};
use crate::auth::slug as slug_auth;
use crate::db::{JoinRequestInsertError, MembershipInsertError};
use crate::domain::{JoinRequest, JoinRequestStatus, Org, Role};
use crate::error::{ApiError, ApiResult};
use crate::handlers::auth::enforce_join_cooldown;
use crate::state::AppState;

const APPLICATION_MESSAGE_MAX: usize = 500;
const REJECTION_REASON_MAX: usize = 500;

/// Body of `POST /me/join-requests` (and the legacy
/// `POST /me/memberships` that forwards to the same submit path).
#[derive(Debug, Deserialize)]
pub struct SubmitJoinRequestRequest {
    pub org_code: String,
    #[serde(default)]
    pub application_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RejectJoinRequestRequest {
    #[serde(default)]
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListJoinRequestsQuery {
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JoinRequestOrgRef {
    pub id: String,
    pub name: String,
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct JoinRequestDto {
    pub id: String,
    pub org: JoinRequestOrgRef,
    pub status: JoinRequestStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    pub requested_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<String>,
}

impl JoinRequestDto {
    pub fn from_pair(req: &JoinRequest, org: &Org) -> Self {
        Self {
            id: req.id.to_hex(),
            org: JoinRequestOrgRef {
                id: org.id.to_hex(),
                name: org.name.clone(),
                code: org.code.clone(),
            },
            status: req.status,
            application_message: req.application_message.clone(),
            rejection_reason: req.rejection_reason.clone(),
            requested_at: req.requested_at.try_to_rfc3339_string().unwrap_or_default(),
            decided_at: req.decided_at.and_then(|t| t.try_to_rfc3339_string().ok()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OrgPendingJoinRequestDto {
    pub id: String,
    pub user_id: String,
    pub email: String,
    pub status: JoinRequestStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    pub requested_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<String>,
}

/// Shared submit path. Caller has already authenticated the user; we
/// look the user up to grab the email for the cooldown check, then run
/// the same gate / insert / error mapping for both `/me/join-requests`
/// and `register mode=join`.
pub(crate) async fn submit_inner(
    state: &AppState,
    user_id: ObjectId,
    user_email: &str,
    org_code: &str,
    application_message: Option<String>,
) -> ApiResult<JoinRequest> {
    if let Some(ref msg) = application_message
        && msg.chars().count() > APPLICATION_MESSAGE_MAX
    {
        return Err(ApiError::Validation(format!(
            "application_message must be <= {APPLICATION_MESSAGE_MAX} characters"
        )));
    }

    let org = slug_auth::resolve_org_for_join(&state.db, org_code).await?;

    let email_key = user_email.trim().to_ascii_lowercase();
    enforce_join_cooldown(state, org.id, &email_key).await?;

    if state
        .db
        .dashboard_memberships
        .find_by_user_and_org(user_id, org.id)
        .await?
        .is_some()
    {
        return Err(ApiError::AlreadyMember);
    }

    match state
        .db
        .join_requests
        .insert_pending(user_id, org.id, application_message)
        .await
    {
        Ok(row) => Ok(row),
        Err(JoinRequestInsertError::Duplicate) => Err(ApiError::JoinRequestPending),
        Err(JoinRequestInsertError::Db(err)) => Err(ApiError::Db(err)),
    }
}

pub async fn submit(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(req): Json<SubmitJoinRequestRequest>,
) -> ApiResult<(StatusCode, Json<JoinRequestDto>)> {
    let user = state
        .db
        .dashboard_users
        .find_by_id(ctx.user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    let row = submit_inner(
        &state,
        user.id,
        &user.email,
        &req.org_code,
        req.application_message,
    )
    .await?;

    let org = state
        .db
        .orgs
        .find_by_id(row.org_id)
        .await?
        .ok_or(ApiError::Internal)?;
    Ok((
        StatusCode::CREATED,
        Json(JoinRequestDto::from_pair(&row, &org)),
    ))
}

pub async fn list_mine(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> ApiResult<Json<Vec<JoinRequestDto>>> {
    let rows = state.db.join_requests.list_by_user(ctx.user_id).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        if let Some(org) = state.db.orgs.find_by_id(row.org_id).await? {
            out.push(JoinRequestDto::from_pair(row, &org));
        }
    }
    Ok(Json(out))
}

pub async fn cancel(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id_hex): Path<String>,
) -> ApiResult<StatusCode> {
    let id = ObjectId::parse_str(&id_hex).map_err(|_| ApiError::NotFound)?;

    // Verify the row exists & ownership / state for distinct error mapping.
    let row = state
        .db
        .join_requests
        .find_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if row.user_id != ctx.user_id {
        return Err(ApiError::NotFound);
    }
    if row.status != JoinRequestStatus::Pending {
        return Err(ApiError::InvalidState);
    }

    state
        .db
        .join_requests
        .cancel_by_owner(id, ctx.user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_for_org(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Query(q): Query<ListJoinRequestsQuery>,
) -> ApiResult<Json<Vec<OrgPendingJoinRequestDto>>> {
    let status = parse_status(q.status.as_deref()).unwrap_or(JoinRequestStatus::Pending);

    let rows = state
        .db
        .join_requests
        .list_by_org_with_status(active.org_id, status)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let user = state
            .db
            .dashboard_users
            .find_by_id(row.user_id)
            .await?
            .map(|u| u.email)
            .unwrap_or_else(|| String::from("(unknown)"));
        out.push(OrgPendingJoinRequestDto {
            id: row.id.to_hex(),
            user_id: row.user_id.to_hex(),
            email: user,
            status: row.status,
            application_message: row.application_message.clone(),
            rejection_reason: row.rejection_reason.clone(),
            requested_at: row.requested_at.try_to_rfc3339_string().unwrap_or_default(),
            decided_at: row.decided_at.and_then(|t| t.try_to_rfc3339_string().ok()),
        });
    }
    Ok(Json(out))
}

pub async fn approve(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id_hex): Path<String>,
) -> ApiResult<StatusCode> {
    let id = ObjectId::parse_str(&id_hex).map_err(|_| ApiError::NotFound)?;

    let row = state
        .db
        .join_requests
        .find_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if row.org_id != active.org_id {
        return Err(ApiError::NotFound);
    }
    if row.status != JoinRequestStatus::Pending {
        return Err(ApiError::InvalidState);
    }

    // Cooldown re-check (defense-in-depth — see design D5).
    let user = state
        .db
        .dashboard_users
        .find_by_id(row.user_id)
        .await?
        .ok_or(ApiError::Internal)?;
    let email_key = user.email.trim().to_ascii_lowercase();
    enforce_join_cooldown(&state, row.org_id, &email_key).await?;

    // Atomic-enough: insert membership first (idempotent — duplicate index
    // means user is already in, treat as success), then flip request status.
    // If the second write fails the request is still pending and admin can
    // retry. This matches the "fallback for single-node mongo" path noted
    // in design D3; if/when we adopt mongo replica set transactions we can
    // wrap both in `ClientSession::with_transaction`.
    match state
        .db
        .dashboard_memberships
        .create(row.user_id, row.org_id, Role::Member)
        .await
    {
        Ok(_) => {}
        Err(MembershipInsertError::Duplicate) => {
            // Already a member somehow — treat as success (idempotent).
        }
        Err(MembershipInsertError::Db(err)) => return Err(ApiError::Db(err)),
    }

    state
        .db
        .join_requests
        .decide(id, JoinRequestStatus::Approved, active.ctx.user_id, None)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn reject(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id_hex): Path<String>,
    Json(req): Json<RejectJoinRequestRequest>,
) -> ApiResult<StatusCode> {
    let id = ObjectId::parse_str(&id_hex).map_err(|_| ApiError::NotFound)?;

    if let Some(ref reason) = req.rejection_reason
        && reason.chars().count() > REJECTION_REASON_MAX
    {
        return Err(ApiError::Validation(format!(
            "rejection_reason must be <= {REJECTION_REASON_MAX} characters"
        )));
    }

    let row = state
        .db
        .join_requests
        .find_by_id(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if row.org_id != active.org_id {
        return Err(ApiError::NotFound);
    }
    if row.status != JoinRequestStatus::Pending {
        return Err(ApiError::InvalidState);
    }

    state
        .db
        .join_requests
        .decide(
            id,
            JoinRequestStatus::Rejected,
            active.ctx.user_id,
            req.rejection_reason,
        )
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn parse_status(raw: Option<&str>) -> Option<JoinRequestStatus> {
    match raw {
        Some("pending") => Some(JoinRequestStatus::Pending),
        Some("approved") => Some(JoinRequestStatus::Approved),
        Some("rejected") => Some(JoinRequestStatus::Rejected),
        Some("cancelled") => Some(JoinRequestStatus::Cancelled),
        _ => None,
    }
}
