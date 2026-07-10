//! Startup-time housekeeping. Currently just the checkin status-drift repair.
//!
//! ## Why this exists
//!
//! `checkin_events` is the source of truth; `checkin_user_status` is a
//! denormalised projection. The two writes (event insert + conditional
//! status update) are sequenced in code rather than in a Mongo transaction.
//! When the second write fails — process crash, network partition, the
//! conditional update losing a race — the projection drifts away from the
//! event log.
//!
//! The hot path tolerates this defensively: every event-submission re-reads
//! the status row before deciding the next state, so a stale projection
//! that says "off_duty" while the latest event was a `clock_in` will simply
//! reject the next `clock_in` attempt and let the AppUser try again.
//! That's noisy.
//!
//! This repair runs once at process startup. It scans `checkin_user_status`
//! against the latest event for each AppUser, and:
//!
//! - If a status row exists but disagrees with the latest event → fix it.
//! - If an AppUser has events but no status row → init one matching the
//!   latest event.
//! - If an AppUser has a status row but no events → leave it (could be
//!   brand-new, off_duty is correct).
//! - If `app_users` has rows but neither status nor events → init off_duty.
//!
//! The repair is idempotent and best-effort: failures are logged at warn
//! and the server still starts.

use bson::oid::ObjectId;

use crate::db::Db;
use crate::domain::{AppUserCheckinStatus, CheckinEvent, CheckinEventType};

pub async fn repair_checkin_status_drift(db: &Db) {
    if let Err(err) = repair_inner(db).await {
        tracing::warn!(?err, "checkin status repair failed; continuing startup");
    }
}

async fn repair_inner(db: &Db) -> Result<(), mongodb::error::Error> {
    use mongodb::bson::doc;

    // Pull every AppUser id. We iterate by id rather than by Org because
    // the canonical state per AppUser is what needs reconciling.
    let mut cursor = db
        .database
        .collection::<bson::Document>("app_users")
        .find(doc! {})
        .await?;

    let mut fixed = 0_u64;
    let mut initialised = 0_u64;

    while cursor.advance().await? {
        let raw = cursor.current();
        let id = match raw.get_object_id("_id") {
            Ok(v) => v,
            Err(_) => continue,
        };
        let org_id = match raw.get_object_id("org_id") {
            Ok(v) => v,
            Err(_) => continue,
        };

        match repair_one(db, id, org_id).await {
            Ok(RepairOutcome::Fixed) => fixed += 1,
            Ok(RepairOutcome::Initialised) => initialised += 1,
            Ok(RepairOutcome::Ok) => {}
            Err(err) => {
                tracing::warn!(?err, app_user_id = %id, "failed to repair status row");
            }
        }
    }

    if fixed > 0 || initialised > 0 {
        tracing::info!(fixed, initialised, "checkin status repair complete");
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub enum RepairOutcome {
    Ok,
    Fixed,
    Initialised,
}

/// Derive `checkin_user_status` for one AppUser from their latest event.
/// Reused by the legacy check-in backfill worker after inserting historical
/// events — the situation (events exist, no status row yet) lands squarely in
/// the `(None, Some(latest))` branch below, so no separate status-derivation
/// logic is needed for backfilled AppUsers.
pub async fn repair_one(
    db: &Db,
    app_user_id: ObjectId,
    org_id: ObjectId,
) -> Result<RepairOutcome, mongodb::error::Error> {
    let latest = match db.checkin_events.latest_for_app_user(app_user_id).await {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(?err, app_user_id = %app_user_id, "failed to read latest event during repair");
            return Ok(RepairOutcome::Ok);
        }
    };
    let status = match db.checkin_user_status.find(app_user_id).await {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(?err, app_user_id = %app_user_id, "failed to read status row during repair");
            return Ok(RepairOutcome::Ok);
        }
    };

    match (status, latest) {
        (None, None) => {
            // AppUser exists with no events and no status — init off_duty.
            // Don't fail on duplicate: a parallel boot may have done it.
            match db
                .checkin_user_status
                .init_off_duty(app_user_id, org_id)
                .await
            {
                Ok(_) => Ok(RepairOutcome::Initialised),
                Err(crate::db::CheckinStatusInsertError::Duplicate) => Ok(RepairOutcome::Ok),
                Err(crate::db::CheckinStatusInsertError::Db(err)) => Err(err),
            }
        }
        (None, Some(latest)) => {
            // AppUser has events but no status row. Init it to whatever the
            // latest event implies. We can't determine `current_shift_started_at`
            // perfectly without scanning history; we approximate by picking
            // the latest event's `occurred_at_client` if the implied state
            // is `on_site`/`in_transit`, and `null` otherwise.
            let implied = imply_status(&latest);
            match db
                .checkin_user_status
                .init_off_duty(app_user_id, org_id)
                .await
            {
                Ok(_) => {}
                Err(crate::db::CheckinStatusInsertError::Duplicate) => {}
                Err(crate::db::CheckinStatusInsertError::Db(err)) => return Err(err),
            }
            // Now update from off_duty to the implied state. update_to is
            // conditional on the prior; we just init'd to off_duty so it
            // will succeed unless there's a concurrent writer (in which
            // case skip — the writer's view is more authoritative).
            let started_at = if matches!(
                implied,
                AppUserCheckinStatus::OnSite | AppUserCheckinStatus::InTransit
            ) {
                Some(latest.occurred_at_client)
            } else {
                None
            };
            let _ = db
                .checkin_user_status
                .update_to(
                    app_user_id,
                    AppUserCheckinStatus::OffDuty,
                    implied,
                    started_at,
                    latest.id,
                )
                .await;
            Ok(RepairOutcome::Fixed)
        }
        (Some(status_row), None) => {
            // Status row exists but no events. Should be off_duty; if not,
            // reset.
            if matches!(status_row.status, AppUserCheckinStatus::OffDuty) {
                Ok(RepairOutcome::Ok)
            } else {
                let _ = db.checkin_user_status.delete_by_app_user(app_user_id).await;
                let _ = db
                    .checkin_user_status
                    .init_off_duty(app_user_id, org_id)
                    .await;
                Ok(RepairOutcome::Fixed)
            }
        }
        (Some(status_row), Some(latest)) => {
            let implied = imply_status(&latest);
            if status_row.status == implied && status_row.last_event_id == Some(latest.id) {
                return Ok(RepairOutcome::Ok);
            }
            // Drift detected. Re-seat by deleting the row and re-inserting
            // off_duty, then transition to the implied state. This is more
            // robust than trying to find_one_and_update from the wrong
            // prior — the conditional would just fail.
            let _ = db.checkin_user_status.delete_by_app_user(app_user_id).await;
            let _ = db
                .checkin_user_status
                .init_off_duty(app_user_id, org_id)
                .await;
            let started_at = if matches!(
                implied,
                AppUserCheckinStatus::OnSite | AppUserCheckinStatus::InTransit
            ) {
                Some(latest.occurred_at_client)
            } else {
                None
            };
            let _ = db
                .checkin_user_status
                .update_to(
                    app_user_id,
                    AppUserCheckinStatus::OffDuty,
                    implied,
                    started_at,
                    latest.id,
                )
                .await;
            Ok(RepairOutcome::Fixed)
        }
    }
}

/// What status does a single event imply on its own? Used by the repair
/// task to derive a target state from "the latest event". For the four
/// event types this is just the destination of the canonical transition;
/// for force-checkout (`clock_out`) the destination is `off_duty` regardless.
fn imply_status(latest: &CheckinEvent) -> AppUserCheckinStatus {
    match latest.event_type {
        CheckinEventType::ClockIn => AppUserCheckinStatus::OnSite,
        CheckinEventType::TransferIn => AppUserCheckinStatus::OnSite,
        CheckinEventType::TransferOut => AppUserCheckinStatus::InTransit,
        CheckinEventType::ClockOut => AppUserCheckinStatus::OffDuty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventInitiatorKind, EventLocation, EventSource, GeoPoint};
    use bson::DateTime;

    fn fake_event(event_type: CheckinEventType) -> CheckinEvent {
        CheckinEvent {
            id: ObjectId::new(),
            org_id: ObjectId::new(),
            app_user_id: ObjectId::new(),
            event_type,
            occurred_at_client: DateTime::now(),
            occurred_at_server: DateTime::now(),
            source: EventSource::App,
            initiated_by_kind: EventInitiatorKind::AppUser,
            initiated_by_id: ObjectId::new(),
            location: EventLocation {
                coordinates: GeoPoint { lat: 0.0, lng: 0.0 },
                accuracy_meters: None,
                region_name: None,
                manual_label: None,
            },
            reason: None,
        }
    }

    #[test]
    fn imply_status_clock_in_to_on_site() {
        let e = fake_event(CheckinEventType::ClockIn);
        assert_eq!(imply_status(&e), AppUserCheckinStatus::OnSite);
    }

    #[test]
    fn imply_status_transfer_out_to_in_transit() {
        let e = fake_event(CheckinEventType::TransferOut);
        assert_eq!(imply_status(&e), AppUserCheckinStatus::InTransit);
    }

    #[test]
    fn imply_status_clock_out_to_off_duty() {
        let e = fake_event(CheckinEventType::ClockOut);
        assert_eq!(imply_status(&e), AppUserCheckinStatus::OffDuty);
    }

    #[test]
    fn imply_status_transfer_in_to_on_site() {
        let e = fake_event(CheckinEventType::TransferIn);
        assert_eq!(imply_status(&e), AppUserCheckinStatus::OnSite);
    }
}
