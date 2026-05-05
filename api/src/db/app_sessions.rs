use std::time::Duration;

use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;

use crate::auth::session_token;
use crate::domain::AppSession;
use crate::error::ApiResult;

#[derive(Clone)]
pub struct AppSessionRepository {
    coll: Collection<AppSession>,
}

impl AppSessionRepository {
    pub fn new(coll: Collection<AppSession>) -> Self {
        Self { coll }
    }

    /// Create a new AppSession for `app_user_id`. Token is opaque random
    /// base64 (≥256 bits), stored in `_id`.
    pub async fn create(&self, app_user_id: ObjectId, ttl: Duration) -> ApiResult<AppSession> {
        let now = DateTime::now();
        let expires_at = DateTime::from_millis(now.timestamp_millis() + ttl.as_millis() as i64);
        let session = AppSession {
            token: session_token::generate(),
            app_user_id,
            expires_at,
            created_at: now,
        };
        self.coll.insert_one(&session).await?;
        Ok(session)
    }

    pub async fn find_by_token(&self, token: &str) -> ApiResult<Option<AppSession>> {
        Ok(self.coll.find_one(doc! { "_id": token }).await?)
    }

    pub async fn delete_by_token(&self, token: &str) -> ApiResult<()> {
        self.coll.delete_one(doc! { "_id": token }).await?;
        Ok(())
    }

    /// Hard delete every session for `app_user_id`. Used by admin
    /// password-reset and `status -> disabled` transitions.
    pub async fn delete_by_app_user(&self, app_user_id: ObjectId) -> ApiResult<u64> {
        let result = self
            .coll
            .delete_many(doc! { "app_user_id": app_user_id })
            .await?;
        Ok(result.deleted_count)
    }

    /// Sliding refresh: extend the session expiry to `now + ttl`.
    pub async fn touch_expires(&self, token: &str, ttl: Duration) -> ApiResult<()> {
        let now = DateTime::now();
        let new_expires = DateTime::from_millis(now.timestamp_millis() + ttl.as_millis() as i64);
        self.coll
            .update_one(
                doc! { "_id": token },
                doc! { "$set": { "expires_at": new_expires } },
            )
            .await?;
        Ok(())
    }
}
