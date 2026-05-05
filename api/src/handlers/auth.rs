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
use crate::db::MembershipInsertError;
use crate::domain::{DashboardUser, Membership, Org, Role};
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
}

#[derive(Debug, Serialize)]
pub struct OrgDto {
    pub id: String,
    pub name: String,
    pub code: String,
    pub owner_id: String,
    pub timezone: String,
    pub checkin: OrgCheckinDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug_changed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrgCheckinDto {
    pub transfer_enabled: bool,
    pub location_tracking_enabled: bool,
}

impl OrgDto {
    pub fn from_org(org: &Org) -> Self {
        Self {
            id: org.id.to_hex(),
            name: org.name.clone(),
            code: org.code.clone(),
            owner_id: org.owner_id.to_hex(),
            timezone: org.timezone.clone(),
            checkin: OrgCheckinDto {
                transfer_enabled: org.checkin_transfer_enabled(),
                location_tracking_enabled: org.checkin_location_tracking_enabled(),
            },
            slug: org.slug.clone(),
            slug_changed_at: org
                .slug_changed_at
                .and_then(|d| d.try_to_rfc3339_string().ok()),
        }
    }
}

/// One entry in the user's `memberships` array. Pairs an `OrgDto` with the
/// caller's role in that Org.
#[derive(Debug, Serialize)]
pub struct MembershipDto {
    pub org: OrgDto,
    pub role: Role,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: UserDto,
    pub memberships: Vec<MembershipDto>,
    /// `null` when the user has no memberships (zero-Org state) or has logged
    /// in with no current Org selected.
    pub current_org: Option<OrgDto>,
    /// Role within `current_org`. `null` whenever `current_org` is `null`.
    pub role: Option<Role>,
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

            // Strict separation: existing identity must use /me/orgs instead.
            // Mirror the earlier semantics by short-circuiting on the email
            // unique-index check before doing any heavy work.
            if state
                .db
                .dashboard_users
                .find_by_email(&email)
                .await?
                .is_some()
            {
                return Err(ApiError::EmailTaken);
            }

            let password_hash = password::hash(&password)?;
            // Pre-allocate the user id so the Org can record it as `owner_id` at creation
            // time, avoiding a second write to the org doc.
            let user_id = ObjectId::new();
            let org = create_org_with_unique_code(&state, &org_name, user_id).await?;

            let user = match state
                .db
                .dashboard_users
                .create(user_id, &email, &password_hash)
                .await
            {
                Ok(u) => u,
                Err(err) => {
                    if let Err(cleanup_err) = state.db.orgs.delete_by_id(org.id).await {
                        tracing::error!(?cleanup_err, org_id = %org.id, "failed to roll back org after user creation error");
                    }
                    return Err(err);
                }
            };

            let membership = match state
                .db
                .dashboard_memberships
                .create(user.id, org.id, Role::Admin)
                .await
            {
                Ok(m) => m,
                Err(MembershipInsertError::Duplicate) => {
                    // Should be unreachable: brand-new user, brand-new org. If
                    // it ever fires, treat as internal and rewind the writes.
                    let _ = state.db.dashboard_users.delete_by_id(user.id).await;
                    let _ = state.db.orgs.delete_by_id(org.id).await;
                    return Err(ApiError::Internal);
                }
                Err(MembershipInsertError::Db(err)) => {
                    let _ = state.db.dashboard_users.delete_by_id(user.id).await;
                    let _ = state.db.orgs.delete_by_id(org.id).await;
                    return Err(ApiError::Db(err));
                }
            };

            issue_session(&state, &jar, user, vec![(membership, org.clone())], Some(org)).await
        }
        RegisterRequest::Join {
            email,
            password,
            org_code,
        } => {
            validate_email(&email)?;
            validate_password(&password)?;

            // Strict separation: existing identity must use /me/memberships instead.
            if state
                .db
                .dashboard_users
                .find_by_email(&email)
                .await?
                .is_some()
            {
                return Err(ApiError::EmailTaken);
            }

            let org = slug_auth::resolve_org_for_join(&state.db, &org_code).await?;

            // Cooldown enforcement: a marker for (org, lowercase(email)) blocks rejoin
            // until cooldown_until expires. Lookup is case-insensitive via lowercased key.
            let email_key = email.trim().to_ascii_lowercase();
            enforce_join_cooldown(&state, org.id, &email_key).await?;

            let password_hash = password::hash(&password)?;
            let user = state
                .db
                .dashboard_users
                .create(ObjectId::new(), &email, &password_hash)
                .await?;
            let membership = match state
                .db
                .dashboard_memberships
                .create(user.id, org.id, Role::Member)
                .await
            {
                Ok(m) => m,
                Err(MembershipInsertError::Duplicate) => {
                    // Brand-new identity colliding on (user_id, org_id) should be
                    // unreachable; treat as internal and clean up the user row.
                    let _ = state.db.dashboard_users.delete_by_id(user.id).await;
                    return Err(ApiError::Internal);
                }
                Err(MembershipInsertError::Db(err)) => {
                    let _ = state.db.dashboard_users.delete_by_id(user.id).await;
                    return Err(ApiError::Db(err));
                }
            };
            issue_session(&state, &jar, user, vec![(membership, org.clone())], Some(org)).await
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

    let memberships = state
        .db
        .dashboard_memberships
        .list_by_user(user.id)
        .await?;
    let pairs = load_membership_orgs(&state, memberships).await?;
    let current_org = pick_default_org(user.id, &pairs);

    issue_session(&state, &jar, user, pairs, current_org).await
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

/// Hydrate `(membership, org)` pairs in `joined_at` order. Memberships whose
/// orgs have vanished (a future Org delete cascade) are skipped silently.
pub(crate) async fn load_membership_orgs(
    state: &AppState,
    mut memberships: Vec<Membership>,
) -> ApiResult<Vec<(Membership, Org)>> {
    memberships.sort_by_key(|m| m.joined_at.timestamp_millis());
    let mut out = Vec::with_capacity(memberships.len());
    for m in memberships {
        if let Some(org) = state.db.orgs.find_by_id(m.org_id).await? {
            out.push((m, org));
        } else {
            tracing::warn!(membership_id = %m.id, org_id = %m.org_id, "membership references missing org; skipping");
        }
    }
    Ok(out)
}

/// Default-org rule: oldest owned Org first; otherwise oldest membership;
/// otherwise `None` (zero-Org state).
pub(crate) fn pick_default_org(user_id: ObjectId, pairs: &[(Membership, Org)]) -> Option<Org> {
    if let Some(owned) = pairs
        .iter()
        .filter(|(_, o)| o.owner_id == user_id)
        .min_by_key(|(_, o)| o.created_at.timestamp_millis())
    {
        return Some(owned.1.clone());
    }
    pairs
        .iter()
        .min_by_key(|(m, _)| m.joined_at.timestamp_millis())
        .map(|(_, o)| o.clone())
}

/// Cooldown gate: shared between `register mode=join` and `POST /me/memberships`.
pub(crate) async fn enforce_join_cooldown(
    state: &AppState,
    org_id: ObjectId,
    lowercased_email: &str,
) -> ApiResult<()> {
    if let Some(marker) = state
        .db
        .removed_memberships
        .find(org_id, lowercased_email)
        .await?
    {
        let now_ms = DateTime::now().timestamp_millis();
        if marker.cooldown_until.timestamp_millis() > now_ms {
            return Err(ApiError::EmailInCooldown);
        }
    }
    Ok(())
}

pub(crate) async fn issue_session(
    state: &AppState,
    jar: &CookieJar,
    user: DashboardUser,
    pairs: Vec<(Membership, Org)>,
    current_org: Option<Org>,
) -> ApiResult<axum::response::Response> {
    let current_org_id = current_org.as_ref().map(|o| o.id);
    let session = state
        .db
        .dashboard_sessions
        .create(user.id, current_org_id, state.config.session_ttl)
        .await?;
    let cookie = build_session_cookie(session.token.clone(), &state.config);
    let new_jar = jar.clone().add(cookie);

    let body = build_auth_response(user, pairs, current_org);
    Ok((new_jar, Json(body)).into_response())
}

pub(crate) fn build_auth_response(
    user: DashboardUser,
    pairs: Vec<(Membership, Org)>,
    current_org: Option<Org>,
) -> AuthResponse {
    let role = current_org
        .as_ref()
        .and_then(|co| pairs.iter().find(|(_, o)| o.id == co.id).map(|(m, _)| m.role));

    let memberships = pairs
        .into_iter()
        .map(|(m, o)| MembershipDto {
            org: OrgDto::from_org(&o),
            role: m.role,
        })
        .collect();

    AuthResponse {
        user: UserDto {
            id: user.id.to_hex(),
            email: user.email,
        },
        memberships,
        current_org: current_org.as_ref().map(OrgDto::from_org),
        role,
    }
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

pub(crate) fn build_session_cookie(token: String, config: &Config) -> Cookie<'static> {
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

pub(crate) fn validate_email(email: &str) -> ApiResult<()> {
    let trimmed = email.trim();
    if trimmed.is_empty() || !trimmed.contains('@') || trimmed.len() > 254 {
        return Err(ApiError::Validation("invalid email".into()));
    }
    Ok(())
}

pub(crate) fn validate_password(password: &str) -> ApiResult<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(ApiError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    Ok(())
}

pub(crate) fn validate_org_name(name: &str) -> ApiResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 {
        return Err(ApiError::Validation("org_name length must be 1..=120".into()));
    }
    Ok(())
}

/// Allocate an Org with a fresh `org_code`, retrying on the rare unique-index
/// collision. Used by both `register mode=create` and `POST /me/orgs`.
pub(crate) async fn create_org_unique(
    state: &AppState,
    name: &str,
    owner_id: ObjectId,
) -> ApiResult<Org> {
    create_org_with_unique_code(state, name, owner_id).await
}
