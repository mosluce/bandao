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

    /// Create a new session for `user_id`. `current_org_id` may be `None` for a
    /// zero-Org user; otherwise it should point at one of the user's
    /// memberships at insert time (the middleware verifies it on read).
    pub async fn create(
        &self,
        user_id: ObjectId,
        current_org_id: Option<ObjectId>,
        ttl: Duration,
    ) -> ApiResult<DashboardSession> {
        let now = DateTime::now();
        let expires_at = DateTime::from_millis(now.timestamp_millis() + ttl.as_millis() as i64);
        let session = DashboardSession {
            token: session_token::generate(),
            user_id,
            current_org_id,
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

    /// Hard delete every session belonging to `user_id`, regardless of
    /// `current_org_id`. Reserved for "delete the user identity entirely"
    /// flows; per-org leave / remove uses `delete_by_user_and_org` instead.
    pub async fn delete_all_by_user_id(&self, user_id: ObjectId) -> ApiResult<u64> {
        let result = self.coll.delete_many(doc! { "user_id": user_id }).await?;
        Ok(result.deleted_count)
    }

    /// Force-kick scope: delete only sessions whose `current_org_id` matches
    /// the org the user is leaving / being removed from. Sessions of the same
    /// user that point at other Orgs survive untouched.
    pub async fn delete_by_user_and_org(
        &self,
        user_id: ObjectId,
        org_id: ObjectId,
    ) -> ApiResult<u64> {
        let result = self
            .coll
            .delete_many(doc! { "user_id": user_id, "current_org_id": org_id })
            .await?;
        Ok(result.deleted_count)
    }

    /// Switch the session's active Org. `new_current_org_id = None` is allowed
    /// (e.g. last membership just left).
    pub async fn update_current_org(
        &self,
        token: &str,
        new_current_org_id: Option<ObjectId>,
    ) -> ApiResult<()> {
        let value = match new_current_org_id {
            Some(id) => bson::to_bson(&id)?,
            None => bson::Bson::Null,
        };
        self.coll
            .update_one(
                doc! { "_id": token },
                doc! { "$set": { "current_org_id": value } },
            )
            .await?;
        Ok(())
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
