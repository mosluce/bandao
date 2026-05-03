use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bson::oid::ObjectId;
use serde::Serialize;

use crate::auth::extractor::RequireAdmin;
use crate::auth::{app_password, password};
use crate::db::AppUserInsertError;
use crate::domain::AppUserStatus;
use crate::error::{ApiError, ApiResult};
use crate::handlers::app_dto::{
    AppUserDto, CreateAppUserRequest, CreateAppUserResponse, UpdateAppUserRequest,
};
use crate::state::AppState;

/// Username shape: `^[a-zA-Z0-9_.-]{2,32}$`. Looser than slug / org_code
/// because usernames are human-controlled and human-displayed (people use
/// `firstname.lastname` etc.). No `@` so it stays visually distinct from email.
const USERNAME_MIN: usize = 2;
const USERNAME_MAX: usize = 32;
const DISPLAY_NAME_MIN: usize = 1;
const DISPLAY_NAME_MAX: usize = 60;

#[derive(Debug, Serialize)]
pub struct PasswordResetResponse {
    pub user: AppUserDto,
    pub initial_password: String,
}

/// `GET /app-users` — admin-only, scoped to `current_org`. Returns the
/// AppUsers in the caller's current Org. Members get `FORBIDDEN`; sessions
/// without `current_org_id` get `NO_ACTIVE_ORG` via `RequireAdmin`.
pub async fn list(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
) -> ApiResult<Json<Vec<AppUserDto>>> {
    let users = state.db.app_users.list_by_org(active.org_id).await?;
    let mut out: Vec<AppUserDto> = users
        .iter()
        .map(AppUserDto::from_app_user)
        .collect();
    // Newest-first reads naturally on the admin list.
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(out))
}

/// `POST /app-users` — admin-only. Validates input shape, generates a fresh
/// initial password, stores its bcrypt hash, and returns the cleartext
/// password exactly once alongside the new AppUser DTO.
pub async fn create(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Json(req): Json<CreateAppUserRequest>,
) -> ApiResult<(StatusCode, Json<CreateAppUserResponse>)> {
    let username_raw = req.username.trim();
    validate_username(username_raw)?;
    let display_name = req.display_name.trim();
    validate_display_name(display_name)?;

    let username_lower = username_raw.to_ascii_lowercase();

    let initial_password = app_password::generate_initial();
    let password_hash = password::hash(&initial_password)?;

    let user = match state
        .db
        .app_users
        .create(
            active.org_id,
            username_raw,
            &username_lower,
            display_name,
            &password_hash,
            active.ctx.user_id,
        )
        .await
    {
        Ok(u) => u,
        Err(AppUserInsertError::Duplicate) => return Err(ApiError::UsernameTaken),
        Err(AppUserInsertError::Db(err)) => return Err(ApiError::Db(err)),
    };

    Ok((
        StatusCode::CREATED,
        Json(CreateAppUserResponse {
            user: AppUserDto::from_app_user(&user),
            initial_password,
        }),
    ))
}

/// `PATCH /app-users/:id` — admin-only, scoped to `current_org`. Accepts
/// partial updates of `display_name?` and/or `status?`. Cross-Org targets
/// surface as `NOT_FOUND`. When `status` transitions to `disabled`, every
/// `app_sessions` row for the AppUser is deleted.
pub async fn update(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id): Path<String>,
    Json(req): Json<UpdateAppUserRequest>,
) -> ApiResult<Json<AppUserDto>> {
    let target_id = ObjectId::parse_str(&id).map_err(|_| ApiError::NotFound)?;
    let mut user = load_in_org(&state, active.org_id, target_id).await?;

    if req.display_name.is_none() && req.status.is_none() {
        // No-op patch: just echo the current state. Keeps the wire shape
        // stable for clients that send empty bodies probing for permission.
        return Ok(Json(AppUserDto::from_app_user(&user)));
    }

    if let Some(name) = req.display_name.as_deref() {
        let trimmed = name.trim();
        validate_display_name(trimmed)?;
        user = state.db.app_users.update_profile(user.id, trimmed).await?;
    }

    if let Some(new_status) = req.status {
        if user.status != new_status {
            user = state.db.app_users.update_status(user.id, new_status).await?;
            if matches!(new_status, AppUserStatus::Disabled) {
                // Force-kick: drop every session for this AppUser. Re-enable
                // does NOT auto-issue a session — they have to log back in.
                state.db.app_sessions.delete_by_app_user(user.id).await?;
            }
        }
    }

    Ok(Json(AppUserDto::from_app_user(&user)))
}

/// `POST /app-users/:id/password-reset` — admin-only, scoped to
/// `current_org`. Generates a fresh initial password, replaces the hash,
/// forces `needs_password_change = true`, deletes every `app_sessions` row
/// for the AppUser, and returns the cleartext password exactly once.
pub async fn password_reset(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id): Path<String>,
) -> ApiResult<Json<PasswordResetResponse>> {
    let target_id = ObjectId::parse_str(&id).map_err(|_| ApiError::NotFound)?;
    let user = load_in_org(&state, active.org_id, target_id).await?;

    let initial_password = app_password::generate_initial();
    let password_hash = password::hash(&initial_password)?;

    let updated = state
        .db
        .app_users
        .update_password(user.id, &password_hash)
        .await?;
    state.db.app_sessions.delete_by_app_user(user.id).await?;

    Ok(Json(PasswordResetResponse {
        user: AppUserDto::from_app_user(&updated),
        initial_password,
    }))
}

/// Cross-Org safety check rolled into a single helper. Loads the AppUser
/// only when its `org_id` matches `current_org`; everything else (unknown
/// id, valid id but different Org) collapses to `NOT_FOUND`.
async fn load_in_org(
    state: &AppState,
    current_org_id: ObjectId,
    target_id: ObjectId,
) -> ApiResult<crate::domain::AppUser> {
    let user = state
        .db
        .app_users
        .find_by_id(target_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if user.org_id != current_org_id {
        return Err(ApiError::NotFound);
    }
    Ok(user)
}

fn validate_username(value: &str) -> ApiResult<()> {
    let len = value.chars().count();
    if !(USERNAME_MIN..=USERNAME_MAX).contains(&len) {
        return Err(ApiError::InvalidUsernameFormat);
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return Err(ApiError::InvalidUsernameFormat);
    }
    Ok(())
}

fn validate_display_name(value: &str) -> ApiResult<()> {
    let len = value.chars().count();
    if !(DISPLAY_NAME_MIN..=DISPLAY_NAME_MAX).contains(&len) {
        return Err(ApiError::Validation(format!(
            "display_name length must be {DISPLAY_NAME_MIN}..={DISPLAY_NAME_MAX}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_username_accepts_typical_shapes() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("alice.chen").is_ok());
        assert!(validate_username("alice-chen").is_ok());
        assert!(validate_username("alice_chen").is_ok());
        assert!(validate_username("a1").is_ok());
        assert!(validate_username(&"a".repeat(32)).is_ok());
    }

    #[test]
    fn validate_username_rejects_bad_shapes() {
        assert!(matches!(
            validate_username("a"),
            Err(ApiError::InvalidUsernameFormat)
        ));
        assert!(matches!(
            validate_username(&"a".repeat(33)),
            Err(ApiError::InvalidUsernameFormat)
        ));
        assert!(matches!(
            validate_username("alice@example"),
            Err(ApiError::InvalidUsernameFormat)
        ));
        assert!(matches!(
            validate_username("hi there"),
            Err(ApiError::InvalidUsernameFormat)
        ));
        assert!(matches!(validate_username(""), Err(ApiError::InvalidUsernameFormat)));
    }

    #[test]
    fn validate_display_name_bounds() {
        assert!(validate_display_name("Alice").is_ok());
        assert!(validate_display_name(&"x".repeat(60)).is_ok());
        assert!(validate_display_name("").is_err());
        assert!(validate_display_name(&"x".repeat(61)).is_err());
    }
}
