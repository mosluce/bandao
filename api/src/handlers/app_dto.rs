//! Shared DTO shapes for the `/app/*` (mobile-facing) and `/app-users/*`
//! (admin-facing) handlers. Defined once and reused so the wire shape is
//! identical regardless of which handler emits it.

use serde::{Deserialize, Serialize};

use crate::domain::{AppUser, AppUserStatus};
use crate::handlers::auth::OrgDto;

#[derive(Debug, Serialize)]
pub struct AppUserDto {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub status: AppUserStatus,
    pub needs_password_change: bool,
    /// RFC3339-encoded; `null` when the AppUser has never logged in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_login_at: Option<String>,
    pub created_at: String,
}

impl AppUserDto {
    pub fn from_app_user(u: &AppUser) -> Self {
        Self {
            id: u.id.to_hex(),
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            status: u.status,
            needs_password_change: u.needs_password_change,
            last_login_at: u
                .last_login_at
                .and_then(|d| d.try_to_rfc3339_string().ok()),
            created_at: u
                .created_at
                .try_to_rfc3339_string()
                .unwrap_or_default(),
        }
    }
}

// --- Mobile-facing (`/app/*`) request / response shapes ---

#[derive(Debug, Deserialize)]
pub struct AppLoginRequest {
    pub org_code: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AppLoginResponse {
    pub token: String,
    pub expires_at: String,
    pub user: AppUserDto,
    pub org: OrgDto,
    pub needs_password_change: bool,
}

#[derive(Debug, Serialize)]
pub struct AppMeResponse {
    pub user: AppUserDto,
    pub org: OrgDto,
    pub needs_password_change: bool,
}

#[derive(Debug, Deserialize)]
pub struct AppPasswordChangeRequest {
    pub current_password: String,
    pub new_password: String,
}

// --- Admin-facing (`/app-users/*`) request / response shapes ---

#[derive(Debug, Deserialize)]
pub struct CreateAppUserRequest {
    pub username: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateAppUserRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub status: Option<AppUserStatus>,
}

#[derive(Debug, Serialize)]
pub struct CreateAppUserResponse {
    pub user: AppUserDto,
    pub initial_password: String,
}
