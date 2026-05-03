use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bson::DateTime;
use serde_json::json;

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("email already taken")]
    EmailTaken,
    #[error("invalid org code")]
    InvalidOrgCode,
    #[error("operation cannot target the org owner")]
    OwnerProtected,
    #[error("this email cannot rejoin this org until cooldown expires")]
    EmailInCooldown,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("validation failed: {0}")]
    Validation(String),

    #[error("invalid slug format")]
    InvalidSlugFormat,
    #[error("slug is reserved")]
    SlugReserved,
    #[error("slug already taken")]
    SlugTaken,
    #[error("slug change too soon")]
    SlugChangeTooSoon { retry_after: DateTime },

    /// Caller hit an org-scoped endpoint with no `current_org_id` selected.
    #[error("no active org selected")]
    NoActiveOrg,
    /// Target user has no membership in the relevant Org. Surfaced when the
    /// caller tries to switch into / transfer ownership to an Org they
    /// (or the target) don't belong to.
    #[error("not a member of this org")]
    NotAMember,
    /// New membership would collide with an existing `(user_id, org_id)` row.
    #[error("user is already a member of this org")]
    AlreadyMember,
    /// Re-auth password didn't match the caller's stored hash.
    #[error("invalid password")]
    InvalidPassword,
    /// Owner-transfer target is not currently an admin of the Org, or has no
    /// membership at all.
    #[error("invalid target user")]
    InvalidTarget,
    /// Owner tried to transfer the Org to themselves.
    #[error("new owner must differ from the current owner")]
    SameOwner,

    // --- AppUser surface ---
    /// `(org_id, username_lower)` collides with an existing AppUser.
    #[error("username already taken")]
    UsernameTaken,
    /// AppUser username failed the `^[a-zA-Z0-9_.-]{2,32}$` shape check.
    #[error("invalid username format")]
    InvalidUsernameFormat,
    /// AppUser is authenticated but `needs_password_change` is set; the
    /// route is gated until they finish the forced change.
    #[error("password change required before this action")]
    NeedsPasswordChange,

    #[error("password hashing failed")]
    Password,
    #[error("database error")]
    Db(#[from] mongodb::error::Error),
    #[error("bson serialization error")]
    BsonSer(#[from] bson::ser::Error),
    #[error("bson deserialization error")]
    BsonDe(#[from] bson::de::Error),
    #[error("internal error")]
    Internal,
}

impl ApiError {
    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            ApiError::EmailTaken => (StatusCode::CONFLICT, "EMAIL_TAKEN"),
            ApiError::InvalidOrgCode => (StatusCode::BAD_REQUEST, "INVALID_ORG_CODE"),
            ApiError::OwnerProtected => (StatusCode::FORBIDDEN, "OWNER_PROTECTED"),
            ApiError::EmailInCooldown => (StatusCode::CONFLICT, "EMAIL_IN_COOLDOWN"),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            ApiError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS"),
            ApiError::Validation(_) => (StatusCode::BAD_REQUEST, "VALIDATION"),
            ApiError::InvalidSlugFormat => (StatusCode::BAD_REQUEST, "INVALID_SLUG_FORMAT"),
            ApiError::SlugReserved => (StatusCode::BAD_REQUEST, "SLUG_RESERVED"),
            ApiError::SlugTaken => (StatusCode::CONFLICT, "SLUG_TAKEN"),
            ApiError::SlugChangeTooSoon { .. } => {
                (StatusCode::TOO_MANY_REQUESTS, "SLUG_CHANGE_TOO_SOON")
            }
            ApiError::NoActiveOrg => (StatusCode::FORBIDDEN, "NO_ACTIVE_ORG"),
            // `NOT_A_MEMBER` is surfaced by `POST /me/current-org`, which is the
            // one place a caller has a legitimate reason to learn that an Org
            // they tried to switch into isn't in their membership set. Other
            // cross-Org probes still flatten to `NOT_FOUND`.
            ApiError::NotAMember => (StatusCode::NOT_FOUND, "NOT_A_MEMBER"),
            ApiError::AlreadyMember => (StatusCode::CONFLICT, "ALREADY_MEMBER"),
            ApiError::InvalidPassword => (StatusCode::UNAUTHORIZED, "INVALID_PASSWORD"),
            ApiError::InvalidTarget => (StatusCode::BAD_REQUEST, "INVALID_TARGET"),
            ApiError::SameOwner => (StatusCode::BAD_REQUEST, "SAME_OWNER"),
            ApiError::UsernameTaken => (StatusCode::CONFLICT, "USERNAME_TAKEN"),
            ApiError::InvalidUsernameFormat => {
                (StatusCode::BAD_REQUEST, "INVALID_USERNAME_FORMAT")
            }
            ApiError::NeedsPasswordChange => {
                (StatusCode::LOCKED, "NEEDS_PASSWORD_CHANGE")
            }
            ApiError::Password
            | ApiError::Db(_)
            | ApiError::BsonSer(_)
            | ApiError::BsonDe(_)
            | ApiError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        if status.is_server_error() {
            tracing::error!(error = ?self, "server error");
        }
        let message = match &self {
            ApiError::Validation(msg) => msg.clone(),
            other => other.to_string(),
        };
        let body = match &self {
            ApiError::SlugChangeTooSoon { retry_after } => Json(json!({
                "error": {
                    "code": code,
                    "message": message,
                    "retry_after": retry_after.try_to_rfc3339_string().ok(),
                }
            })),
            _ => Json(json!({ "error": { "code": code, "message": message } })),
        };
        (status, body).into_response()
    }
}
