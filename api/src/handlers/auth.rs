use std::time::Duration;

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
use crate::auth::{api_token, org_code, password, session_token, slug as slug_auth};
use crate::config::Config;
use crate::db::MembershipInsertError;
use crate::domain::{DashboardUser, EncryptMode, Membership, Org, Role};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const ORG_CODE_RETRIES: usize = 3;
const MIN_PASSWORD_LEN: usize = 8;
/// How long a `POST /auth/forgot-password` token stays valid.
const RESET_TOKEN_TTL: Duration = Duration::from_secs(60 * 60);
/// Minimum gap between two token issuances for the same user — see the
/// `dashboard-auth` spec's "Password-reset requests for the same user are
/// rate-limited" requirement.
const RESET_REQUEST_COOLDOWN: Duration = Duration::from_secs(60);

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
    pub auth_source: crate::domain::OrgAuthSource,
    /// External-auth configuration WITHOUT the connection password. Present only
    /// when the Org has an `external_auth` config stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_auth: Option<ExternalAuthSummaryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug_changed_at: Option<String>,
}

/// Password-free view of `Org.settings.external_auth`. The connection password
/// is never serialized; callers learn only whether one is set.
#[derive(Debug, Serialize)]
pub struct ExternalAuthSummaryDto {
    pub driver: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub query: String,
    pub key_col: String,
    pub display_col: String,
    pub password_set: bool,
    /// Non-secret connection settings, surfaced so admins can see/edit them.
    pub encrypt: EncryptMode,
    pub trust_server_certificate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_query: Option<String>,
}

impl ExternalAuthSummaryDto {
    fn from_config(cfg: &crate::domain::ExternalAuthConfig) -> Self {
        Self {
            driver: cfg.driver.clone(),
            host: cfg.host.clone(),
            port: cfg.port,
            database: cfg.database.clone(),
            username: cfg.username.clone(),
            query: cfg.query.clone(),
            key_col: cfg.key_col.clone(),
            display_col: cfg.display_col.clone(),
            password_set: !cfg.password_encrypted.is_empty(),
            encrypt: cfg.encrypt,
            trust_server_certificate: cfg.trust_server_certificate,
            list_query: cfg.list_query.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OrgCheckinDto {
    pub transfer_enabled: bool,
    pub location_tracking_enabled: bool,
}

impl OrgDto {
    /// Caller is a confirmed dashboard `admin` of this Org — includes
    /// `external_auth` (password-free) in the response.
    pub fn from_org_as_admin(org: &Org) -> Self {
        Self::build(org, true)
    }

    /// Caller is anything else: a dashboard `member`, an AppUser session, or
    /// any context where the caller's role is not a confirmed `admin`.
    /// `external_auth` is entirely absent from the response, not null/empty
    /// — see `openspec/specs/external-db-auth/spec.md`'s "External-auth
    /// configuration is only visible to dashboard admins" requirement.
    pub fn from_org_as_non_admin(org: &Org) -> Self {
        Self::build(org, false)
    }

    fn build(org: &Org, include_external_auth: bool) -> Self {
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
            auth_source: org.auth_source(),
            external_auth: if include_external_auth {
                org.external_auth()
                    .as_ref()
                    .map(ExternalAuthSummaryDto::from_config)
            } else {
                None
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

            issue_session(
                &state,
                &jar,
                user,
                vec![(membership, org.clone())],
                Some(org),
            )
            .await
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

            // Resolve the Org and run cooldown BEFORE creating the user, so
            // a cooldown failure leaves no orphan dashboard_user rows.
            let org = slug_auth::resolve_org_for_join(&state.db, &org_code).await?;
            let email_key = email.trim().to_ascii_lowercase();
            enforce_join_cooldown(&state, org.id, &email_key).await?;

            // Behavior change vs prior: register mode=join no longer creates
            // a `dashboard_memberships` row directly. It creates the user
            // identity + a pending `join_requests` row. Session is issued
            // with `current_org_id=null` (zero-Org state) until an admin
            // approves the request. See `org-join-requests` capability.
            let password_hash = password::hash(&password)?;
            let user = state
                .db
                .dashboard_users
                .create(ObjectId::new(), &email, &password_hash)
                .await?;

            match state
                .db
                .join_requests
                .insert_pending(user.id, org.id, None)
                .await
            {
                Ok(_) => {}
                Err(crate::db::JoinRequestInsertError::Duplicate) => {
                    // Brand-new identity should never collide on the partial
                    // unique index, but if it somehow does, roll back.
                    let _ = state.db.dashboard_users.delete_by_id(user.id).await;
                    return Err(ApiError::JoinRequestPending);
                }
                Err(crate::db::JoinRequestInsertError::Db(err)) => {
                    let _ = state.db.dashboard_users.delete_by_id(user.id).await;
                    return Err(ApiError::Db(err));
                }
            }

            // Zero-org session — user must wait for admin approval. They
            // can poll `/me/join-requests` to see status and cancel if they
            // change their mind.
            issue_session(&state, &jar, user, Vec::new(), None).await
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

    // Locked accounts are rejected without checking the password, both to
    // avoid the wasted bcrypt work and — more importantly — so repeated
    // attempts against a locked account never extend the lock window.
    if user
        .locked_until
        .is_some_and(|until| until > DateTime::now())
    {
        return Err(ApiError::InvalidCredentials);
    }

    let ok = password::verify(&req.password, &user.password_hash)?;
    if !ok {
        let attempts = state
            .db
            .dashboard_users
            .record_failed_attempt(user.id)
            .await?;
        if attempts >= state.config.login_lockout_threshold {
            let until = DateTime::from_millis(
                DateTime::now().timestamp_millis()
                    + state.config.login_lockout_duration.as_millis() as i64,
            );
            state
                .db
                .dashboard_users
                .set_locked_until(user.id, until)
                .await?;
        }
        return Err(ApiError::InvalidCredentials);
    }
    state.db.dashboard_users.reset_lockout(user.id).await?;

    let memberships = state.db.dashboard_memberships.list_by_user(user.id).await?;
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

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

/// `POST /auth/forgot-password` — public. Always `204`, regardless of
/// whether `email` matches an account, whether the requester is within the
/// cooldown, or whether the send itself succeeds — see the `dashboard-auth`
/// spec's "request a password reset link without revealing whether the
/// email exists" requirement. Every early-return in this handler funnels to
/// the same response; do not add a branch that surfaces different status
/// codes for different cases.
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(req): Json<ForgotPasswordRequest>,
) -> ApiResult<StatusCode> {
    if let Some(user) = state
        .db
        .dashboard_users
        .find_by_email(req.email.trim())
        .await?
    {
        let within_cooldown = state
            .db
            .password_reset_tokens
            .find_latest_for_user(user.id)
            .await?
            .is_some_and(|t| {
                let elapsed_ms =
                    DateTime::now().timestamp_millis() - t.created_at.timestamp_millis();
                elapsed_ms < RESET_REQUEST_COOLDOWN.as_millis() as i64
            });

        if !within_cooldown {
            let raw_token = session_token::generate();
            let token_hash = api_token::hash_token(&raw_token);
            state
                .db
                .password_reset_tokens
                .insert(user.id, &token_hash, RESET_TOKEN_TTL)
                .await?;

            let link = format!(
                "{}/reset-password?token={}",
                state.config.admin_web_base_url, raw_token
            );
            if let Err(err) = state
                .email
                .send(
                    &user.email,
                    "重設班到密碼",
                    &render_reset_password_email(&link),
                )
                .await
            {
                tracing::warn!(?err, user_id = %user.id, "forgot-password email send failed");
            }
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

fn render_reset_password_email(link: &str) -> String {
    format!(
        "<p>你好，</p>\
         <p>我們收到重設班到密碼的請求。點擊下方連結設定新密碼，此連結 60 分鐘內有效：</p>\
         <p><a href=\"{link}\">{link}</a></p>\
         <p>如果這不是你本人的操作，請忽略這封信，你的密碼不會被變更。</p>"
    )
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

/// `POST /auth/reset-password` — public. Unlike `forgot_password`, the
/// caller here already possesses a value that can only have come from the
/// email (the token) — there is no anonymous-guessing threat model to
/// protect against, so a specific `INVALID_RESET_TOKEN` error is fine.
pub async fn reset_password(
    State(state): State<AppState>,
    Json(req): Json<ResetPasswordRequest>,
) -> ApiResult<StatusCode> {
    let token_hash = api_token::hash_token(&req.token);
    let record = state
        .db
        .password_reset_tokens
        .find_by_hash(&token_hash)
        .await?
        .filter(|t| {
            t.used_at.is_none()
                && t.expires_at.timestamp_millis() > DateTime::now().timestamp_millis()
        })
        .ok_or(ApiError::InvalidResetToken)?;

    validate_password(&req.new_password)?;

    let new_hash = password::hash(&req.new_password)?;
    state
        .db
        .dashboard_users
        .update_password_hash(record.user_id, &new_hash)
        .await?;
    state.db.password_reset_tokens.mark_used(record.id).await?;
    state
        .db
        .dashboard_sessions
        .delete_all_by_user_id(record.user_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
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
    let role = current_org.as_ref().and_then(|co| {
        pairs
            .iter()
            .find(|(_, o)| o.id == co.id)
            .map(|(m, _)| m.role)
    });

    let memberships = pairs
        .into_iter()
        .map(|(m, o)| {
            let org = if matches!(m.role, Role::Admin) {
                OrgDto::from_org_as_admin(&o)
            } else {
                OrgDto::from_org_as_non_admin(&o)
            };
            MembershipDto { org, role: m.role }
        })
        .collect();

    AuthResponse {
        user: UserDto {
            id: user.id.to_hex(),
            email: user.email,
        },
        memberships,
        current_org: current_org.as_ref().map(|o| {
            if matches!(role, Some(Role::Admin)) {
                OrgDto::from_org_as_admin(o)
            } else {
                OrgDto::from_org_as_non_admin(o)
            }
        }),
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
        return Err(ApiError::Validation(
            "org_name length must be 1..=120".into(),
        ));
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
