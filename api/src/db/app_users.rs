use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::error::{ErrorKind, WriteFailure};

use crate::domain::{AppUser, AppUserAuthSource, AppUserStatus};
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

/// Result of one `sync_upsert_shadow` call — whether the row was newly
/// created or an existing one was refreshed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncUpsertOutcome {
    Created,
    Updated,
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
            username: Some(username.to_string()),
            username_lower: Some(username_lower.to_string()),
            display_name: display_name.to_string(),
            password_hash: Some(password_hash.to_string()),
            auth_source: AppUserAuthSource::Internal,
            external_key: None,
            status: AppUserStatus::Active,
            needs_password_change: true,
            failed_login_attempts: 0,
            locked_until: None,
            last_login_at: None,
            created_by_dashboard_user_id: Some(created_by_dashboard_user_id),
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

    /// Look up an external shadow user by `(org_id, external_key)` and refresh
    /// its `display_name` / `last_login_at`, or create it on first login. The
    /// resulting row carries `auth_source = External`, no `username` /
    /// `password_hash`, and `needs_password_change = false`. Concurrent first
    /// logins race on the unique `(org_id, external_key)` index; on a duplicate
    /// insert we fall back to a refreshing update so both callers succeed.
    pub async fn upsert_shadow(
        &self,
        org_id: ObjectId,
        external_key: &str,
        display_name: &str,
    ) -> ApiResult<AppUser> {
        let now = DateTime::now();
        // Refresh-or-nothing first: the common case (repeat login) is a plain
        // update that also bumps last_login_at.
        let refreshed = self
            .coll
            .find_one_and_update(
                doc! { "org_id": org_id, "external_key": external_key },
                doc! { "$set": {
                    "display_name": display_name,
                    "last_login_at": now,
                    "updated_at": now,
                } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        if let Some(user) = refreshed {
            return Ok(user);
        }

        // First login: insert a fresh shadow row.
        let user = AppUser {
            id: ObjectId::new(),
            org_id,
            username: None,
            username_lower: None,
            display_name: display_name.to_string(),
            password_hash: None,
            auth_source: AppUserAuthSource::External,
            external_key: Some(external_key.to_string()),
            status: AppUserStatus::Active,
            needs_password_change: false,
            failed_login_attempts: 0,
            locked_until: None,
            last_login_at: Some(now),
            created_by_dashboard_user_id: None,
            created_at: now,
            updated_at: now,
        };
        match self.coll.insert_one(&user).await {
            Ok(_) => Ok(user),
            Err(err) if is_duplicate_key(&err) => {
                // Lost the first-login race — another request just created the
                // row. Re-run the refreshing update, which now hits it.
                let user = self
                    .coll
                    .find_one_and_update(
                        doc! { "org_id": org_id, "external_key": external_key },
                        doc! { "$set": {
                            "display_name": display_name,
                            "last_login_at": now,
                            "updated_at": now,
                        } },
                    )
                    .return_document(mongodb::options::ReturnDocument::After)
                    .await?;
                user.ok_or(ApiError::NotFound)
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Bulk-oriented counterpart to `upsert_shadow`, used by `POST
    /// /orgs/me/external-auth/sync`. Same identity semantics (external
    /// shadow user keyed by `(org_id, external_key)`), but deliberately does
    /// NOT touch `last_login_at` — a synced-but-never-logged-in user must
    /// not look like they just logged in.
    pub async fn sync_upsert_shadow(
        &self,
        org_id: ObjectId,
        external_key: &str,
        display_name: &str,
    ) -> ApiResult<SyncUpsertOutcome> {
        let now = DateTime::now();
        let refreshed = self
            .coll
            .find_one_and_update(
                doc! { "org_id": org_id, "external_key": external_key },
                doc! { "$set": { "display_name": display_name, "updated_at": now } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        if refreshed.is_some() {
            return Ok(SyncUpsertOutcome::Updated);
        }

        let user = AppUser {
            id: ObjectId::new(),
            org_id,
            username: None,
            username_lower: None,
            display_name: display_name.to_string(),
            password_hash: None,
            auth_source: AppUserAuthSource::External,
            external_key: Some(external_key.to_string()),
            status: AppUserStatus::Active,
            needs_password_change: false,
            failed_login_attempts: 0,
            locked_until: None,
            last_login_at: None,
            created_by_dashboard_user_id: None,
            created_at: now,
            updated_at: now,
        };
        match self.coll.insert_one(&user).await {
            Ok(_) => Ok(SyncUpsertOutcome::Created),
            // Lost a race against a concurrent sync/login — the row exists
            // now either way, treat it as an update rather than erroring.
            Err(err) if is_duplicate_key(&err) => {
                self.coll
                    .find_one_and_update(
                        doc! { "org_id": org_id, "external_key": external_key },
                        doc! { "$set": { "display_name": display_name, "updated_at": now } },
                    )
                    .await?;
                Ok(SyncUpsertOutcome::Updated)
            }
            Err(err) => Err(err.into()),
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

    pub async fn find_by_org_and_external_key(
        &self,
        org_id: ObjectId,
        external_key: &str,
    ) -> ApiResult<Option<AppUser>> {
        Ok(self
            .coll
            .find_one(doc! { "org_id": org_id, "external_key": external_key })
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

    /// Atomically increment `failed_login_attempts` and return the new
    /// count. Called on a wrong-password login attempt against an internal
    /// AppUser that is not currently locked.
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
}

fn is_duplicate_key(err: &mongodb::error::Error) -> bool {
    match err.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(we)) => we.code == MONGO_DUPLICATE_KEY,
        _ => false,
    }
}
