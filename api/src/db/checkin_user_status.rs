use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::options::ReturnDocument;

use crate::domain::{AppUserCheckinStatus, CheckinUserStatus};
use crate::error::ApiResult;

const MONGO_DUPLICATE_KEY: i32 = 11000;

/// Differentiated insert error for `init_off_duty`. Duplicates are not a
/// failure mode — they just mean the row already exists. Callers translate
/// this into "no-op, already initialised".
#[derive(Debug)]
pub enum CheckinStatusInsertError {
    Duplicate,
    Db(mongodb::error::Error),
}

impl From<mongodb::error::Error> for CheckinStatusInsertError {
    fn from(err: mongodb::error::Error) -> Self {
        Self::Db(err)
    }
}

#[derive(Clone)]
pub struct CheckinUserStatusRepository {
    coll: Collection<CheckinUserStatus>,
}

impl CheckinUserStatusRepository {
    pub fn new(coll: Collection<CheckinUserStatus>) -> Self {
        Self { coll }
    }

    /// Insert a fresh `off_duty` row for a brand-new AppUser. Returns
    /// `Duplicate` when the row already exists — which the AppUser-create
    /// flow translates into "fine, repeat call from a retry".
    pub async fn init_off_duty(
        &self,
        app_user_id: ObjectId,
        org_id: ObjectId,
    ) -> Result<CheckinUserStatus, CheckinStatusInsertError> {
        let now = DateTime::now();
        let row = CheckinUserStatus {
            app_user_id,
            org_id,
            status: AppUserCheckinStatus::OffDuty,
            current_shift_started_at: None,
            last_event_id: None,
            updated_at: now,
        };
        match self.coll.insert_one(&row).await {
            Ok(_) => Ok(row),
            Err(err) => {
                if is_duplicate_key(&err) {
                    Err(CheckinStatusInsertError::Duplicate)
                } else {
                    Err(CheckinStatusInsertError::Db(err))
                }
            }
        }
    }

    /// Best-effort delete used to roll back AppUser create failures. Returns
    /// the count for tests; production callers ignore the result.
    pub async fn delete_by_app_user(&self, app_user_id: ObjectId) -> ApiResult<u64> {
        let result = self.coll.delete_one(doc! { "_id": app_user_id }).await?;
        Ok(result.deleted_count)
    }

    pub async fn find(&self, app_user_id: ObjectId) -> ApiResult<Option<CheckinUserStatus>> {
        Ok(self.coll.find_one(doc! { "_id": app_user_id }).await?)
    }

    pub async fn list_by_org(&self, org_id: ObjectId) -> ApiResult<Vec<CheckinUserStatus>> {
        let mut cursor = self.coll.find(doc! { "org_id": org_id }).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }

    /// Count AppUsers in `org_id` whose status is anything other than
    /// `off_duty`. Used by the state-lock guard on `PATCH /orgs/me/settings`.
    pub async fn count_on_duty_in_org(&self, org_id: ObjectId) -> ApiResult<u64> {
        let off = bson::to_bson(&AppUserCheckinStatus::OffDuty)?;
        Ok(self
            .coll
            .count_documents(doc! { "org_id": org_id, "status": { "$ne": off } })
            .await?)
    }

    /// Conditional state transition. Returns `None` when the prior status
    /// doesn't match (race lost) so the caller can roll back the event row
    /// and emit `INVALID_TRANSITION`. Sets `current_shift_started_at` and
    /// `last_event_id` per the caller's plan.
    pub async fn update_to(
        &self,
        app_user_id: ObjectId,
        expected_prior: AppUserCheckinStatus,
        new_status: AppUserCheckinStatus,
        current_shift_started_at: Option<DateTime>,
        last_event_id: ObjectId,
    ) -> ApiResult<Option<CheckinUserStatus>> {
        let now = DateTime::now();
        let prior_bson = bson::to_bson(&expected_prior)?;
        let new_bson = bson::to_bson(&new_status)?;

        let mut set = doc! {
            "status": new_bson,
            "last_event_id": last_event_id,
            "updated_at": now,
        };
        // current_shift_started_at: explicit `null` for off_duty (clear shift),
        // explicit timestamp for on_site-from-off_duty (start shift), and a
        // marker indicating "no change" for transfer events.
        match current_shift_started_at {
            Some(started_at) => {
                set.insert("current_shift_started_at", started_at);
            }
            None if matches!(new_status, AppUserCheckinStatus::OffDuty) => {
                set.insert("current_shift_started_at", bson::Bson::Null);
            }
            None => {
                // No-op: keep whatever's there. We do this by not setting the
                // key — Mongo `$set` only touches listed fields.
            }
        }

        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": app_user_id, "status": prior_bson },
                doc! { "$set": set },
            )
            .return_document(ReturnDocument::After)
            .await?;
        Ok(result)
    }
}

fn is_duplicate_key(err: &mongodb::error::Error) -> bool {
    match err.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(we)) => we.code == MONGO_DUPLICATE_KEY,
        _ => false,
    }
}
