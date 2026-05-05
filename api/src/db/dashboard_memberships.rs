use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::error::{ErrorKind, WriteFailure};

use crate::domain::{Membership, Role};
use crate::error::{ApiError, ApiResult};

pub const MONGO_DUPLICATE_KEY: i32 = 11000;

/// Returned by `create` when the unique `(user_id, org_id)` index rejects a
/// duplicate insert. Callers translate this into the appropriate user-facing
/// error (e.g. `ALREADY_MEMBER`).
#[derive(Debug)]
pub enum MembershipInsertError {
    Duplicate,
    Db(mongodb::error::Error),
}

impl From<mongodb::error::Error> for MembershipInsertError {
    fn from(err: mongodb::error::Error) -> Self {
        Self::Db(err)
    }
}

#[derive(Clone)]
pub struct MembershipRepository {
    coll: Collection<Membership>,
}

impl MembershipRepository {
    pub fn new(coll: Collection<Membership>) -> Self {
        Self { coll }
    }

    /// Insert a new membership. Returns `Duplicate` when the unique
    /// `(user_id, org_id)` index rejects the insert.
    pub async fn create(
        &self,
        user_id: ObjectId,
        org_id: ObjectId,
        role: Role,
    ) -> Result<Membership, MembershipInsertError> {
        let now = DateTime::now();
        let membership = Membership {
            id: ObjectId::new(),
            user_id,
            org_id,
            role,
            joined_at: now,
            updated_at: now,
        };
        match self.coll.insert_one(&membership).await {
            Ok(_) => Ok(membership),
            Err(err) => {
                if is_duplicate_key(&err) {
                    Err(MembershipInsertError::Duplicate)
                } else {
                    Err(MembershipInsertError::Db(err))
                }
            }
        }
    }

    pub async fn find(&self, id: ObjectId) -> ApiResult<Option<Membership>> {
        Ok(self.coll.find_one(doc! { "_id": id }).await?)
    }

    pub async fn find_by_user_and_org(
        &self,
        user_id: ObjectId,
        org_id: ObjectId,
    ) -> ApiResult<Option<Membership>> {
        Ok(self
            .coll
            .find_one(doc! { "user_id": user_id, "org_id": org_id })
            .await?)
    }

    pub async fn list_by_user(&self, user_id: ObjectId) -> ApiResult<Vec<Membership>> {
        let mut cursor = self.coll.find(doc! { "user_id": user_id }).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }

    pub async fn list_by_org(&self, org_id: ObjectId) -> ApiResult<Vec<Membership>> {
        let mut cursor = self.coll.find(doc! { "org_id": org_id }).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }

    pub async fn update_role(
        &self,
        user_id: ObjectId,
        org_id: ObjectId,
        new_role: Role,
    ) -> ApiResult<Membership> {
        let now = DateTime::now();
        let role_bson = bson::to_bson(&new_role)?;
        let result = self
            .coll
            .find_one_and_update(
                doc! { "user_id": user_id, "org_id": org_id },
                doc! { "$set": { "role": role_bson, "updated_at": now } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    /// Hard-delete a single membership row. Returns the number of documents
    /// removed (0 or 1).
    pub async fn delete(&self, user_id: ObjectId, org_id: ObjectId) -> ApiResult<u64> {
        let result = self
            .coll
            .delete_one(doc! { "user_id": user_id, "org_id": org_id })
            .await?;
        Ok(result.deleted_count)
    }

    /// Hard-delete every membership row for an org. Used by future Org
    /// deletion cascades; not exercised by the current change but listed in
    /// `tasks.md` 1.4 to keep the repo surface complete.
    pub async fn delete_by_org(&self, org_id: ObjectId) -> ApiResult<u64> {
        let result = self.coll.delete_many(doc! { "org_id": org_id }).await?;
        Ok(result.deleted_count)
    }

    pub async fn count_by_user(&self, user_id: ObjectId) -> ApiResult<u64> {
        Ok(self
            .coll
            .count_documents(doc! { "user_id": user_id })
            .await?)
    }
}

fn is_duplicate_key(err: &mongodb::error::Error) -> bool {
    match err.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(we)) => we.code == MONGO_DUPLICATE_KEY,
        _ => false,
    }
}
