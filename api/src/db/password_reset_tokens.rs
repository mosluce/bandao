use std::time::Duration;

use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;

use crate::domain::PasswordResetToken;
use crate::error::ApiResult;

#[derive(Clone)]
pub struct PasswordResetTokenRepository {
    coll: Collection<PasswordResetToken>,
}

impl PasswordResetTokenRepository {
    pub fn new(coll: Collection<PasswordResetToken>) -> Self {
        Self { coll }
    }

    pub async fn insert(
        &self,
        user_id: ObjectId,
        token_hash: &str,
        ttl: Duration,
    ) -> ApiResult<PasswordResetToken> {
        let now = DateTime::now();
        let expires_at = DateTime::from_millis(now.timestamp_millis() + ttl.as_millis() as i64);
        let token = PasswordResetToken {
            id: ObjectId::new(),
            user_id,
            token_hash: token_hash.to_string(),
            expires_at,
            used_at: None,
            created_at: now,
        };
        self.coll.insert_one(&token).await?;
        Ok(token)
    }

    /// Auth-path lookup: matched by hash alone, same pattern as
    /// `org_api_tokens::find_active_by_hash` — the hash determines identity,
    /// used/expired checks happen in the caller.
    pub async fn find_by_hash(&self, token_hash: &str) -> ApiResult<Option<PasswordResetToken>> {
        Ok(self
            .coll
            .find_one(doc! { "token_hash": token_hash })
            .await?)
    }

    /// Most recently issued token for `user_id`, regardless of used/expired
    /// state. Backs the `POST /auth/forgot-password` cooldown check: the
    /// caller compares this row's `created_at` against the cooldown window.
    pub async fn find_latest_for_user(
        &self,
        user_id: ObjectId,
    ) -> ApiResult<Option<PasswordResetToken>> {
        Ok(self
            .coll
            .find_one(doc! { "user_id": user_id })
            .sort(doc! { "created_at": -1 })
            .await?)
    }

    pub async fn mark_used(&self, id: ObjectId) -> ApiResult<()> {
        self.coll
            .update_one(
                doc! { "_id": id },
                doc! { "$set": { "used_at": DateTime::now() } },
            )
            .await?;
        Ok(())
    }
}
