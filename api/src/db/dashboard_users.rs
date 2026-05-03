use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::error::{ErrorKind, WriteFailure};

use crate::domain::{DashboardUser, Role};
use crate::error::{ApiError, ApiResult};

const MONGO_DUPLICATE_KEY: i32 = 11000;

#[derive(Clone)]
pub struct DashboardUserRepository {
    coll: Collection<DashboardUser>,
}

impl DashboardUserRepository {
    pub fn new(coll: Collection<DashboardUser>) -> Self {
        Self { coll }
    }

    pub async fn create(
        &self,
        id: ObjectId,
        org_id: ObjectId,
        email: &str,
        password_hash: &str,
        role: Role,
    ) -> ApiResult<DashboardUser> {
        let now = DateTime::now();
        let user = DashboardUser {
            id,
            org_id,
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            role,
            created_at: now,
            updated_at: now,
        };
        match self.coll.insert_one(&user).await {
            Ok(_) => Ok(user),
            Err(err) => {
                if is_duplicate_key(&err) {
                    Err(ApiError::EmailTaken)
                } else {
                    Err(ApiError::Db(err))
                }
            }
        }
    }

    pub async fn find_by_email(&self, email: &str) -> ApiResult<Option<DashboardUser>> {
        Ok(self.coll.find_one(doc! { "email": email }).await?)
    }

    pub async fn find_by_id(&self, id: ObjectId) -> ApiResult<Option<DashboardUser>> {
        Ok(self.coll.find_one(doc! { "_id": id }).await?)
    }

    pub async fn list_in_org(&self, org_id: ObjectId) -> ApiResult<Vec<DashboardUser>> {
        let mut cursor = self.coll.find(doc! { "org_id": org_id }).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }

    pub async fn delete_by_id(&self, id: ObjectId) -> ApiResult<()> {
        self.coll.delete_one(doc! { "_id": id }).await?;
        Ok(())
    }

    /// Updates a user's role within the same org. The caller must enforce
    /// the "at least one admin" invariant before invoking this method.
    pub async fn update_role(
        &self,
        user_id: ObjectId,
        org_id: ObjectId,
        new_role: Role,
    ) -> ApiResult<DashboardUser> {
        let now = DateTime::now();
        let role_bson = bson::to_bson(&new_role)?;
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": user_id, "org_id": org_id },
                doc! { "$set": { "role": role_bson, "updated_at": now } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
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
