use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use crate::auth::extractor::{AuthContext, RequireAdmin};
use crate::domain::{RemovalKind, Role};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct DashboardUserDto {
    pub id: String,
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub role: Role,
}

#[derive(Debug, Serialize)]
pub struct CooldownDto {
    pub email: String,
    pub removed_at: Option<String>,
    pub cooldown_until: Option<String>,
    pub removal_kind: RemovalKind,
}

pub async fn list_in_org(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> ApiResult<Json<Vec<DashboardUserDto>>> {
    let users = state.db.dashboard_users.list_in_org(ctx.org_id).await?;
    let out = users
        .into_iter()
        .map(|u| DashboardUserDto {
            id: u.id.to_hex(),
            email: u.email,
            role: u.role,
        })
        .collect();
    Ok(Json(out))
}

pub async fn update_role(
    State(state): State<AppState>,
    RequireAdmin(ctx): RequireAdmin,
    Path(id): Path<String>,
    Json(req): Json<UpdateRoleRequest>,
) -> ApiResult<Json<DashboardUserDto>> {
    let user_id = ObjectId::parse_str(&id).map_err(|_| ApiError::NotFound)?;

    // Cross-org targets must look like NotFound; loading first lets us guard owner
    // protection before mutating.
    let target = state
        .db
        .dashboard_users
        .find_by_id(user_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if target.org_id != ctx.org_id {
        return Err(ApiError::NotFound);
    }

    let org = state
        .db
        .orgs
        .find_by_id(ctx.org_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Owner is permanently admin: any attempt to demote them is rejected.
    if target.id == org.owner_id && matches!(req.role, Role::Member) {
        return Err(ApiError::OwnerProtected);
    }

    if target.role == req.role {
        return Ok(Json(DashboardUserDto {
            id: target.id.to_hex(),
            email: target.email,
            role: target.role,
        }));
    }

    let updated = state
        .db
        .dashboard_users
        .update_role(user_id, ctx.org_id, req.role)
        .await?;

    Ok(Json(DashboardUserDto {
        id: updated.id.to_hex(),
        email: updated.email,
        role: updated.role,
    }))
}

pub async fn remove(
    State(state): State<AppState>,
    RequireAdmin(ctx): RequireAdmin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let target_id = ObjectId::parse_str(&id).map_err(|_| ApiError::NotFound)?;

    // Self-removal must go through /me/leave so removal_kind is unambiguous.
    if target_id == ctx.user_id {
        return Err(ApiError::Forbidden);
    }

    let target = state
        .db
        .dashboard_users
        .find_by_id(target_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if target.org_id != ctx.org_id {
        return Err(ApiError::NotFound);
    }

    let org = state
        .db
        .orgs
        .find_by_id(ctx.org_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if target.id == org.owner_id {
        return Err(ApiError::OwnerProtected);
    }

    cascade_delete_user(&state, &target).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_cooldowns(
    State(state): State<AppState>,
    RequireAdmin(ctx): RequireAdmin,
) -> ApiResult<Json<Vec<CooldownDto>>> {
    let markers = state
        .db
        .removed_memberships
        .list_for_org(ctx.org_id)
        .await?;
    let out = markers
        .into_iter()
        .map(|m| CooldownDto {
            email: m.email,
            removed_at: m.removed_at.try_to_rfc3339_string().ok(),
            cooldown_until: m.cooldown_until.try_to_rfc3339_string().ok(),
            removal_kind: m.removal_kind,
        })
        .collect();
    Ok(Json(out))
}

pub async fn clear_cooldown(
    State(state): State<AppState>,
    RequireAdmin(ctx): RequireAdmin,
    Path(email): Path<String>,
) -> ApiResult<StatusCode> {
    let key = email.trim().to_ascii_lowercase();
    state
        .db
        .removed_memberships
        .delete(ctx.org_id, &key)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Cascade: sessions → user → marker. The order matters per design: if the
/// process crashes mid-cascade, leaving a still-valid user with no sessions
/// is less harmful than leaving a deleted user with a stale "active" session.
pub(crate) async fn cascade_delete_user(
    state: &AppState,
    target: &crate::domain::DashboardUser,
) -> ApiResult<()> {
    state
        .db
        .dashboard_sessions
        .delete_all_by_user_id(target.id)
        .await?;
    state.db.dashboard_users.delete_by_id(target.id).await?;
    let email_key = target.email.trim().to_ascii_lowercase();
    if let Err(err) = state
        .db
        .removed_memberships
        .insert(target.org_id, &email_key, RemovalKind::Kicked)
        .await
    {
        tracing::error!(?err, "failed to write removed_memberships marker after cascade delete");
        return Err(err);
    }
    Ok(())
}

/// Same cascade as `cascade_delete_user`, but stamps the marker with
/// `RemovalKind::Left` so the audit trail distinguishes self-leave from kick.
pub(crate) async fn cascade_self_leave(
    state: &AppState,
    target: &crate::domain::DashboardUser,
) -> ApiResult<()> {
    state
        .db
        .dashboard_sessions
        .delete_all_by_user_id(target.id)
        .await?;
    state.db.dashboard_users.delete_by_id(target.id).await?;
    let email_key = target.email.trim().to_ascii_lowercase();
    if let Err(err) = state
        .db
        .removed_memberships
        .insert(target.org_id, &email_key, RemovalKind::Left)
        .await
    {
        tracing::error!(?err, "failed to write removed_memberships marker after self-leave");
        return Err(err);
    }
    Ok(())
}
