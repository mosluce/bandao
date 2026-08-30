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
//! This sweep runs once at process startup and applies
//! [`crate::services::checkin_status::reconcile_app_user`] to every
//! AppUser, which per AppUser:
//!
//! - If a status row exists but disagrees with the latest event → fixes it.
//! - If an AppUser has events but no status row → inits one matching the
//!   latest event.
//! - If an AppUser has a status row but no events → leaves it (could be
//!   brand-new, off_duty is correct).
//! - If `app_users` has rows but neither status nor events → inits off_duty.
//!
//! The sweep is idempotent and best-effort: failures are logged at warn
//! and the server still starts.
//!
//! It is not the only caller of that primitive, and deliberately not the
//! safety net it once was: the legacy backfill import reconciles the
//! AppUsers it touches at the end of each run, because it runs unattended
//! on a schedule with no restart to wait for.

use crate::db::Db;
use crate::services::checkin_status::{RepairOutcome, reconcile_app_user};

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

        match reconcile_app_user(db, id, org_id).await {
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
