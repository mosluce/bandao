use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::error::{ErrorKind, WriteFailure};

use crate::domain::DashboardUser;
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

    /// Insert a new identity. Org affiliation lives on `dashboard_memberships`,
    /// so the user row only carries email + password_hash + timestamps.
    pub async fn create(
        &self,
        id: ObjectId,
        email: &str,
        password_hash: &str,
    ) -> ApiResult<DashboardUser> {
        let now = DateTime::now();
        let user = DashboardUser {
            id,
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            failed_login_attempts: 0,
            locked_until: None,
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

    pub async fn delete_by_id(&self, id: ObjectId) -> ApiResult<()> {
        self.coll.delete_one(doc! { "_id": id }).await?;
        Ok(())
    }

    /// Atomically increment `failed_login_attempts` and return the new count.
    /// Called on a wrong-password login attempt against an account that is
    /// not currently locked.
    pub async fn record_failed_attempt(&self, id: ObjectId) -> ApiResult<u32> {
        let now = DateTime::now();
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$inc": { "failed_login_attempts": 1i32 }, "$set": { "updated_at": now } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        Ok(result.map(|u| u.failed_login_attempts).unwrap_or(0))
    }

    /// Lock the account until `until`. Called once `failed_login_attempts`
    /// crosses the configured threshold.
    pub async fn set_locked_until(&self, id: ObjectId, until: DateTime) -> ApiResult<()> {
        let now = DateTime::now();
        self.coll
            .update_one(
                doc! { "_id": id },
                doc! { "$set": { "locked_until": until, "updated_at": now } },
            )
            .await?;
        Ok(())
    }

    /// Clear `failed_login_attempts` and `locked_until`. Called on a
    /// successful login and by the admin unlock endpoint.
    pub async fn reset_lockout(&self, id: ObjectId) -> ApiResult<()> {
        let now = DateTime::now();
        self.coll
            .update_one(
                doc! { "_id": id },
                doc! {
                    "$set": { "failed_login_attempts": 0i32, "updated_at": now },
                    "$unset": { "locked_until": "" },
                },
            )
            .await?;
        Ok(())
    }

    /// Used by `POST /auth/reset-password`. Bumps `updated_at` alongside the
    /// hash, same convention as every other mutating repository method here.
    pub async fn update_password_hash(&self, id: ObjectId, password_hash: &str) -> ApiResult<()> {
        self.coll
            .update_one(
                doc! { "_id": id },
                doc! { "$set": { "password_hash": password_hash, "updated_at": DateTime::now() } },
            )
            .await?;
        Ok(())
    }
}

fn is_duplicate_key(err: &mongodb::error::Error) -> bool {
    match err.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(we)) => we.code == MONGO_DUPLICATE_KEY,
        _ => false,
    }
}
