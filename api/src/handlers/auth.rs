use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use bson::DateTime;
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use crate::auth::extractor::{AuthContext, SESSION_COOKIE, build_clearing_cookie};
use crate::auth::{org_code, password, slug as slug_auth};
use crate::config::Config;
use crate::domain::{DashboardUser, Org, Role};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const ORG_CODE_RETRIES: usize = 3;
const MIN_PASSWORD_LEN: usize = 8;

#[derive(Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum RegisterRequest {
    Create {
        email: String,
        password: String,
        org_name: String,
    },
    Join {
        email: String,
        password: String,
        org_code: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Serialize)]
pub struct OrgDto {
    pub id: String,
    pub name: String,
    pub code: String,
    pub owner_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug_changed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: UserDto,
    pub org: OrgDto,
    pub role: Role,
}

pub async fn register(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<RegisterRequest>,
) -> ApiResult<impl IntoResponse> {
    match req {
        RegisterRequest::Create {
            email,
            password,
            org_name,
        } => {
            validate_email(&email)?;
            validate_password(&password)?;
            validate_org_name(&org_name)?;

            let password_hash = password::hash(&password)?;
            // Pre-allocate the user id so the Org can record it as `owner_id` at creation
            // time, avoiding a second write to the org doc.
            let user_id = ObjectId::new();
            let org = create_org_with_unique_code(&state, &org_name, user_id).await?;
            let user = match state
                .db
                .dashboard_users
                .create(user_id, org.id, &email, &password_hash, Role::Admin)
                .await
            {
                Ok(u) => u,
                Err(err) => {
                    // Roll back the org so the email can retry without leaking orphan orgs.
                    if let Err(cleanup_err) = state.db.orgs.delete_by_id(org.id).await {
                        tracing::error!(?cleanup_err, org_id = %org.id, "failed to roll back org after user creation error");
                    }
                    return Err(err);
                }
            };
            issue_session(&state, &jar, user, org).await
        }
        RegisterRequest::Join {
            email,
            password,
            org_code,
        } => {
            validate_email(&email)?;
            validate_password(&password)?;

            let org = slug_auth::resolve_org_for_join(&state.db, &org_code).await?;

            // Cooldown enforcement: a marker for (org, lowercase(email)) blocks rejoin
            // until cooldown_until expires. Lookup is case-insensitive via lowercased key.
            let email_key = email.trim().to_ascii_lowercase();
            if let Some(marker) = state
                .db
                .removed_memberships
                .find(org.id, &email_key)
                .await?
            {
                let now_ms = DateTime::now().timestamp_millis();
                if marker.cooldown_until.timestamp_millis() > now_ms {
                    return Err(ApiError::EmailInCooldown);
                }
            }

            let password_hash = password::hash(&password)?;
            let user = state
                .db
                .dashboard_users
                .create(ObjectId::new(), org.id, &email, &password_hash, Role::Member)
                .await?;
            issue_session(&state, &jar, user, org).await
        }
    }
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> ApiResult<impl IntoResponse> {
    let user = state
        .db
        .dashboard_users
        .find_by_email(&req.email)
        .await?
        .ok_or(ApiError::InvalidCredentials)?;
    let ok = password::verify(&req.password, &user.password_hash)?;
    if !ok {
        return Err(ApiError::InvalidCredentials);
    }
    let org = state
        .db
        .orgs
        .find_by_id(user.org_id)
        .await?
        .ok_or(ApiError::InvalidCredentials)?;
    issue_session(&state, &jar, user, org).await
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    ctx: AuthContext,
) -> ApiResult<impl IntoResponse> {
    state
        .db
        .dashboard_sessions
        .delete_by_token(&ctx.session_token)
        .await?;
    let cleared = jar.remove(build_clearing_cookie());
    Ok((StatusCode::NO_CONTENT, cleared))
}

async fn issue_session(
    state: &AppState,
    jar: &CookieJar,
    user: DashboardUser,
    org: Org,
) -> ApiResult<axum::response::Response> {
    let session = state
        .db
        .dashboard_sessions
        .create(user.id, org.id, state.config.session_ttl)
        .await?;
    let cookie = build_session_cookie(session.token.clone(), &state.config);
    let new_jar = jar.clone().add(cookie);

    let body = AuthResponse {
        user: UserDto {
            id: user.id.to_hex(),
            email: user.email,
            role: user.role,
        },
        org: OrgDto {
            id: org.id.to_hex(),
            name: org.name,
            code: org.code,
            owner_id: org.owner_id.to_hex(),
            slug: org.slug,
            slug_changed_at: org
                .slug_changed_at
                .and_then(|d| d.try_to_rfc3339_string().ok()),
        },
        role: user.role,
    };
    Ok((new_jar, Json(body)).into_response())
}

async fn create_org_with_unique_code(
    state: &AppState,
    name: &str,
    owner_id: ObjectId,
) -> ApiResult<Org> {
    use mongodb::error::{ErrorKind, WriteFailure};
    const DUPLICATE_KEY: i32 = 11000;

    for attempt in 0..ORG_CODE_RETRIES {
        let code = org_code::generate();
        match state.db.orgs.create(name, &code, owner_id).await {
            Ok(org) => return Ok(org),
            Err(ApiError::Db(err)) => {
                let is_dup = matches!(
                    err.kind.as_ref(),
                    ErrorKind::Write(WriteFailure::WriteError(we)) if we.code == DUPLICATE_KEY
                );
                if is_dup && attempt + 1 < ORG_CODE_RETRIES {
                    tracing::warn!(?code, attempt, "org code collision; retrying");
                    continue;
                }
                return Err(ApiError::Db(err));
            }
            Err(other) => return Err(other),
        }
    }
    Err(ApiError::Internal)
}

fn build_session_cookie(token: String, config: &Config) -> Cookie<'static> {
    let max_age = ::time::Duration::seconds(config.session_ttl.as_secs() as i64);
    let mut builder = Cookie::build((SESSION_COOKIE, token))
        .path("/")
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(SameSite::Lax)
        .max_age(max_age);
    if let Some(domain) = config.cookie_domain.clone() {
        builder = builder.domain(domain);
    }
    builder.build()
}

fn validate_email(email: &str) -> ApiResult<()> {
    let trimmed = email.trim();
    if trimmed.is_empty() || !trimmed.contains('@') || trimmed.len() > 254 {
        return Err(ApiError::Validation("invalid email".into()));
    }
    Ok(())
}

fn validate_password(password: &str) -> ApiResult<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(ApiError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    Ok(())
}

fn validate_org_name(name: &str) -> ApiResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 {
        return Err(ApiError::Validation("org_name length must be 1..=120".into()));
    }
    Ok(())
}
