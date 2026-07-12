use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::auth::app_extractor::AppAuthContext;
use crate::auth::providers::{self, AuthProviderError};
use crate::auth::{password, slug as slug_auth};
use crate::domain::AppUserStatus;
use crate::error::{ApiError, ApiResult};
use crate::handlers::app_dto::{
    AppLoginRequest, AppLoginResponse, AppMeResponse, AppPasswordChangeRequest, AppUserDto,
};
use crate::handlers::auth::OrgDto;
use crate::state::AppState;

const MIN_PASSWORD_LEN: usize = 8;

/// `POST /app/auth/login` — public. Resolves `org_code` (random 10-char code,
/// active slug, or grace-period slug), looks up the AppUser by
/// `(org_id, username_lower)`, verifies the password and `status == active`,
/// issues an `app_sessions` row, and returns the token + identity context.
///
/// Every failure mode collapses to `INVALID_CREDENTIALS` so the caller cannot
/// distinguish "unknown org" from "unknown username" from "wrong password"
/// from "disabled".
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<AppLoginRequest>,
) -> ApiResult<Json<AppLoginResponse>> {
    // Resolve `org_code` to an Org. Any failure here surfaces as
    // INVALID_CREDENTIALS so we don't leak whether the Org exists.
    let org = match slug_auth::resolve_org_for_join(&state.db, &req.org_code).await {
        Ok(org) => org,
        Err(_) => return Err(ApiError::InvalidCredentials),
    };

    // Verify credentials through the provider selected by the Org's auth
    // source. `internal` looks up + verifies the local hash; `external_db`
    // delegates to the driver-specific provider, provisioning a shadow user.
    // Credential failures collapse to INVALID_CREDENTIALS; a provider that
    // can't complete verification surfaces as EXTERNAL_AUTH_UNAVAILABLE.
    let provider = match providers::provider_for(&state, &org) {
        Ok(provider) => provider,
        Err(AuthProviderError::InvalidCredentials) => return Err(ApiError::InvalidCredentials),
        Err(AuthProviderError::Unavailable(_)) => return Err(ApiError::ExternalAuthUnavailable),
    };
    let user = match provider.authenticate(&req.username, &req.password).await {
        Ok(user) => user,
        Err(AuthProviderError::InvalidCredentials) => return Err(ApiError::InvalidCredentials),
        Err(AuthProviderError::Unavailable(_)) => return Err(ApiError::ExternalAuthUnavailable),
    };

    // Disabled AppUser — same generic error as wrong password / unknown user.
    if !matches!(user.status, AppUserStatus::Active) {
        return Err(ApiError::InvalidCredentials);
    }

    let session = state
        .db
        .app_sessions
        .create(user.id, state.config.session_ttl)
        .await?;
    state.db.app_users.touch_last_login(user.id).await?;

    // Reflect the freshly-bumped `last_login_at` in the response so callers
    // don't see a stale `null` when they just authenticated.
    let mut user_for_dto = user.clone();
    user_for_dto.last_login_at = Some(bson::DateTime::now());

    Ok(Json(AppLoginResponse {
        token: session.token,
        expires_at: session
            .expires_at
            .try_to_rfc3339_string()
            .unwrap_or_default(),
        user: AppUserDto::from_app_user(&user_for_dto),
        org: OrgDto::from_org_as_non_admin(&org),
        needs_password_change: user.needs_password_change,
    }))
}

/// `POST /app/auth/logout` — Bearer auth, allow-listed (still reachable when
/// `needs_password_change == true`). Deletes the caller's `app_sessions`
/// row. Other devices' sessions are unaffected.
pub async fn logout(
    State(state): State<AppState>,
    ctx: AppAuthContext,
) -> ApiResult<impl IntoResponse> {
    state
        .db
        .app_sessions
        .delete_by_token(&ctx.session_token)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /app/me` — Bearer auth, allow-listed. Returns the caller's identity
/// context (AppUser + Org + the forced-change flag).
pub async fn me(
    State(state): State<AppState>,
    ctx: AppAuthContext,
) -> ApiResult<Json<AppMeResponse>> {
    let user = state
        .db
        .app_users
        .find_by_id(ctx.app_user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let org = state
        .db
        .orgs
        .find_by_id(user.org_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    Ok(Json(AppMeResponse {
        needs_password_change: user.needs_password_change,
        user: AppUserDto::from_app_user(&user),
        org: OrgDto::from_org_as_non_admin(&org),
    }))
}

/// `POST /app/me/password` — Bearer auth, allow-listed (reachable while
/// `needs_password_change == true` so the user can clear the gate).
/// Verifies `current_password`, validates `new_password` length >= 8,
/// updates the hash, and clears `needs_password_change`. Sessions are
/// untouched.
pub async fn change_password(
    State(state): State<AppState>,
    ctx: AppAuthContext,
    Json(req): Json<AppPasswordChangeRequest>,
) -> ApiResult<impl IntoResponse> {
    let user = state
        .db
        .app_users
        .find_by_id(ctx.app_user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    let password_hash = user
        .password_hash
        .as_deref()
        .ok_or(ApiError::InvalidPassword)?;
    if !password::verify(&req.current_password, password_hash)? {
        return Err(ApiError::InvalidPassword);
    }

    if req.new_password.chars().count() < MIN_PASSWORD_LEN {
        return Err(ApiError::Validation(format!(
            "new_password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }

    let new_hash = password::hash(&req.new_password)?;
    state
        .db
        .app_users
        .mark_password_changed(user.id, &new_hash)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
