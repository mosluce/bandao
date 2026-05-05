use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::error::{ErrorKind, WriteFailure};

use crate::domain::{AppUser, AppUserStatus};
use crate::error::{ApiError, ApiResult};

const MONGO_DUPLICATE_KEY: i32 = 11000;

/// Returned by `create` when the unique `(org_id, username_lower)` index
/// rejects a duplicate insert. Callers translate this into the user-facing
/// `USERNAME_TAKEN`. Mirrors the `MembershipInsertError::Duplicate` shape.
#[derive(Debug)]
pub enum AppUserInsertError {
    Duplicate,
    Db(mongodb::error::Error),
}

impl From<mongodb::error::Error> for AppUserInsertError {
    fn from(err: mongodb::error::Error) -> Self {
        Self::Db(err)
    }
}

#[derive(Clone)]
pub struct AppUserRepository {
    coll: Collection<AppUser>,
}

impl AppUserRepository {
    pub fn new(coll: Collection<AppUser>) -> Self {
        Self { coll }
    }

    /// Insert a brand-new AppUser. Returns `Duplicate` when the unique
    /// `(org_id, username_lower)` index rejects.
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        org_id: ObjectId,
        username: &str,
        username_lower: &str,
        display_name: &str,
        password_hash: &str,
        created_by_dashboard_user_id: ObjectId,
    ) -> Result<AppUser, AppUserInsertError> {
        let now = DateTime::now();
        let user = AppUser {
            id: ObjectId::new(),
            org_id,
            username: username.to_string(),
            username_lower: username_lower.to_string(),
            display_name: display_name.to_string(),
            password_hash: password_hash.to_string(),
            status: AppUserStatus::Active,
            needs_password_change: true,
            last_login_at: None,
            created_by_dashboard_user_id,
            created_at: now,
            updated_at: now,
        };
        match self.coll.insert_one(&user).await {
            Ok(_) => Ok(user),
            Err(err) => {
                if is_duplicate_key(&err) {
                    Err(AppUserInsertError::Duplicate)
                } else {
                    Err(AppUserInsertError::Db(err))
                }
            }
        }
    }

    pub async fn find_by_id(&self, id: ObjectId) -> ApiResult<Option<AppUser>> {
        Ok(self.coll.find_one(doc! { "_id": id }).await?)
    }

    pub async fn find_by_org_and_username_lower(
        &self,
        org_id: ObjectId,
        username_lower: &str,
    ) -> ApiResult<Option<AppUser>> {
        Ok(self
            .coll
            .find_one(doc! { "org_id": org_id, "username_lower": username_lower })
            .await?)
    }

    pub async fn list_by_org(&self, org_id: ObjectId) -> ApiResult<Vec<AppUser>> {
        let mut cursor = self.coll.find(doc! { "org_id": org_id }).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }

    /// Update `display_name` only. `username`, `org_id`, and other identity
    /// fields are intentionally not settable here.
    pub async fn update_profile(&self, id: ObjectId, display_name: &str) -> ApiResult<AppUser> {
        let now = DateTime::now();
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$set": { "display_name": display_name, "updated_at": now } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    /// Update `status` only. Caller is responsible for the session-cascade
    /// when `disabled` is the new value.
    pub async fn update_status(&self, id: ObjectId, status: AppUserStatus) -> ApiResult<AppUser> {
        let now = DateTime::now();
        let status_bson = bson::to_bson(&status)?;
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$set": { "status": status_bson, "updated_at": now } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    /// Replace `password_hash` and force `needs_password_change = true`.
    /// Used by admin password-reset.
    pub async fn update_password(&self, id: ObjectId, password_hash: &str) -> ApiResult<AppUser> {
        let now = DateTime::now();
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$set": {
                    "password_hash": password_hash,
                    "needs_password_change": true,
                    "updated_at": now,
                } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    /// Replace `password_hash` and clear `needs_password_change`.
    /// Used by `POST /app/me/password` (the AppUser changing their own password).
    pub async fn mark_password_changed(
        &self,
        id: ObjectId,
        password_hash: &str,
    ) -> ApiResult<AppUser> {
        let now = DateTime::now();
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$set": {
                    "password_hash": password_hash,
                    "needs_password_change": false,
                    "updated_at": now,
                } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    /// Bump `last_login_at` to `now`. Called after a successful
    /// `POST /app/auth/login`.
    pub async fn touch_last_login(&self, id: ObjectId) -> ApiResult<()> {
        let now = DateTime::now();
        self.coll
            .update_one(
                doc! { "_id": id },
                doc! { "$set": { "last_login_at": now } },
            )
            .await?;
        Ok(())
    }

    /// Hard delete by id. Used by the `POST /app-users` rollback path when
    /// the post-insert hook (`checkin_user_status` init) fails. Returns the
    /// number of removed rows.
    pub async fn delete_by_id(&self, id: ObjectId) -> ApiResult<u64> {
        let result = self.coll.delete_one(doc! { "_id": id }).await?;
        Ok(result.deleted_count)
    }
}

fn is_duplicate_key(err: &mongodb::error::Error) -> bool {
    match err.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(we)) => we.code == MONGO_DUPLICATE_KEY,
        _ => false,
    }
}
