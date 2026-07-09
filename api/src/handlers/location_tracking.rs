//! `/app/checkin/locations` (AppUser bearer) — batch ingest of location
//! pings collected by the mobile client during a shift.
//!
//! `/app/checkin/me/locations` (AppUser bearer) — self-paginated query so
//! the AppUser can review their own movement in the "我的工作日記" surface.
//! Mirrors the admin list endpoint's validation rules; intentionally
//! NOT gated by the Org `location_tracking_enabled` toggle so an AppUser
//! can still read pings that were persisted before the toggle was turned
//! off (the toggle only gates ingest).
//!
//! `/checkin/users/:id/locations` (admin cookie) — paginated query.
//!
//! `/checkin/users/:id/locations/export` (admin cookie) — xlsx export of one
//! AppUser's pings within a 90-day-capped time range.
//!
//! Pings are NOT reverse-geocoded (volume rules out per-ping Nominatim
//! calls); admin map / xlsx render raw coordinates.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::Response;
use bson::DateTime;
use bson::oid::ObjectId;
use rust_xlsxwriter::{Format, FormatBorder, Workbook};
use serde::{Deserialize, Serialize};

use crate::auth::app_extractor::RequireAppUser;
use crate::auth::extractor::RequireAdmin;
use crate::db::LOCATION_PING_BATCH_MAX;
use crate::domain::LocationPing;
use crate::error::{ApiError, ApiResult};
use crate::handlers::app_checkin::parse_rfc3339;
use crate::state::AppState;

// --- Constants ---

const LIST_DEFAULT_LIMIT: i64 = 200;
const LIST_MAX_LIMIT: i64 = 1000;
/// Pings older than this are rejected per-row in `INVALID_PING_TIMESTAMP`.
/// Generous enough to absorb a phone that was offline for a long weekend
/// without dropping its backlog; tight enough that a >30-day-old ping is
/// almost certainly a client bug or a clock-skewed device.
const PING_MAX_AGE_DAYS: i64 = 30;
/// Cap on the (to - from) span of an export query. Aligns with the 90-day
/// TTL — admins can't export beyond the retention window anyway.
const EXPORT_RANGE_MAX_DAYS: i64 = 90;
const MILLIS_PER_DAY: i64 = 24 * 3600 * 1000;

// --- DTOs ---

#[derive(Debug, Deserialize)]
pub struct SubmitLocationPingsRequest {
    pub pings: Vec<LocationPingInput>,
}

#[derive(Debug, Deserialize)]
pub struct LocationPingInput {
    pub lat: f64,
    pub lng: f64,
    #[serde(default)]
    pub accuracy: Option<f64>,
    pub occurred_at_client: String,
}

#[derive(Debug, Serialize)]
pub struct SubmitLocationPingsResponse {
    pub accepted_count: u32,
    pub rejected: Vec<RejectedPingDto>,
}

#[derive(Debug, Serialize)]
pub struct RejectedPingDto {
    pub index: usize,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct LocationPingDto {
    pub id: String,
    pub app_user_id: String,
    pub lat: f64,
    pub lng: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy_meters: Option<f64>,
    pub occurred_at_client: String,
    pub occurred_at_server: String,
}

impl LocationPingDto {
    fn from_ping(p: &LocationPing) -> Self {
        Self {
            id: p.id.to_hex(),
            app_user_id: p.app_user_id.to_hex(),
            lat: p.lat,
            lng: p.lng,
            accuracy_meters: p.accuracy_meters,
            occurred_at_client: p
                .occurred_at_client
                .try_to_rfc3339_string()
                .unwrap_or_default(),
            occurred_at_server: p
                .occurred_at_server
                .try_to_rfc3339_string()
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LocationListQuery {
    #[serde(default)]
    pub before: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LocationExportQuery {
    pub from: Option<String>,
    pub to: Option<String>,
}

// --- POST /app/checkin/locations ---

/// AppUser-driven batch ingest. The toggle gate runs first (whole-batch
/// reject); per-ping validation produces `partial accept` semantics —
/// valid pings persist via `insert_many(ordered: false)` and any failures
/// land in the response's `rejected[]` with their original batch index.
pub async fn submit_location_pings(
    State(state): State<AppState>,
    RequireAppUser(ctx): RequireAppUser,
    Json(req): Json<SubmitLocationPingsRequest>,
) -> ApiResult<(StatusCode, Json<SubmitLocationPingsResponse>)> {
    // 1) Org toggle gate — whole batch.
    let org = state
        .db
        .orgs
        .find_by_id(ctx.org_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    if !org.checkin_location_tracking_enabled() {
        return Err(ApiError::LocationTrackingDisabled);
    }

    // 2) Batch size gate.
    if req.pings.is_empty() || req.pings.len() > LOCATION_PING_BATCH_MAX {
        return Err(ApiError::InvalidBatch);
    }

    // 3) Per-ping validation — pure Rust, no db I/O.
    let now = DateTime::now();
    let now_millis = now.timestamp_millis();
    let max_age_millis = PING_MAX_AGE_DAYS * MILLIS_PER_DAY;

    let mut rejected: Vec<RejectedPingDto> = Vec::new();
    // (original_index, valid_ping) pairs preserved so we can map mongo's
    // sub-array indices back to caller-visible batch indices.
    let mut valid: Vec<(usize, LocationPing)> = Vec::new();

    for (idx, p) in req.pings.iter().enumerate() {
        if let Err(reject) = validate_ping(p, now_millis, max_age_millis) {
            rejected.push(RejectedPingDto {
                index: idx,
                code: reject.code,
                message: reject.message,
            });
            continue;
        }
        let parsed_client_ts = match parse_rfc3339(&p.occurred_at_client) {
            Ok(t) => t,
            Err(_) => {
                // already-validated by validate_ping; theoretically unreachable
                rejected.push(RejectedPingDto {
                    index: idx,
                    code: "INVALID_PING_TIMESTAMP".to_string(),
                    message: format!("invalid RFC3339 timestamp: `{}`", p.occurred_at_client),
                });
                continue;
            }
        };
        valid.push((
            idx,
            LocationPing {
                id: ObjectId::new(),
                org_id: ctx.org_id,
                app_user_id: ctx.app_user_id,
                lat: p.lat,
                lng: p.lng,
                accuracy_meters: p.accuracy,
                occurred_at_client: parsed_client_ts,
                occurred_at_server: now,
            },
        ));
    }

    // 4) Insert valid pings (if any).
    let mut accepted_count: u32 = 0;
    if !valid.is_empty() {
        let pings: Vec<LocationPing> = valid.iter().map(|(_, p)| p.clone()).collect();
        let outcome = state
            .db
            .location_pings
            .insert_many_unordered(&pings)
            .await?;
        accepted_count = outcome.inserted_indices.len() as u32;
        for (sub_idx, code) in outcome.failed_indices {
            // sub_idx → original batch index via the (orig_idx, _) tuple
            let orig_idx = valid[sub_idx].0;
            rejected.push(RejectedPingDto {
                index: orig_idx,
                code,
                message: "database insert failed".to_string(),
            });
        }
    }

    // Sort rejected by index so the response is stable for clients.
    rejected.sort_by_key(|r| r.index);

    Ok((
        StatusCode::CREATED,
        Json(SubmitLocationPingsResponse {
            accepted_count,
            rejected,
        }),
    ))
}

struct PingRejection {
    code: String,
    message: String,
}

fn validate_ping(
    p: &LocationPingInput,
    now_millis: i64,
    max_age_millis: i64,
) -> Result<(), PingRejection> {
    if !(-90.0..=90.0).contains(&p.lat) || !(-180.0..=180.0).contains(&p.lng) {
        return Err(PingRejection {
            code: "INVALID_PING_COORDINATES".to_string(),
            message: format!("coordinates out of range: lat={}, lng={}", p.lat, p.lng),
        });
    }
    if let Some(acc) = p.accuracy
        && acc < 0.0
    {
        return Err(PingRejection {
            code: "INVALID_PING_COORDINATES".to_string(),
            message: format!("accuracy must be >= 0: {acc}"),
        });
    }
    let parsed = parse_rfc3339(&p.occurred_at_client).map_err(|_| PingRejection {
        code: "INVALID_PING_TIMESTAMP".to_string(),
        message: format!("invalid RFC3339 timestamp: `{}`", p.occurred_at_client),
    })?;
    let client_millis = parsed.timestamp_millis();
    if client_millis > now_millis {
        return Err(PingRejection {
            code: "INVALID_PING_TIMESTAMP".to_string(),
            message: format!(
                "occurred_at_client is in the future: `{}`",
                p.occurred_at_client
            ),
        });
    }
    if now_millis - client_millis > max_age_millis {
        return Err(PingRejection {
            code: "INVALID_PING_TIMESTAMP".to_string(),
            message: format!(
                "occurred_at_client is older than {} days: `{}`",
                PING_MAX_AGE_DAYS, p.occurred_at_client
            ),
        });
    }
    Ok(())
}

// --- GET /app/checkin/me/locations ---

/// Self-list — the AppUser reads their own pings. Same validation and
/// ordering as the admin endpoint; identity is taken from the bearer
/// token, never from the request body or path.
pub async fn list_my_locations(
    State(state): State<AppState>,
    RequireAppUser(ctx): RequireAppUser,
    Query(q): Query<LocationListQuery>,
) -> ApiResult<Json<Vec<LocationPingDto>>> {
    let before = match q.before.as_deref() {
        Some(raw) => Some(parse_rfc3339(raw)?),
        None => None,
    };
    let from = match q.from.as_deref() {
        Some(raw) => Some(parse_rfc3339(raw).map_err(|_| ApiError::InvalidRange)?),
        None => None,
    };
    let to = match q.to.as_deref() {
        Some(raw) => Some(parse_rfc3339(raw).map_err(|_| ApiError::InvalidRange)?),
        None => None,
    };
    if from.is_some() || to.is_some() {
        validate_range(from, to)?;
    }
    let limit = q
        .limit
        .unwrap_or(LIST_DEFAULT_LIMIT)
        .clamp(1, LIST_MAX_LIMIT);

    let pings = state
        .db
        .location_pings
        .list_by_app_user_paginated(ctx.app_user_id, before, from, to, limit)
        .await?;
    Ok(Json(pings.iter().map(LocationPingDto::from_ping).collect()))
}

// --- GET /checkin/users/:id/locations ---

pub async fn list_locations(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id_hex): Path<String>,
    Query(q): Query<LocationListQuery>,
) -> ApiResult<Json<Vec<LocationPingDto>>> {
    let app_user_id = ObjectId::parse_str(&id_hex).map_err(|_| ApiError::NotFound)?;

    // Cross-Org check: the AppUser must belong to the caller's current_org.
    let app_user = state
        .db
        .app_users
        .find_by_id(app_user_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if app_user.org_id != active.org_id {
        return Err(ApiError::NotFound);
    }

    let before = match q.before.as_deref() {
        Some(raw) => Some(parse_rfc3339(raw)?),
        None => None,
    };
    let from = match q.from.as_deref() {
        Some(raw) => Some(parse_rfc3339(raw).map_err(|_| ApiError::InvalidRange)?),
        None => None,
    };
    let to = match q.to.as_deref() {
        Some(raw) => Some(parse_rfc3339(raw).map_err(|_| ApiError::InvalidRange)?),
        None => None,
    };
    if from.is_some() || to.is_some() {
        validate_range(from, to)?;
    }
    let limit = q
        .limit
        .unwrap_or(LIST_DEFAULT_LIMIT)
        .clamp(1, LIST_MAX_LIMIT);

    let pings = state
        .db
        .location_pings
        .list_by_app_user_paginated(app_user_id, before, from, to, limit)
        .await?;
    Ok(Json(pings.iter().map(LocationPingDto::from_ping).collect()))
}

/// Shared range validator used by list + export. Each absent side is a
/// no-op for its corresponding check (single-sided ranges are allowed).
fn validate_range(from: Option<DateTime>, to: Option<DateTime>) -> ApiResult<()> {
    let span_max_millis = EXPORT_RANGE_MAX_DAYS * MILLIS_PER_DAY;
    let now_millis = DateTime::now().timestamp_millis();
    if let (Some(f), Some(t)) = (from, to) {
        let from_ms = f.timestamp_millis();
        let to_ms = t.timestamp_millis();
        if to_ms < from_ms || to_ms - from_ms > span_max_millis {
            return Err(ApiError::InvalidRange);
        }
    }
    if let Some(f) = from {
        let from_ms = f.timestamp_millis();
        if now_millis - from_ms > span_max_millis {
            return Err(ApiError::InvalidRange);
        }
    }
    Ok(())
}

// --- GET /checkin/users/:id/locations/export ---

pub async fn export_locations(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id_hex): Path<String>,
    Query(q): Query<LocationExportQuery>,
) -> ApiResult<Response> {
    let app_user_id = ObjectId::parse_str(&id_hex).map_err(|_| ApiError::NotFound)?;

    let app_user = state
        .db
        .app_users
        .find_by_id(app_user_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if app_user.org_id != active.org_id {
        return Err(ApiError::NotFound);
    }

    // Export requires both sides; reuse the shared validator that the list
    // endpoint also runs.
    let from_raw = q.from.as_deref().ok_or(ApiError::InvalidRange)?;
    let to_raw = q.to.as_deref().ok_or(ApiError::InvalidRange)?;
    let from = parse_rfc3339(from_raw).map_err(|_| ApiError::InvalidRange)?;
    let to = parse_rfc3339(to_raw).map_err(|_| ApiError::InvalidRange)?;
    validate_range(Some(from), Some(to))?;

    let pings = state
        .db
        .location_pings
        .list_for_export(app_user_id, from, to)
        .await?;

    let bytes = build_xlsx(&pings).map_err(|err| {
        tracing::error!(?err, "xlsx build failed");
        ApiError::Internal
    })?;

    let from_date = from_raw.split('T').next().unwrap_or("from");
    let to_date = to_raw.split('T').next().unwrap_or("to");
    let user_label = app_user
        .username
        .as_deref()
        .or(app_user.external_key.as_deref())
        .unwrap_or("user");
    let filename = format!(
        "bandao-locations-{}-{}-{}.xlsx",
        user_label, from_date, to_date
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(
            CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(bytes.into())
        .map_err(|err| {
            tracing::error!(?err, "xlsx response build failed");
            ApiError::Internal
        })
}

fn build_xlsx(pings: &[LocationPing]) -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet().set_name("locations")?;

    // Header row: bold, bottom border, frozen.
    let header_format = Format::new()
        .set_bold()
        .set_border_bottom(FormatBorder::Thin);
    let headers = [
        "occurred_at_client (UTC)",
        "occurred_at_server (UTC)",
        "lat",
        "lng",
        "accuracy_meters",
    ];
    for (col, label) in headers.iter().enumerate() {
        sheet.write_string_with_format(0, col as u16, *label, &header_format)?;
    }
    sheet.set_freeze_panes(1, 0)?;

    // Sensible column widths.
    sheet.set_column_width(0, 24)?;
    sheet.set_column_width(1, 24)?;
    sheet.set_column_width(2, 12)?;
    sheet.set_column_width(3, 12)?;
    sheet.set_column_width(4, 14)?;

    for (i, ping) in pings.iter().enumerate() {
        let row = (i + 1) as u32;
        sheet.write_string(
            row,
            0,
            ping.occurred_at_client
                .try_to_rfc3339_string()
                .unwrap_or_default(),
        )?;
        sheet.write_string(
            row,
            1,
            ping.occurred_at_server
                .try_to_rfc3339_string()
                .unwrap_or_default(),
        )?;
        sheet.write_number(row, 2, ping.lat)?;
        sheet.write_number(row, 3, ping.lng)?;
        if let Some(acc) = ping.accuracy_meters {
            sheet.write_number(row, 4, acc)?;
        }
    }

    workbook.save_to_buffer()
}
