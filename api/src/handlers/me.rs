use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum_extra::extract::CookieJar;
use bson::oid::ObjectId;
use serde::Deserialize;

use crate::auth::extractor::AuthContext;
use crate::auth::slug as slug_auth;
use crate::db::MembershipInsertError;
use crate::domain::{RemovalKind, Role};
use crate::error::{ApiError, ApiResult};
use crate::handlers::auth::{
    AuthResponse, MembershipDto, OrgDto, UserDto, build_auth_response, create_org_unique,
    enforce_join_cooldown, load_membership_orgs, validate_org_name,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateOrgRequest {
    pub org_name: String,
}

#[derive(Debug, Deserialize)]
pub struct JoinMembershipRequest {
    pub org_code: String,
}

#[derive(Debug, Deserialize)]
pub struct SwitchOrgRequest {
    pub org_id: String,
}

pub async fn me(State(state): State<AppState>, ctx: AuthContext) -> ApiResult<Json<AuthResponse>> {
    let user = state
        .db
        .dashboard_users
        .find_by_id(ctx.user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    let memberships = state
        .db
        .dashboard_memberships
        .list_by_user(user.id)
        .await?;
    let pairs = load_membership_orgs(&state, memberships).await?;

    let current_org = match ctx.current_org_id {
        Some(org_id) => pairs
            .iter()
            .find(|(_, o)| o.id == org_id)
            .map(|(_, o)| o.clone()),
        None => None,
    };

    Ok(Json(build_auth_response(user, pairs, current_org)))
}

/// `POST /me/orgs` — org-agnostic. Creates a new Org with the caller as owner,
/// inserts a membership row with role=admin, and points the caller's session
/// at the new Org.
pub async fn create_org(
    State(state): State<AppState>,
    jar: CookieJar,
    ctx: AuthContext,
    Json(req): Json<CreateOrgRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let _ = jar; // present for symmetry; cookie is already valid.

    validate_org_name(&req.org_name)?;

    let user = state
        .db
        .dashboard_users
        .find_by_id(ctx.user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    let org = create_org_unique(&state, &req.org_name, user.id).await?;

    if let Err(err) = state
        .db
        .dashboard_memberships
        .create(user.id, org.id, Role::Admin)
        .await
    {
        // Brand-new Org cannot collide on (user_id, org_id), but if anything
        // ever does we roll back the Org so the user retries cleanly.
        let _ = state.db.orgs.delete_by_id(org.id).await;
        return match err {
            MembershipInsertError::Duplicate => Err(ApiError::Internal),
            MembershipInsertError::Db(e) => Err(ApiError::Db(e)),
        };
    }

    state
        .db
        .dashboard_sessions
        .update_current_org(&ctx.session_token, Some(org.id))
        .await?;

    let memberships = state
        .db
        .dashboard_memberships
        .list_by_user(user.id)
        .await?;
    let pairs = load_membership_orgs(&state, memberships).await?;
    let current_org = pairs
        .iter()
        .find(|(_, o)| o.id == org.id)
        .map(|(_, o)| o.clone());

    Ok(Json(build_auth_response(user, pairs, current_org)))
}

/// `POST /me/memberships` — org-agnostic. Joins an existing Org as a member.
pub async fn join_membership(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(req): Json<JoinMembershipRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let user = state
        .db
        .dashboard_users
        .find_by_id(ctx.user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    let org = slug_auth::resolve_org_for_join(&state.db, &req.org_code).await?;

    // Cooldown applies to existing identities adding new memberships, mirroring
    // register mode=join.
    let email_key = user.email.trim().to_ascii_lowercase();
    enforce_join_cooldown(&state, org.id, &email_key).await?;

    match state
        .db
        .dashboard_memberships
        .create(user.id, org.id, Role::Member)
        .await
    {
        Ok(_) => {}
        Err(MembershipInsertError::Duplicate) => return Err(ApiError::AlreadyMember),
        Err(MembershipInsertError::Db(err)) => return Err(ApiError::Db(err)),
    }

    state
        .db
        .dashboard_sessions
        .update_current_org(&ctx.session_token, Some(org.id))
        .await?;

    let memberships = state
        .db
        .dashboard_memberships
        .list_by_user(user.id)
        .await?;
    let pairs = load_membership_orgs(&state, memberships).await?;
    let current_org = pairs
        .iter()
        .find(|(_, o)| o.id == org.id)
        .map(|(_, o)| o.clone());

    Ok(Json(build_auth_response(user, pairs, current_org)))
}

/// `POST /me/current-org` — org-agnostic. Switch the active Org for this
/// session. The target must be one the caller is a member of.
pub async fn switch_current_org(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(req): Json<SwitchOrgRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let target_id = ObjectId::parse_str(&req.org_id).map_err(|_| ApiError::NotAMember)?;

    let user = state
        .db
        .dashboard_users
        .find_by_id(ctx.user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    // Membership lookup also functions as the "is the caller in this Org?" gate.
    if state
        .db
        .dashboard_memberships
        .find_by_user_and_org(user.id, target_id)
        .await?
        .is_none()
    {
        return Err(ApiError::NotAMember);
    }

    state
        .db
        .dashboard_sessions
        .update_current_org(&ctx.session_token, Some(target_id))
        .await?;

    let memberships = state
        .db
        .dashboard_memberships
        .list_by_user(user.id)
        .await?;
    let pairs = load_membership_orgs(&state, memberships).await?;
    let current_org = pairs
        .iter()
        .find(|(_, o)| o.id == target_id)
        .map(|(_, o)| o.clone());

    Ok(Json(build_auth_response(user, pairs, current_org)))
}

/// `POST /me/leave` — leave `current_org` only. Requires an active Org.
/// Force-deletes only sessions whose `current_org_id` matches the left Org.
pub async fn leave(
    State(state): State<AppState>,
    jar: CookieJar,
    ctx: AuthContext,
) -> ApiResult<impl IntoResponse> {
    let (org_id, _role) = ctx.require_active_org()?;

    let user = state
        .db
        .dashboard_users
        .find_by_id(ctx.user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    let org = state
        .db
        .orgs
        .find_by_id(org_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if user.id == org.owner_id {
        return Err(ApiError::OwnerProtected);
    }

    let deleted = state
        .db
        .dashboard_memberships
        .delete(user.id, org.id)
        .await?;
    if deleted == 0 {
        // Membership vanished mid-flight — middleware would normally catch
        // this, but re-surface as Unauthorized for safety.
        return Err(ApiError::Unauthorized);
    }
    state
        .db
        .dashboard_sessions
        .delete_by_user_and_org(user.id, org.id)
        .await?;
    let email_key = user.email.trim().to_ascii_lowercase();
    state
        .db
        .removed_memberships
        .insert(org.id, &email_key, RemovalKind::Left)
        .await?;

    // The caller's own session was just deleted; nudge the browser cookie too.
    let cleared = jar.remove(crate::auth::extractor::build_clearing_cookie());
    Ok((StatusCode::NO_CONTENT, cleared))
}

/// Compatibility shim used elsewhere if a future flow needs the joined list.
#[allow(dead_code)]
pub(crate) fn empty_response() -> AuthResponse {
    AuthResponse {
        user: UserDto {
            id: String::new(),
            email: String::new(),
        },
        memberships: Vec::<MembershipDto>::new(),
        current_org: None::<OrgDto>,
        role: None,
    }
}
