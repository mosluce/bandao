//! `/app/checkin/*` — AppUser-facing event submission and queries.
//!
//! `POST /app/checkin/events` is the busy path. Order matters and is
//! intentionally documented inline:
//!
//! 1. Parse + validate the body (event_type, RFC3339 client time, optional
//!    fields).
//! 2. Resolve the AppUser's current `CheckinUserStatus`. Auto-init off_duty
//!    when missing — startup repair handles drift, but a brand-new AppUser
//!    that never had a status row inserted (older bootstraps, manual
//!    fixtures) shouldn't be locked out of clock-in.
//! 3. State-machine: reject unsupported `(prior, event)` with
//!    `INVALID_TRANSITION` and DO NOT touch the DB.
//! 4. Transfer-toggle: reject `transfer_*` when `Org.settings.checkin.transfer_enabled == false`.
//! 5. Ordering: reject `client <= last_event.client` (per-AppUser only,
//!    first event always passes).
//! 6. Reverse-geocode synchronously (fail-soft → `region_name = null`).
//! 7. Insert `checkin_events` row, then `update_to(prior, new_status, ...)`.
//!    On race (None returned), best-effort delete the event row and emit
//!    `INVALID_TRANSITION`.
//!
//! Concurrency note: we deliberately don't use Mongo transactions. The
//! conditional `find_one_and_update` matching the prior status is the
//! atomicity guarantee — a second-arriving request loses the conditional
//! update and rewinds.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use bson::DateTime;

use crate::auth::app_extractor::RequireAppUser;
use crate::db::CheckinStatusInsertError;
use crate::domain::{
    AppUserCheckinStatus, CheckinEvent, CheckinEventType, CheckinUserStatus, EventInitiatorKind,
    EventLocation, EventSource, GeoPoint,
};
use crate::error::{ApiError, ApiResult};
use crate::handlers::checkin_dto::{
    CheckinEventDto, CheckinUserStatusDto, EventsCursorQuery, SubmitCheckinEventRequest,
    SubmitCheckinEventResponse,
};
use crate::state::AppState;

pub(crate) const MANUAL_LABEL_MIN: usize = 1;
pub(crate) const MANUAL_LABEL_MAX: usize = 120;
const DEFAULT_PAGE_SIZE: i64 = 50;
const MAX_PAGE_SIZE: i64 = 200;

/// `POST /app/checkin/events` — submit one event.
pub async fn submit_event(
    State(state): State<AppState>,
    RequireAppUser(ctx): RequireAppUser,
    Json(req): Json<SubmitCheckinEventRequest>,
) -> ApiResult<(StatusCode, Json<SubmitCheckinEventResponse>)> {
    // 1) Validate body.
    let occurred_at_client = parse_rfc3339(&req.occurred_at_client)?;
    let manual_label = req
        .manual_label
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(label) = manual_label.as_deref() {
        let len = label.chars().count();
        if !(MANUAL_LABEL_MIN..=MANUAL_LABEL_MAX).contains(&len) {
            return Err(ApiError::Validation(format!(
                "manual_label length must be {MANUAL_LABEL_MIN}..={MANUAL_LABEL_MAX}"
            )));
        }
    }

    // 2) Resolve status row, auto-init when missing (defensive).
    let status_row = ensure_status_row(&state, &ctx).await?;

    let event_type = req.event_type;
    let prior_status = status_row.status;

    // 3) State-machine.
    let new_status = event_type
        .next_status(prior_status)
        .ok_or(ApiError::InvalidTransition {
            from: prior_status,
            attempted: event_type,
        })?;

    // 4) Transfer toggle. clock_in/clock_out are unaffected. Read the Org
    // fresh — admin may have flipped the toggle between the AppUser's
    // last event and this one.
    if event_type.is_transfer() {
        let org = state
            .db
            .orgs
            .find_by_id(ctx.org_id)
            .await?
            .ok_or(ApiError::Unauthorized)?;
        if !org.checkin_transfer_enabled() {
            return Err(ApiError::TransferDisabled);
        }
    }

    // 5) Ordering check.
    if let Some(latest) = state
        .db
        .checkin_events
        .latest_for_app_user(ctx.app_user_id)
        .await?
        && occurred_at_client.timestamp_millis() <= latest.occurred_at_client.timestamp_millis()
    {
        return Err(ApiError::OutOfOrder);
    }

    // 6) Reverse-geocode synchronously (fail-soft).
    let region_name = state.geocoder.lookup(req.lat, req.lng).await;

    // 7) Insert event then conditionally update status. Note: we capture
    // `now` once so the event and the status row agree on `occurred_at_server`.
    let occurred_at_server = DateTime::now();
    let location = EventLocation {
        coordinates: GeoPoint {
            lat: req.lat,
            lng: req.lng,
        },
        accuracy_meters: req.accuracy,
        region_name,
        manual_label,
    };
    let event = state
        .db
        .checkin_events
        .create(
            ctx.org_id,
            ctx.app_user_id,
            event_type,
            occurred_at_client,
            occurred_at_server,
            EventSource::App,
            EventInitiatorKind::AppUser,
            ctx.app_user_id,
            location,
            None,
        )
        .await?;

    let new_started_at = next_shift_started_at(prior_status, new_status, occurred_at_client);
    let updated = state
        .db
        .checkin_user_status
        .update_to(
            ctx.app_user_id,
            prior_status,
            new_status,
            new_started_at,
            event.id,
        )
        .await?;

    let updated_status = match updated {
        Some(s) => s,
        None => {
            // Race lost: another concurrent request flipped the prior
            // status between our read and our write. Roll back the event
            // row best-effort and surface INVALID_TRANSITION so the client
            // can re-derive.
            if let Err(err) = state.db.checkin_events.delete_by_id(event.id).await {
                tracing::warn!(?err, event_id = %event.id, "failed to rewind checkin event after race");
            }
            return Err(ApiError::InvalidTransition {
                from: prior_status,
                attempted: event_type,
            });
        }
    };

    let event_dto = CheckinEventDto::from_event(&event);
    let status_dto = CheckinUserStatusDto::from_status(&updated_status, Some(&event));
    Ok((
        StatusCode::CREATED,
        Json(SubmitCheckinEventResponse {
            event: event_dto,
            status: status_dto,
        }),
    ))
}

/// `GET /app/checkin/status` — caller's current state plus their last event.
pub async fn status(
    State(state): State<AppState>,
    RequireAppUser(ctx): RequireAppUser,
) -> ApiResult<Json<CheckinUserStatusDto>> {
    let status_row = ensure_status_row(&state, &ctx).await?;
    let last_event = match status_row.last_event_id {
        Some(id) => state.db.checkin_events.find_by_id(id).await?,
        None => None,
    };
    Ok(Json(CheckinUserStatusDto::from_status(
        &status_row,
        last_event.as_ref(),
    )))
}

/// `GET /app/checkin/events` — caller's own events, newest-first by client time.
pub async fn list_events(
    State(state): State<AppState>,
    RequireAppUser(ctx): RequireAppUser,
    Query(q): Query<EventsCursorQuery>,
) -> ApiResult<Json<Vec<CheckinEventDto>>> {
    let before = match q.before.as_deref() {
        Some(raw) => Some(parse_rfc3339(raw)?),
        None => None,
    };
    let limit = q.limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);

    let events = state
        .db
        .checkin_events
        .list_by_app_user_paginated(ctx.app_user_id, before, limit)
        .await?;
    Ok(Json(
        events.iter().map(CheckinEventDto::from_event).collect(),
    ))
}

/// Guarantee a `checkin_user_status` row exists for the AppUser. Brand-new
/// AppUsers ought to have one inserted by the create handler, but this
/// handler is also reachable for older fixtures and during migration.
async fn ensure_status_row(
    state: &AppState,
    ctx: &crate::auth::app_extractor::AppAuthContext,
) -> ApiResult<CheckinUserStatus> {
    if let Some(row) = state.db.checkin_user_status.find(ctx.app_user_id).await? {
        return Ok(row);
    }
    match state
        .db
        .checkin_user_status
        .init_off_duty(ctx.app_user_id, ctx.org_id)
        .await
    {
        Ok(row) => Ok(row),
        Err(CheckinStatusInsertError::Duplicate) => state
            .db
            .checkin_user_status
            .find(ctx.app_user_id)
            .await?
            .ok_or(ApiError::Internal),
        Err(CheckinStatusInsertError::Db(err)) => Err(ApiError::Db(err)),
    }
}

pub(crate) fn parse_rfc3339(raw: &str) -> ApiResult<DateTime> {
    use ::time::OffsetDateTime;
    use ::time::format_description::well_known::Rfc3339;
    let parsed = OffsetDateTime::parse(raw, &Rfc3339)
        .map_err(|_| ApiError::Validation(format!("invalid RFC3339 timestamp: `{raw}`")))?;
    // OffsetDateTime → milliseconds-since-epoch. unix_timestamp_nanos returns
    // i128; convert to i64 millis. The cast saturates on overflow but
    // realistic inputs won't trip it.
    let nanos = parsed.unix_timestamp_nanos();
    let millis: i64 = (nanos / 1_000_000) as i64;
    Ok(DateTime::from_millis(millis))
}

/// Decide what to do with `current_shift_started_at` for a transition.
/// Mirrors design.md: starting on_site from off_duty stamps it, leaving on
/// `clock_out` clears it (returns `None` and the repo handles the null write
/// when the new status is `off_duty`), transfer events keep the existing
/// value (returns `None`, repo treats it as a no-op).
pub(crate) fn next_shift_started_at(
    prior: AppUserCheckinStatus,
    new_status: AppUserCheckinStatus,
    occurred_at_client: DateTime,
) -> Option<DateTime> {
    use AppUserCheckinStatus::*;
    match (prior, new_status) {
        (OffDuty, OnSite) => Some(occurred_at_client),
        // For everything else the repo handles the semantics — `OffDuty`
        // always nulls the field, and other transitions keep the old value.
        _ => None,
    }
}

#[allow(dead_code)]
fn _ensure_used(_e: &CheckinEvent, _t: CheckinEventType) {}
