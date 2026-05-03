use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use bson::DateTime;
use serde::{Deserialize, Serialize};

use crate::auth::extractor::RequireAdmin;
use crate::auth::slug::{GRACE_TTL, SlugValidationError};
use crate::auth::{org_code, slug as slug_auth};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const ORG_CODE_RETRIES: usize = 3;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Debug, Serialize)]
pub struct RotateCodeResponse {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct SetSlugRequest {
    pub slug: String,
}

#[derive(Debug, Serialize)]
pub struct SetSlugResponse {
    pub slug: String,
}

pub async fn rotate_code(
    State(state): State<AppState>,
    RequireAdmin(ctx): RequireAdmin,
) -> ApiResult<Json<RotateCodeResponse>> {
    use mongodb::error::{ErrorKind, WriteFailure};
    const DUPLICATE_KEY: i32 = 11000;

    for attempt in 0..ORG_CODE_RETRIES {
        let new_code = org_code::generate();
        match state.db.orgs.rotate_code(ctx.org_id, &new_code).await {
            Ok(org) => return Ok(Json(RotateCodeResponse { code: org.code })),
            Err(ApiError::Db(err)) => {
                let is_dup = matches!(
                    err.kind.as_ref(),
                    ErrorKind::Write(WriteFailure::WriteError(we)) if we.code == DUPLICATE_KEY
                );
                if is_dup && attempt + 1 < ORG_CODE_RETRIES {
                    tracing::warn!(?new_code, attempt, "org code collision on rotate; retrying");
                    continue;
                }
                return Err(ApiError::Db(err));
            }
            Err(other) => return Err(other),
        }
    }
    Err(ApiError::Internal)
}

pub async fn set_slug(
    State(state): State<AppState>,
    RequireAdmin(ctx): RequireAdmin,
    Json(req): Json<SetSlugRequest>,
) -> ApiResult<Json<SetSlugResponse>> {
    let normalized = slug_auth::normalize(&req.slug);
    slug_auth::validate(&normalized).map_err(|err| match err {
        SlugValidationError::InvalidFormat => ApiError::InvalidSlugFormat,
        SlugValidationError::Reserved => ApiError::SlugReserved,
    })?;

    let org = state
        .db
        .orgs
        .find_by_id(ctx.org_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let now = DateTime::now();
    enforce_rate_limit(&org, now)?;

    let updated = slug_auth::set_slug_atomic(&state.db, &org, &normalized, now, GRACE_TTL).await?;
    let slug = updated.slug.unwrap_or(normalized);
    Ok(Json(SetSlugResponse { slug }))
}

pub async fn clear_slug(
    State(state): State<AppState>,
    RequireAdmin(ctx): RequireAdmin,
) -> ApiResult<StatusCode> {
    let org = state
        .db
        .orgs
        .find_by_id(ctx.org_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let now = DateTime::now();
    enforce_rate_limit(&org, now)?;

    slug_auth::clear_slug_atomic(&state.db, &org, now, GRACE_TTL).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn enforce_rate_limit(org: &crate::domain::Org, now: DateTime) -> ApiResult<()> {
    let Some(last) = org.slug_changed_at else {
        return Ok(());
    };
    let elapsed_ms = now.timestamp_millis() - last.timestamp_millis();
    let window_ms = RATE_LIMIT_WINDOW.as_millis() as i64;
    if elapsed_ms < window_ms {
        let retry_after = DateTime::from_millis(last.timestamp_millis() + window_ms);
        return Err(ApiError::SlugChangeTooSoon { retry_after });
    }
    Ok(())
}

