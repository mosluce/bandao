use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bson::DateTime;
use serde_json::json;

use crate::domain::{AppUserCheckinStatus, CheckinEventType};

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

    // --- Checkin-events surface ---
    /// `(prior_status, attempted_event)` is not in the legal-transition table.
    /// Body includes both fields so clients can render a useful message.
    #[error("invalid checkin transition")]
    InvalidTransition {
        from: AppUserCheckinStatus,
        attempted: CheckinEventType,
    },
    /// Org-level toggle is off; transfer events are blocked. clock_in /
    /// clock_out are unaffected.
    #[error("transfer events are disabled for this org")]
    TransferDisabled,
    /// New event's `occurred_at_client` is `<=` the AppUser's most recent
    /// stored event. Strict less-than-or-equal: same client time also fails.
    #[error("event is out of order with respect to the latest stored event")]
    OutOfOrder,
    /// Cannot flip `transfer_enabled` while AppUsers are on shift. Body
    /// carries the count so admin-web can render "目前在班 N 人，需先全部下班".
    #[error("cannot change setting while AppUsers are on shift")]
    StateLocked { on_duty_count: u32 },
    /// Force-checkout target is already off-duty.
    #[error("AppUser is not currently on shift")]
    NotOnDuty,
    /// `Org.timezone` write failed IANA validation.
    #[error("invalid timezone identifier")]
    InvalidTimezone,
    /// AppUser submitted a location ping batch but the org has
    /// `location_tracking_enabled = false`. Whole batch rejected.
    #[error("location tracking is disabled for this org")]
    LocationTrackingDisabled,
    /// Export endpoint's `from` / `to` query violates one of:
    /// both required, `to >= from`, span ≤ 90 days, `from ≥ now - 90 days`.
    #[error("invalid time range")]
    InvalidRange,
    /// Location ping batch is empty or exceeds the per-batch cap (100).
    #[error("invalid batch size")]
    InvalidBatch,
    /// User attempted to submit a join request while one is already pending
    /// for the same `(user_id, org_id)`. Triggers off the partial unique
    /// index on `join_requests`.
    #[error("a join request is already pending for this org")]
    JoinRequestPending,
    /// Operation requires the target row to be in a specific state and it
    /// isn't (e.g. trying to cancel an already-decided join request).
    #[error("operation not valid in current state")]
    InvalidState,

    /// External-database authentication could not be completed for a reason
    /// other than bad credentials — connection failure, query error, missing or
    /// malformed config, or an unsupported driver. Distinct from
    /// `InvalidCredentials` so the caller can tell a system problem apart from a
    /// mistyped password.
    #[error("external authentication is unavailable")]
    ExternalAuthUnavailable,
    /// An internal-only AppUser mutation (create / password-reset) was attempted
    /// while the Org's auth source is `external_db`, where credentials are owned
    /// by the external database rather than by this system.
    #[error("operation not available while the org uses external authentication")]
    ExternalAuthMode,

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
            ApiError::ExternalAuthUnavailable => {
                (StatusCode::SERVICE_UNAVAILABLE, "EXTERNAL_AUTH_UNAVAILABLE")
            }
            ApiError::ExternalAuthMode => (StatusCode::CONFLICT, "EXTERNAL_AUTH_MODE"),
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
            ApiError::InvalidUsernameFormat => (StatusCode::BAD_REQUEST, "INVALID_USERNAME_FORMAT"),
            ApiError::NeedsPasswordChange => (StatusCode::LOCKED, "NEEDS_PASSWORD_CHANGE"),
            ApiError::InvalidTransition { .. } => {
                (StatusCode::UNPROCESSABLE_ENTITY, "INVALID_TRANSITION")
            }
            ApiError::TransferDisabled => (StatusCode::FORBIDDEN, "TRANSFER_DISABLED"),
            ApiError::OutOfOrder => (StatusCode::CONFLICT, "OUT_OF_ORDER"),
            ApiError::StateLocked { .. } => (StatusCode::CONFLICT, "STATE_LOCKED"),
            ApiError::NotOnDuty => (StatusCode::CONFLICT, "NOT_ON_DUTY"),
            ApiError::InvalidTimezone => (StatusCode::BAD_REQUEST, "INVALID_TIMEZONE"),
            ApiError::LocationTrackingDisabled => {
                (StatusCode::FORBIDDEN, "LOCATION_TRACKING_DISABLED")
            }
            ApiError::InvalidRange => (StatusCode::BAD_REQUEST, "INVALID_RANGE"),
            ApiError::InvalidBatch => (StatusCode::BAD_REQUEST, "INVALID_BATCH"),
            ApiError::JoinRequestPending => (StatusCode::CONFLICT, "JOIN_REQUEST_PENDING"),
            ApiError::InvalidState => (StatusCode::BAD_REQUEST, "INVALID_STATE"),
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
            ApiError::InvalidTransition { from, attempted } => Json(json!({
                "error": {
                    "code": code,
                    "message": message,
                    "from": from,
                    "attempted": attempted,
                }
            })),
            ApiError::StateLocked { on_duty_count } => Json(json!({
                "error": {
                    "code": code,
                    "message": message,
                    "on_duty_count": on_duty_count,
                }
            })),
            _ => Json(json!({ "error": { "code": code, "message": message } })),
        };
        (status, body).into_response()
    }
}
