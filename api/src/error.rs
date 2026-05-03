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
