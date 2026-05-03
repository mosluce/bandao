//! `/checkin/*` (admin live-board, history, force-checkout) plus
//! `PATCH /orgs/me/settings`.
//!
//! These endpoints sit under the dashboard cookie + admin guard. The shape
//! of the live-board row, the cross-Org NOT_FOUND collapse, and the
//! state-lock semantics are all spelled out here rather than hidden behind
//! a service layer — the logic is small and reading it inline is easier
//! than chasing helpers.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use bson::DateTime;
use bson::oid::ObjectId;

use crate::auth::extractor::RequireAdmin;
use crate::domain::{
    AppUser, AppUserCheckinStatus, CheckinEvent, CheckinEventType, CheckinUserStatus,
    EventInitiatorKind, EventLocation, EventSource, GeoPoint,
};
use crate::error::{ApiError, ApiResult};
use crate::handlers::checkin_dto::{
    BoardAppUserDto, CheckinEventDto, CheckinUserBoardRowDto, ForceCheckoutRequest,
    OrgSettingsDto, UpdateOrgSettingsRequest,
};
use crate::state::AppState;

const REASON_MAX: usize = 240;
const FORCE_CHECKOUT_LABEL: &str = "管理員強制收班";

const DEFAULT_PAGE_SIZE: i64 = 50;
const MAX_PAGE_SIZE: i64 = 200;

/// `GET /checkin/users` — admin live status board for `current_org`.
pub async fn list_users(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
) -> ApiResult<Json<Vec<CheckinUserBoardRowDto>>> {
    let users = state.db.app_users.list_by_org(active.org_id).await?;
    if users.is_empty() {
        return Ok(Json(Vec::new()));
    }

    // Pull every status row in this Org in one query; index keyed
    // `(org_id, status)` covers it. Build a HashMap for the join.
    let statuses = state
        .db
        .checkin_user_status
        .list_by_org(active.org_id)
        .await?;
    let mut status_by_user: HashMap<ObjectId, CheckinUserStatus> = statuses
        .into_iter()
        .map(|s| (s.app_user_id, s))
        .collect();

    let mut rows = Vec::with_capacity(users.len());
    for user in &users {
        let status_row = match status_by_user.remove(&user.id) {
            Some(s) => s,
            None => {
                // Defensive: AppUsers without a status row treated as off_duty.
                // Startup repair fixes drift but a brand-new AppUser created
                // before this change rolled out hits this path.
                CheckinUserStatus {
                    app_user_id: user.id,
                    org_id: user.org_id,
                    status: AppUserCheckinStatus::OffDuty,
                    current_shift_started_at: None,
                    last_event_id: None,
                    updated_at: DateTime::now(),
                }
            }
        };
        let last_event = match status_row.last_event_id {
            Some(id) => state.db.checkin_events.find_by_id(id).await?,
            None => None,
        };
        rows.push(build_board_row(user, &status_row, last_event.as_ref()));
    }

    Ok(Json(rows))
}

fn build_board_row(
    user: &AppUser,
    status_row: &CheckinUserStatus,
    last_event: Option<&CheckinEvent>,
) -> CheckinUserBoardRowDto {
    let last_event_dto = last_event.map(CheckinEventDto::from_event);
    let has_skew_warning = last_event_dto
        .as_ref()
        .map(|e| e.has_skew_warning)
        .unwrap_or(false);
    CheckinUserBoardRowDto {
        user: BoardAppUserDto::from_app_user(user),
        status: status_row.status,
        current_shift_started_at: status_row
            .current_shift_started_at
            .and_then(|d| d.try_to_rfc3339_string().ok()),
        last_event: last_event_dto,
        has_skew_warning,
    }
}

/// `GET /checkin/users/:id/events` — single AppUser's history.
pub async fn list_user_events(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id): Path<String>,
    Query(q): Query<crate::handlers::checkin_dto::EventsCursorQuery>,
) -> ApiResult<Json<Vec<CheckinEventDto>>> {
    let target_id = ObjectId::parse_str(&id).map_err(|_| ApiError::NotFound)?;
    // Cross-Org collapses to NOT_FOUND — same pattern as `/app-users/:id`.
    let user = state
        .db
        .app_users
        .find_by_id(target_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if user.org_id != active.org_id {
        return Err(ApiError::NotFound);
    }

    let before = match q.before.as_deref() {
        Some(raw) => Some(crate::handlers::app_checkin::parse_rfc3339(raw)?),
        None => None,
    };
    let limit = q.limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);

    let events = state
        .db
        .checkin_events
        .list_by_app_user_paginated(user.id, before, limit)
        .await?;
    Ok(Json(events.iter().map(CheckinEventDto::from_event).collect()))
}

/// `POST /checkin/users/:id/force-checkout` — admin synthesises a
/// `clock_out` event with `source = AdminForce`.
pub async fn force_checkout(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Path(id): Path<String>,
    body: Option<Json<ForceCheckoutRequest>>,
) -> ApiResult<Json<crate::handlers::checkin_dto::SubmitCheckinEventResponse>> {
    let target_id = ObjectId::parse_str(&id).map_err(|_| ApiError::NotFound)?;
    let user = state
        .db
        .app_users
        .find_by_id(target_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if user.org_id != active.org_id {
        return Err(ApiError::NotFound);
    }

    let req = body.map(|Json(b)| b).unwrap_or_default();
    let reason = req
        .reason
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(r) = reason.as_deref() {
        if r.chars().count() > REASON_MAX {
            return Err(ApiError::Validation(format!(
                "reason length must be <= {REASON_MAX}"
            )));
        }
    }

    let status_row = state
        .db
        .checkin_user_status
        .find(user.id)
        .await?
        .ok_or(ApiError::NotOnDuty)?;
    if matches!(status_row.status, AppUserCheckinStatus::OffDuty) {
        return Err(ApiError::NotOnDuty);
    }

    // Copy location from the last event. Defensive synthetic when missing
    // (shouldn't happen if status != off_duty, but the failure mode is
    // ugly so we don't 500).
    let last_event = match status_row.last_event_id {
        Some(eid) => state.db.checkin_events.find_by_id(eid).await?,
        None => None,
    };
    let location = match last_event.as_ref() {
        Some(e) => EventLocation {
            coordinates: GeoPoint {
                lat: e.location.coordinates.lat,
                lng: e.location.coordinates.lng,
            },
            accuracy_meters: e.location.accuracy_meters,
            region_name: e.location.region_name.clone(),
            manual_label: Some(FORCE_CHECKOUT_LABEL.to_string()),
        },
        None => EventLocation {
            coordinates: GeoPoint { lat: 0.0, lng: 0.0 },
            accuracy_meters: None,
            region_name: None,
            manual_label: Some(FORCE_CHECKOUT_LABEL.to_string()),
        },
    };

    let now = DateTime::now();
    let prior_status = status_row.status;

    // The state machine still validates: in_transit and on_site both have
    // clock_out as a legal transition.
    let new_status = CheckinEventType::ClockOut
        .next_status(prior_status)
        .ok_or(ApiError::InvalidTransition {
            from: prior_status,
            attempted: CheckinEventType::ClockOut,
        })?;

    let event = state
        .db
        .checkin_events
        .create(
            user.org_id,
            user.id,
            CheckinEventType::ClockOut,
            now,
            now,
            EventSource::AdminForce,
            EventInitiatorKind::DashboardUser,
            active.ctx.user_id,
            location,
            reason,
        )
        .await?;

    let updated = state
        .db
        .checkin_user_status
        .update_to(user.id, prior_status, new_status, None, event.id)
        .await?;
    let updated_status = match updated {
        Some(s) => s,
        None => {
            // Race: someone clocked them out between our read and our write.
            if let Err(err) = state.db.checkin_events.delete_by_id(event.id).await {
                tracing::warn!(?err, event_id = %event.id, "failed to rewind force-checkout after race");
            }
            return Err(ApiError::NotOnDuty);
        }
    };

    let event_dto = CheckinEventDto::from_event(&event);
    let status_dto = crate::handlers::checkin_dto::CheckinUserStatusDto::from_status(
        &updated_status,
        Some(&event),
    );
    Ok(Json(crate::handlers::checkin_dto::SubmitCheckinEventResponse {
        event: event_dto,
        status: status_dto,
    }))
}

/// `PATCH /orgs/me/settings` — admin updates `transfer_enabled` and/or
/// `timezone`. State-lock applies only when `transfer_enabled` is part of
/// the patch; `timezone` updates are always allowed.
pub async fn update_settings(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Json(req): Json<UpdateOrgSettingsRequest>,
) -> ApiResult<Json<OrgSettingsDto>> {
    if req.transfer_enabled.is_none() && req.timezone.is_none() {
        // No-op patch — echo current settings rather than 400.
        let org = state
            .db
            .orgs
            .find_by_id(active.org_id)
            .await?
            .ok_or(ApiError::NotFound)?;
        return Ok(Json(OrgSettingsDto::from_org(&org)));
    }

    if req.transfer_enabled.is_some() {
        let on_duty_count = state
            .db
            .checkin_user_status
            .count_on_duty_in_org(active.org_id)
            .await?;
        if on_duty_count > 0 {
            return Err(ApiError::StateLocked {
                on_duty_count: on_duty_count.min(u32::MAX as u64) as u32,
            });
        }
    }

    if let Some(tz) = req.timezone.as_deref() {
        validate_timezone(tz)?;
    }

    let updated = state
        .db
        .orgs
        .update_settings(active.org_id, req.transfer_enabled, req.timezone.as_deref())
        .await?;
    Ok(Json(OrgSettingsDto::from_org(&updated)))
}

/// IANA timezone validation. Empty string and unknown names both surface as
/// `INVALID_TIMEZONE`. The validator is in `services::timezone` — see that
/// module for why we ship our own list rather than pulling in `chrono-tz`.
fn validate_timezone(raw: &str) -> ApiResult<()> {
    if crate::services::timezone::is_valid_iana(raw) {
        Ok(())
    } else {
        Err(ApiError::InvalidTimezone)
    }
}

#[allow(dead_code)]
fn _ensure_status_used(_s: StatusCode) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timezone_validation_accepts_iana() {
        assert!(validate_timezone("Asia/Taipei").is_ok());
        assert!(validate_timezone("America/Los_Angeles").is_ok());
        assert!(validate_timezone("UTC").is_ok());
    }

    #[test]
    fn timezone_validation_rejects_garbage() {
        assert!(validate_timezone("Mars/Olympus").is_err());
        assert!(validate_timezone("GMT+8").is_err());
        assert!(validate_timezone("").is_err());
    }
}
