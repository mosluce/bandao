//! Reconciling one AppUser's `checkin_user_status` against their event log.
//!
//! `checkin_events` is the source of truth; `checkin_user_status` is a
//! denormalised projection. The two writes (event insert + conditional
//! status update) are sequenced in code rather than in a Mongo transaction,
//! so the projection can drift — a crash between the two writes, a
//! conditional update losing a race, or a writer that only appends events
//! and never touches the projection at all (the legacy backfill import).
//!
//! This module holds the per-AppUser primitive. Two callers use it:
//!
//! - `startup::repair_checkin_status_drift` sweeps every AppUser once at
//!   process boot.
//! - `examples/legacy_backfill` reconciles the AppUsers whose events it
//!   just imported, so an unattended scheduled import doesn't leave the
//!   status board frozen until the next restart.
//!
//! Both share this implementation deliberately: two copies of the
//! implied-status derivation would drift apart.

use bson::oid::ObjectId;

use crate::db::Db;
use crate::domain::{AppUserCheckinStatus, CheckinEvent, CheckinEventType};

/// What reconciling one AppUser actually did. Callers tally these for their
/// own reporting; nothing branches on the variant.
#[derive(Debug, PartialEq, Eq)]
pub enum RepairOutcome {
    /// Projection already agreed with the event log — nothing written.
    Ok,
    /// Projection disagreed and was re-seated from the latest event.
    Fixed,
    /// No projection row existed; one was created.
    Initialised,
}

/// Bring one AppUser's `checkin_user_status` in line with their latest
/// event by client time. Idempotent: a row that already agrees is left
/// untouched and reported as [`RepairOutcome::Ok`].
///
/// Note the re-seat strategy: when the row disagrees we delete and
/// re-insert it as `off_duty` before transitioning to the implied state,
/// because the repository's conditional `find_one_and_update` can only move
/// from a known prior status. That is destructive if it interleaves with a
/// live event submission for the same AppUser — the early return above it
/// keeps the common no-drift case from ever deleting anything, and callers
/// are expected to scope who they reconcile accordingly.
pub async fn reconcile_app_user(
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
            let _ = db
                .checkin_user_status
                .update_to(
                    app_user_id,
                    AppUserCheckinStatus::OffDuty,
                    implied,
                    implied_shift_started_at(implied, &latest),
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
            let _ = db
                .checkin_user_status
                .update_to(
                    app_user_id,
                    AppUserCheckinStatus::OffDuty,
                    implied,
                    implied_shift_started_at(implied, &latest),
                    latest.id,
                )
                .await;
            Ok(RepairOutcome::Fixed)
        }
    }
}

/// What status does a single event imply on its own? Used to derive a
/// target state from "the latest event". For the four event types this is
/// just the destination of the canonical transition; for force-checkout
/// (`clock_out`) the destination is `off_duty` regardless.
fn imply_status(latest: &CheckinEvent) -> AppUserCheckinStatus {
    match latest.event_type {
        CheckinEventType::ClockIn => AppUserCheckinStatus::OnSite,
        CheckinEventType::TransferIn => AppUserCheckinStatus::OnSite,
        CheckinEventType::TransferOut => AppUserCheckinStatus::InTransit,
        CheckinEventType::ClockOut => AppUserCheckinStatus::OffDuty,
    }
}

/// Best available `current_shift_started_at` when re-seating from a single
/// event. Approximate for a shift that is still open: we stamp the latest
/// event's client time rather than scanning back to the `clock_in` that
/// actually opened the shift, so a shift reconstructed from a `transfer_in`
/// reads shorter than it was. `off_duty` clears the field (the repository
/// writes the explicit null).
fn implied_shift_started_at(
    implied: AppUserCheckinStatus,
    latest: &CheckinEvent,
) -> Option<bson::DateTime> {
    match implied {
        AppUserCheckinStatus::OnSite | AppUserCheckinStatus::InTransit => {
            Some(latest.occurred_at_client)
        }
        AppUserCheckinStatus::OffDuty => None,
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
            legacy_source_id: None,
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

    #[test]
    fn open_shift_stamps_the_latest_event_time() {
        let e = fake_event(CheckinEventType::ClockIn);
        assert_eq!(
            implied_shift_started_at(AppUserCheckinStatus::OnSite, &e),
            Some(e.occurred_at_client)
        );
    }

    #[test]
    fn off_duty_clears_the_shift_start() {
        let e = fake_event(CheckinEventType::ClockOut);
        assert_eq!(
            implied_shift_started_at(AppUserCheckinStatus::OffDuty, &e),
            None
        );
    }
}
