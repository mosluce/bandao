use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::options::ReturnDocument;

use crate::domain::{LegacyBackfillJob, LegacyBackfillJobStatus};
use crate::error::ApiResult;

const MONGO_DUPLICATE_KEY: i32 = 11000;

/// Returned by `create_pending` when a job for this `app_user_id` already
/// exists (in any status). Not a failure — the login-time enqueue is meant to
/// be idempotent, and a `failed` job in particular must stay `failed` until an
/// admin manually resets it, not get silently reopened by the next login.
#[derive(Debug)]
pub enum LegacyBackfillJobInsertError {
    Duplicate,
    Db(mongodb::error::Error),
}

impl From<mongodb::error::Error> for LegacyBackfillJobInsertError {
    fn from(err: mongodb::error::Error) -> Self {
        Self::Db(err)
    }
}

#[derive(Clone)]
pub struct LegacyBackfillJobRepository {
    coll: Collection<LegacyBackfillJob>,
}

impl LegacyBackfillJobRepository {
    pub fn new(coll: Collection<LegacyBackfillJob>) -> Self {
        Self { coll }
    }

    /// Enqueue a `pending` job for `app_user_id`, ready to run immediately
    /// (`next_attempt_at = now`). No-op (via `Duplicate`) when a job already
    /// exists for this AppUser, regardless of its current status.
    pub async fn create_pending(
        &self,
        org_id: ObjectId,
        app_user_id: ObjectId,
    ) -> Result<(), LegacyBackfillJobInsertError> {
        let now = DateTime::now();
        let job = LegacyBackfillJob {
            id: ObjectId::new(),
            org_id,
            app_user_id,
            status: LegacyBackfillJobStatus::Pending,
            attempts: 0,
            next_attempt_at: now,
            locked_at: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        };
        match self.coll.insert_one(&job).await {
            Ok(_) => Ok(()),
            Err(err) => {
                if is_duplicate_key(&err) {
                    Err(LegacyBackfillJobInsertError::Duplicate)
                } else {
                    Err(LegacyBackfillJobInsertError::Db(err))
                }
            }
        }
    }

    /// Atomically claim one due `pending` job (`next_attempt_at <= now`),
    /// flipping it to `active`. Mirrors the conditional-update idiom used by
    /// `checkin_user_status::update_to` — concurrent workers cannot claim the
    /// same job.
    pub async fn claim_due(&self, now: DateTime) -> ApiResult<Option<LegacyBackfillJob>> {
        let pending = bson::to_bson(&LegacyBackfillJobStatus::Pending)?;
        let active = bson::to_bson(&LegacyBackfillJobStatus::Active)?;
        let result = self
            .coll
            .find_one_and_update(
                doc! { "status": pending, "next_attempt_at": { "$lte": now } },
                doc! { "$set": { "status": active, "locked_at": now, "updated_at": now } },
            )
            .return_document(ReturnDocument::After)
            .await?;
        Ok(result)
    }

    pub async fn mark_done(&self, id: ObjectId) -> ApiResult<()> {
        self.coll
            .update_one(
                doc! { "_id": id },
                doc! { "$set": { "status": "done", "updated_at": DateTime::now() } },
            )
            .await?;
        Ok(())
    }

    /// Failed attempt that hasn't hit the retry cap yet: back to `pending`
    /// with a later `next_attempt_at`, independent of the AppUser logging in
    /// again.
    pub async fn mark_retry(
        &self,
        id: ObjectId,
        attempts: u32,
        next_attempt_at: DateTime,
        last_error: &str,
    ) -> ApiResult<()> {
        self.coll
            .update_one(
                doc! { "_id": id },
                doc! {
                    "$set": {
                        "status": "pending",
                        "attempts": attempts,
                        "next_attempt_at": next_attempt_at,
                        "last_error": last_error,
                        "updated_at": DateTime::now(),
                    },
                    "$unset": { "locked_at": "" },
                },
            )
            .await?;
        Ok(())
    }

    /// Retry cap exceeded: terminal `failed` state, no further automatic
    /// retries. Needs manual intervention (see design D9/D11).
    pub async fn mark_failed(
        &self,
        id: ObjectId,
        attempts: u32,
        last_error: &str,
    ) -> ApiResult<()> {
        self.coll
            .update_one(
                doc! { "_id": id },
                doc! {
                    "$set": {
                        "status": "failed",
                        "attempts": attempts,
                        "last_error": last_error,
                        "updated_at": DateTime::now(),
                    },
                },
            )
            .await?;
        Ok(())
    }

    /// Reset jobs abandoned by a crashed/restarted worker (`active` past the
    /// staleness threshold) back to `pending`, immediately eligible. Returns
    /// the count recovered, for logging.
    pub async fn recover_stale(&self, locked_before: DateTime) -> ApiResult<u64> {
        let active = bson::to_bson(&LegacyBackfillJobStatus::Active)?;
        let result = self
            .coll
            .update_many(
                doc! { "status": active, "locked_at": { "$lt": locked_before } },
                doc! { "$set": { "status": "pending", "next_attempt_at": DateTime::now() } },
            )
            .await?;
        Ok(result.modified_count)
    }

    pub async fn list_by_org(&self, org_id: ObjectId) -> ApiResult<Vec<LegacyBackfillJob>> {
        let mut cursor = self.coll.find(doc! { "org_id": org_id }).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }
}

fn is_duplicate_key(err: &mongodb::error::Error) -> bool {
    match err.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(we)) => we.code == MONGO_DUPLICATE_KEY,
        _ => false,
    }
}
