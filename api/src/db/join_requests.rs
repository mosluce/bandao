use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::options::{FindOptions, ReturnDocument};

use crate::domain::{JoinRequest, JoinRequestStatus};
use crate::error::{ApiError, ApiResult};

pub const MONGO_DUPLICATE_KEY: i32 = 11000;

/// Returned by `insert_pending` when the `(org_id, user_id)` partial unique
/// index (covering only `status=pending`) rejects the insert. Callers turn
/// this into `JOIN_REQUEST_PENDING`.
#[derive(Debug)]
pub enum JoinRequestInsertError {
    Duplicate,
    Db(mongodb::error::Error),
}

impl From<mongodb::error::Error> for JoinRequestInsertError {
    fn from(err: mongodb::error::Error) -> Self {
        Self::Db(err)
    }
}

#[derive(Clone)]
pub struct JoinRequestRepository {
    coll: Collection<JoinRequest>,
}

impl JoinRequestRepository {
    pub fn new(coll: Collection<JoinRequest>) -> Self {
        Self { coll }
    }

    pub async fn insert_pending(
        &self,
        user_id: ObjectId,
        org_id: ObjectId,
        application_message: Option<String>,
    ) -> Result<JoinRequest, JoinRequestInsertError> {
        let now = DateTime::now();
        let row = JoinRequest {
            id: ObjectId::new(),
            user_id,
            org_id,
            status: JoinRequestStatus::Pending,
            application_message,
            rejection_reason: None,
            requested_at: now,
            decided_at: None,
            decided_by: None,
        };
        match self.coll.insert_one(&row).await {
            Ok(_) => Ok(row),
            Err(err) => {
                if is_duplicate_key(&err) {
                    Err(JoinRequestInsertError::Duplicate)
                } else {
                    Err(JoinRequestInsertError::Db(err))
                }
            }
        }
    }

    pub async fn find_by_id(&self, id: ObjectId) -> ApiResult<Option<JoinRequest>> {
        Ok(self.coll.find_one(doc! { "_id": id }).await?)
    }

    pub async fn find_pending_by_user_and_org(
        &self,
        user_id: ObjectId,
        org_id: ObjectId,
    ) -> ApiResult<Option<JoinRequest>> {
        Ok(self
            .coll
            .find_one(doc! {
                "user_id": user_id,
                "org_id": org_id,
                "status": "pending",
            })
            .await?)
    }

    pub async fn list_by_org_with_status(
        &self,
        org_id: ObjectId,
        status: JoinRequestStatus,
    ) -> ApiResult<Vec<JoinRequest>> {
        let status_bson = bson::to_bson(&status)?;
        let opts = FindOptions::builder()
            .sort(doc! { "requested_at": -1 })
            .build();
        let mut cursor = self
            .coll
            .find(doc! { "org_id": org_id, "status": status_bson })
            .with_options(opts)
            .await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }

    pub async fn list_by_user(&self, user_id: ObjectId) -> ApiResult<Vec<JoinRequest>> {
        let opts = FindOptions::builder()
            .sort(doc! { "requested_at": -1 })
            .build();
        let mut cursor = self
            .coll
            .find(doc! { "user_id": user_id })
            .with_options(opts)
            .await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }

    pub async fn count_pending_by_org(&self, org_id: ObjectId) -> ApiResult<u64> {
        Ok(self
            .coll
            .count_documents(doc! { "org_id": org_id, "status": "pending" })
            .await?)
    }

    /// Move a pending row to `approved` or `rejected`. Returns the updated
    /// row. `NotFound` if the row is missing OR not in `pending` (caller
    /// translates that to `400 INVALID_STATE` based on prior find).
    pub async fn decide(
        &self,
        id: ObjectId,
        new_status: JoinRequestStatus,
        decided_by: ObjectId,
        rejection_reason: Option<String>,
    ) -> ApiResult<JoinRequest> {
        let status_bson = bson::to_bson(&new_status)?;
        let now = DateTime::now();
        let mut set = doc! {
            "status": status_bson,
            "decided_at": now,
            "decided_by": decided_by,
        };
        if let Some(reason) = rejection_reason {
            set.insert("rejection_reason", reason);
        }
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id, "status": "pending" },
                doc! { "$set": set },
            )
            .return_document(ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    /// Cancel by the caller. Verifies ownership AND pending status in the
    /// filter so non-owners or already-decided rows return `NotFound` for the
    /// caller to translate into 404 / 400.
    pub async fn cancel_by_owner(&self, id: ObjectId, user_id: ObjectId) -> ApiResult<JoinRequest> {
        let now = DateTime::now();
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id, "user_id": user_id, "status": "pending" },
                doc! {
                    "$set": {
                        "status": "cancelled",
                        "decided_at": now,
                    },
                },
            )
            .return_document(ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }
}

fn is_duplicate_key(err: &mongodb::error::Error) -> bool {
    match err.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(we)) => we.code == MONGO_DUPLICATE_KEY,
        _ => false,
    }
}
