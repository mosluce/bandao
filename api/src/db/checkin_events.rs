use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::options::FindOptions;

use crate::domain::{
    CheckinEvent, CheckinEventType, EventInitiatorKind, EventLocation, EventSource,
};
use crate::error::ApiResult;

/// Default page size for cursor pagination on `/app/checkin/events` and
/// `/checkin/users/:id/events`.
pub const DEFAULT_PAGE_SIZE: i64 = 50;

#[derive(Clone)]
pub struct CheckinEventRepository {
    coll: Collection<CheckinEvent>,
}

impl CheckinEventRepository {
    pub fn new(coll: Collection<CheckinEvent>) -> Self {
        Self { coll }
    }

    /// Append-only insert. Caller is expected to have run state-machine,
    /// transfer-toggle, and ordering checks.
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        org_id: ObjectId,
        app_user_id: ObjectId,
        event_type: CheckinEventType,
        occurred_at_client: DateTime,
        occurred_at_server: DateTime,
        source: EventSource,
        initiated_by_kind: EventInitiatorKind,
        initiated_by_id: ObjectId,
        location: EventLocation,
        reason: Option<String>,
    ) -> ApiResult<CheckinEvent> {
        let event = CheckinEvent {
            id: ObjectId::new(),
            org_id,
            app_user_id,
            event_type,
            occurred_at_client,
            occurred_at_server,
            source,
            initiated_by_kind,
            initiated_by_id,
            location,
            reason,
        };
        self.coll.insert_one(&event).await?;
        Ok(event)
    }

    pub async fn find_by_id(&self, id: ObjectId) -> ApiResult<Option<CheckinEvent>> {
        Ok(self.coll.find_one(doc! { "_id": id }).await?)
    }

    /// Latest event by client-time for an AppUser. Used for both the
    /// `OUT_OF_ORDER` check and the "last_event" fields in DTOs.
    pub async fn latest_for_app_user(
        &self,
        app_user_id: ObjectId,
    ) -> ApiResult<Option<CheckinEvent>> {
        let opts = FindOptions::builder()
            .sort(doc! { "occurred_at_client": -1 })
            .limit(1)
            .build();
        let mut cursor = self
            .coll
            .find(doc! { "app_user_id": app_user_id })
            .with_options(opts)
            .await?;
        if cursor.advance().await? {
            Ok(Some(cursor.deserialize_current()?))
        } else {
            Ok(None)
        }
    }

    /// Best-effort delete used for race rollback: when the conditional
    /// status update fails after the event row was already inserted, we
    /// drop the event so the log doesn't accumulate orphans. Errors are
    /// swallowed by the caller (logged at warn).
    pub async fn delete_by_id(&self, id: ObjectId) -> ApiResult<u64> {
        let result = self.coll.delete_one(doc! { "_id": id }).await?;
        Ok(result.deleted_count)
    }

    /// Page an AppUser's own events newest-first by client time. `before` is
    /// the `occurred_at_client` of the last item from the previous page —
    /// strictly older events are returned. Tie-break by `_id` desc keeps
    /// duplicate client times ordered.
    pub async fn list_by_app_user_paginated(
        &self,
        app_user_id: ObjectId,
        before: Option<DateTime>,
        limit: i64,
    ) -> ApiResult<Vec<CheckinEvent>> {
        let mut filter = doc! { "app_user_id": app_user_id };
        if let Some(t) = before {
            filter.insert("occurred_at_client", doc! { "$lt": t });
        }
        let opts = FindOptions::builder()
            .sort(doc! { "occurred_at_client": -1, "_id": -1 })
            .limit(limit.max(1))
            .build();
        let mut cursor = self.coll.find(filter).with_options(opts).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }

    /// Org-wide event listing. Currently unused by the MVP routes but
    /// listed in tasks 1.7 for parity with the other repos.
    #[allow(dead_code)]
    pub async fn list_by_org_paginated(
        &self,
        org_id: ObjectId,
        before: Option<DateTime>,
        limit: i64,
    ) -> ApiResult<Vec<CheckinEvent>> {
        let mut filter = doc! { "org_id": org_id };
        if let Some(t) = before {
            filter.insert("occurred_at_client", doc! { "$lt": t });
        }
        let opts = FindOptions::builder()
            .sort(doc! { "occurred_at_client": -1, "_id": -1 })
            .limit(limit.max(1))
            .build();
        let mut cursor = self.coll.find(filter).with_options(opts).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }

    /// Used by the startup repair task to pull every AppUser's latest event
    /// in one query path. Listed in tasks 1.7 as `list_by_app_user_after`
    /// — kept here for parity though the repair task uses `latest_for_app_user`.
    #[allow(dead_code)]
    pub async fn list_by_app_user_after(
        &self,
        app_user_id: ObjectId,
        after_client_time: DateTime,
    ) -> ApiResult<Vec<CheckinEvent>> {
        let mut cursor = self
            .coll
            .find(doc! {
                "app_user_id": app_user_id,
                "occurred_at_client": { "$gt": after_client_time }
            })
            .await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }
}
