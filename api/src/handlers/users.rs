use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use crate::auth::extractor::{RequireActiveOrg, RequireAdmin};
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
    active: RequireActiveOrg,
) -> ApiResult<Json<Vec<DashboardUserDto>>> {
    let org_id = active.org_id;
    let memberships = state.db.dashboard_memberships.list_by_org(org_id).await?;
    let mut out = Vec::with_capacity(memberships.len());
    for m in memberships {
        // Best-effort: skip rows whose user identity has gone missing.
        if let Some(user) = state.db.dashboard_users.find_by_id(m.user_id).await? {
            out.push(DashboardUserDto {
                id: user.id.to_hex(),
                email: user.email,
                role: m.role,
            });
        }
    }
    Ok(Json(out))
}

pub async fn update_role(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id): Path<String>,
    Json(req): Json<UpdateRoleRequest>,
) -> ApiResult<Json<DashboardUserDto>> {
    let org_id = active.org_id;
    let target_id = ObjectId::parse_str(&id).map_err(|_| ApiError::NotFound)?;

    // Cross-org targets must look like NotFound; loading the membership first
    // gives us both the existence check and the current role for owner-promote
    // no-op handling.
    let membership = state
        .db
        .dashboard_memberships
        .find_by_user_and_org(target_id, org_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let target_user = state
        .db
        .dashboard_users
        .find_by_id(target_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let org = state
        .db
        .orgs
        .find_by_id(org_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Owner is permanently admin in this Org: any attempt to demote them is
    // rejected. Promotion to admin (if anyone tries) is a no-op.
    if target_user.id == org.owner_id && matches!(req.role, Role::Member) {
        return Err(ApiError::OwnerProtected);
    }

    if membership.role == req.role {
        return Ok(Json(DashboardUserDto {
            id: target_user.id.to_hex(),
            email: target_user.email,
            role: membership.role,
        }));
    }

    let updated = state
        .db
        .dashboard_memberships
        .update_role(target_id, org_id, req.role)
        .await?;

    Ok(Json(DashboardUserDto {
        id: target_user.id.to_hex(),
        email: target_user.email,
        role: updated.role,
    }))
}

pub async fn remove(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let org_id = active.org_id;
    let target_id = ObjectId::parse_str(&id).map_err(|_| ApiError::NotFound)?;

    // Self-removal must go through /me/leave so removal_kind is unambiguous.
    if target_id == active.ctx.user_id {
        return Err(ApiError::Forbidden);
    }

    // Membership must exist in caller's current Org. Cross-org or strangers
    // collapse to NotFound.
    if state
        .db
        .dashboard_memberships
        .find_by_user_and_org(target_id, org_id)
        .await?
        .is_none()
    {
        return Err(ApiError::NotFound);
    }

    let target_user = state
        .db
        .dashboard_users
        .find_by_id(target_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let org = state
        .db
        .orgs
        .find_by_id(org_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if target_user.id == org.owner_id {
        return Err(ApiError::OwnerProtected);
    }

    cascade_remove_membership(&state, &target_user, org_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_cooldowns(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
) -> ApiResult<Json<Vec<CooldownDto>>> {
    let markers = state
        .db
        .removed_memberships
        .list_for_org(active.org_id)
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
    RequireAdmin(active): RequireAdmin,
    Path(email): Path<String>,
) -> ApiResult<StatusCode> {
    let key = email.trim().to_ascii_lowercase();
    state
        .db
        .removed_memberships
        .delete(active.org_id, &key)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Cascade an admin-initiated removal from a single Org: drop membership row,
/// drop sessions scoped to this Org, write the kicked-marker. Identity, other
/// memberships, and other-Org sessions are preserved.
pub(crate) async fn cascade_remove_membership(
    state: &AppState,
    target: &crate::domain::DashboardUser,
    org_id: ObjectId,
) -> ApiResult<()> {
    state
        .db
        .dashboard_memberships
        .delete(target.id, org_id)
        .await?;
    state
        .db
        .dashboard_sessions
        .delete_by_user_and_org(target.id, org_id)
        .await?;
    let email_key = target.email.trim().to_ascii_lowercase();
    state
        .db
        .removed_memberships
        .insert(org_id, &email_key, RemovalKind::Kicked)
        .await?;
    Ok(())
}
