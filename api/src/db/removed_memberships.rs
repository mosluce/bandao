use std::time::Duration;

use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;

use crate::domain::{RemovalKind, RemovedMembership};
use crate::error::ApiResult;

pub const COOLDOWN: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Clone)]
pub struct RemovedMembershipRepository {
    coll: Collection<RemovedMembership>,
}

impl RemovedMembershipRepository {
    pub fn new(coll: Collection<RemovedMembership>) -> Self {
        Self { coll }
    }

    /// Insert a marker. `email` MUST already be lowercased by the caller — the
    /// unique index is exact-match, so casing inconsistency would silently allow
    /// duplicates.
    pub async fn insert(
        &self,
        org_id: ObjectId,
        email: &str,
        kind: RemovalKind,
    ) -> ApiResult<RemovedMembership> {
        let now = DateTime::now();
        let cooldown_until =
            DateTime::from_millis(now.timestamp_millis() + COOLDOWN.as_millis() as i64);
        let doc = RemovedMembership {
            id: ObjectId::new(),
            org_id,
            email: email.to_string(),
            removed_at: now,
            cooldown_until,
            removal_kind: kind,
        };
        self.coll.insert_one(&doc).await?;
        Ok(doc)
    }

    pub async fn find(
        &self,
        org_id: ObjectId,
        email: &str,
    ) -> ApiResult<Option<RemovedMembership>> {
        Ok(self
            .coll
            .find_one(doc! { "org_id": org_id, "email": email })
            .await?)
    }

    pub async fn delete(&self, org_id: ObjectId, email: &str) -> ApiResult<()> {
        self.coll
            .delete_one(doc! { "org_id": org_id, "email": email })
            .await?;
        Ok(())
    }

    pub async fn list_for_org(&self, org_id: ObjectId) -> ApiResult<Vec<RemovedMembership>> {
        let mut cursor = self.coll.find(doc! { "org_id": org_id }).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }
}
