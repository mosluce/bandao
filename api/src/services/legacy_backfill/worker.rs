//! Persistent worker loop for `legacy_backfill_jobs`. Started once at process
//! boot (`tokio::spawn`, alongside the existing one-shot
//! `startup::repair_checkin_status_drift`) and runs for the lifetime of the
//! process — this is the codebase's first long-running background loop.
//!
//! No Redis / external queue system: jobs live in Mongo, and claiming one is
//! guarded by the same conditional `find_one_and_update` idiom already used
//! for `checkin_user_status` transitions, so concurrent workers (if this ever
//! runs with >1 replica) cannot claim the same job twice.

use std::time::{Duration, SystemTime};

use bson::DateTime;

use crate::auth::secret_box::SecretBox;
use crate::domain::LegacyBackfillJob;
use crate::state::AppState;

use super::provider::{self, LegacyBackfillError};

const TICK_INTERVAL: Duration = Duration::from_secs(10);
/// A job left `active` longer than this is assumed abandoned by a
/// crashed/restarted worker and reset to `pending`.
const STALE_ACTIVE_THRESHOLD: Duration = Duration::from_secs(5 * 60);
/// Attempts cap (design D9): beyond this, a job becomes terminally `failed`
/// and needs manual intervention rather than retrying forever.
const MAX_ATTEMPTS: u32 = 5;

pub async fn run_worker_loop(state: AppState) {
    loop {
        recover_stale(&state).await;
        drain_due_jobs(&state).await;
        tokio::time::sleep(TICK_INTERVAL).await;
    }
}

async fn recover_stale(state: &AppState) {
    let locked_before = DateTime::from_system_time(SystemTime::now() - STALE_ACTIVE_THRESHOLD);
    match state
        .db
        .legacy_backfill_jobs
        .recover_stale(locked_before)
        .await
    {
        Ok(0) => {}
        Ok(n) => tracing::info!(recovered = n, "recovered stale legacy backfill jobs"),
        Err(err) => tracing::warn!(?err, "failed to recover stale legacy backfill jobs"),
    }
}

/// Claim and process due jobs one at a time until none remain, so a burst of
/// logins doesn't take multiple `TICK_INTERVAL`s to work through.
async fn drain_due_jobs(state: &AppState) {
    loop {
        let job = match state
            .db
            .legacy_backfill_jobs
            .claim_due(DateTime::now())
            .await
        {
            Ok(Some(job)) => job,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!(?err, "failed to claim legacy backfill job");
                return;
            }
        };
        process_job(state, job).await;
    }
}

async fn process_job(state: &AppState, job: LegacyBackfillJob) {
    match run_one(state, &job).await {
        Ok(outcome) => {
            tracing::info!(
                app_user_id = %job.app_user_id,
                inserted = outcome.inserted,
                skipped_unmapped_action = outcome.skipped_unmapped_action,
                skipped_unparseable = outcome.skipped_unparseable,
                sequence_anomalies = outcome.sequence_anomalies,
                "legacy backfill succeeded"
            );
            if let Err(err) = state.db.legacy_backfill_jobs.mark_done(job.id).await {
                tracing::warn!(?err, job_id = %job.id, "failed to mark legacy backfill job done");
            }
            if let Err(err) = state
                .db
                .app_users
                .mark_legacy_backfill_done(job.app_user_id)
                .await
            {
                tracing::warn!(
                    ?err,
                    app_user_id = %job.app_user_id,
                    "failed to set legacy_backfill_done_at"
                );
            }
        }
        Err(err) => {
            let attempts = job.attempts + 1;
            tracing::warn!(
                %err,
                app_user_id = %job.app_user_id,
                attempts,
                "legacy backfill attempt failed"
            );
            let result = if attempts >= MAX_ATTEMPTS {
                state
                    .db
                    .legacy_backfill_jobs
                    .mark_failed(job.id, attempts, &err.to_string())
                    .await
            } else {
                let next_attempt_at =
                    DateTime::from_system_time(SystemTime::now() + backoff_after(attempts));
                state
                    .db
                    .legacy_backfill_jobs
                    .mark_retry(job.id, attempts, next_attempt_at, &err.to_string())
                    .await
            };
            if let Err(db_err) = result {
                tracing::warn!(
                    ?db_err,
                    job_id = %job.id,
                    "failed to update legacy backfill job after failure"
                );
            }
        }
    }
}

async fn run_one(
    state: &AppState,
    job: &LegacyBackfillJob,
) -> Result<provider::BackfillOutcome, LegacyBackfillError> {
    let org = state
        .db
        .orgs
        .find_by_id(job.org_id)
        .await
        .map_err(|e| LegacyBackfillError(format!("failed to load org: {e}")))?
        .ok_or_else(|| LegacyBackfillError("org not found".to_string()))?;
    let cfg = org
        .legacy_backfill()
        .ok_or_else(|| LegacyBackfillError("legacy_backfill config is missing".to_string()))?;

    let app_user = state
        .db
        .app_users
        .find_by_id(job.app_user_id)
        .await
        .map_err(|e| LegacyBackfillError(format!("failed to load app user: {e}")))?
        .ok_or_else(|| LegacyBackfillError("app user not found".to_string()))?;
    let username = app_user
        .username
        .ok_or_else(|| LegacyBackfillError("app user has no username".to_string()))?;

    let secret_key = state
        .config
        .secret_key
        .ok_or_else(|| LegacyBackfillError("BANDAO_SECRET_KEY is not configured".to_string()))?;
    let secret = SecretBox::from_key_bytes(&secret_key);

    provider::run_backfill(
        &state.db,
        &secret,
        &cfg,
        job.org_id,
        job.app_user_id,
        &username,
    )
    .await
}

fn backoff_after(attempts: u32) -> Duration {
    match attempts {
        1 => Duration::from_secs(60),
        2 => Duration::from_secs(5 * 60),
        3 => Duration::from_secs(30 * 60),
        _ => Duration::from_secs(60 * 60),
    }
}
