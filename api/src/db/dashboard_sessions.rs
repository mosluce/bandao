use std::time::Duration;

use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;

use crate::auth::session_token;
use crate::domain::DashboardSession;
use crate::error::ApiResult;

#[derive(Clone)]
pub struct DashboardSessionRepository {
    coll: Collection<DashboardSession>,
}

impl DashboardSessionRepository {
    pub fn new(coll: Collection<DashboardSession>) -> Self {
        Self { coll }
    }

    pub async fn create(
        &self,
        user_id: ObjectId,
        org_id: ObjectId,
        ttl: Duration,
    ) -> ApiResult<DashboardSession> {
        let now = DateTime::now();
        let expires_at = DateTime::from_millis(now.timestamp_millis() + ttl.as_millis() as i64);
        let session = DashboardSession {
            token: session_token::generate(),
            user_id,
            org_id,
            expires_at,
            created_at: now,
        };
        self.coll.insert_one(&session).await?;
        Ok(session)
    }

    pub async fn find_by_token(&self, token: &str) -> ApiResult<Option<DashboardSession>> {
        Ok(self.coll.find_one(doc! { "_id": token }).await?)
    }

    pub async fn delete_by_token(&self, token: &str) -> ApiResult<()> {
        self.coll.delete_one(doc! { "_id": token }).await?;
        Ok(())
    }

    pub async fn delete_all_by_user_id(&self, user_id: ObjectId) -> ApiResult<u64> {
        let result = self.coll.delete_many(doc! { "user_id": user_id }).await?;
        Ok(result.deleted_count)
    }

    /// Extend the session's expiry to `now + ttl`. Skipped if the resulting
    /// expiry would be earlier than the existing one (defensive no-op).
    pub async fn touch_expires(&self, token: &str, ttl: Duration) -> ApiResult<()> {
        let now = DateTime::now();
        let new_expires =
            DateTime::from_millis(now.timestamp_millis() + ttl.as_millis() as i64);
        self.coll
            .update_one(
                doc! { "_id": token },
                doc! { "$set": { "expires_at": new_expires } },
            )
            .await?;
        Ok(())
    }
}
