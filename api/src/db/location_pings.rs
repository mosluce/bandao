use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::error::ErrorKind;
use mongodb::options::FindOptions;

use crate::domain::LocationPing;
use crate::error::ApiResult;

/// Cap on a single batch insert. Mirrors the handler's `INVALID_BATCH` check;
/// duplicated here as a sanity guard so the repo can't be misused.
pub const LOCATION_PING_BATCH_MAX: usize = 100;

/// Result of an `insert_many_unordered` call. Indices are into the input slice
/// passed to the repo, NOT the original handler batch — the handler is
/// responsible for translating these back to caller-visible indices when it
/// has pre-rejected some pings.
#[derive(Debug, Clone)]
pub struct InsertManyOutcome {
    pub inserted_indices: Vec<usize>,
    /// `(index, code)` where `code` is a short identifier suitable for
    /// surfacing to API clients. Currently always `"INSERT_FAILED"` — the
    /// handler's pre-validation catches all the application-meaningful
    /// failure modes; anything reaching this point is a schema /
    /// connectivity surprise.
    pub failed_indices: Vec<(usize, String)>,
}

#[derive(Clone)]
pub struct LocationPingRepository {
    coll: Collection<LocationPing>,
}

impl LocationPingRepository {
    pub fn new(coll: Collection<LocationPing>) -> Self {
        Self { coll }
    }

    /// Bulk insert with `ordered: false` so a single bad row doesn't abort
    /// the whole write. Returns per-index outcomes; callers map back to the
    /// caller-facing batch indices.
    pub async fn insert_many_unordered(
        &self,
        pings: &[LocationPing],
    ) -> ApiResult<InsertManyOutcome> {
        if pings.is_empty() {
            return Ok(InsertManyOutcome {
                inserted_indices: Vec::new(),
                failed_indices: Vec::new(),
            });
        }

        let result = self.coll.insert_many(pings).ordered(false).await;

        match result {
            Ok(_) => Ok(InsertManyOutcome {
                inserted_indices: (0..pings.len()).collect(),
                failed_indices: Vec::new(),
            }),
            Err(err) => {
                if let ErrorKind::InsertMany(im_err) = &*err.kind {
                    let failed_indices: Vec<(usize, String)> = im_err
                        .write_errors
                        .as_ref()
                        .map(|errs| {
                            errs.iter()
                                .map(|w| {
                                    let code = w
                                        .code_name
                                        .clone()
                                        .filter(|s| !s.is_empty())
                                        .unwrap_or_else(|| "INSERT_FAILED".to_string());
                                    (w.index, code)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let failed_set: std::collections::HashSet<usize> =
                        failed_indices.iter().map(|(i, _)| *i).collect();
                    let inserted_indices: Vec<usize> = (0..pings.len())
                        .filter(|i| !failed_set.contains(i))
                        .collect();
                    Ok(InsertManyOutcome {
                        inserted_indices,
                        failed_indices,
                    })
                } else {
                    Err(err.into())
                }
            }
        }
    }

    /// Cursor pagination, newest-first by `occurred_at_client`. `before` is
    /// the boundary timestamp (exclusive) supplied by the client; the
    /// `_id` desc tie-break keeps duplicate client times stable.
    pub async fn list_by_app_user_paginated(
        &self,
        app_user_id: ObjectId,
        before: Option<DateTime>,
        limit: i64,
    ) -> ApiResult<Vec<LocationPing>> {
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

    /// Range query for the xlsx export path. Ascending order so the
    /// resulting spreadsheet reads chronologically top-to-bottom.
    pub async fn list_for_export(
        &self,
        app_user_id: ObjectId,
        from: DateTime,
        to: DateTime,
    ) -> ApiResult<Vec<LocationPing>> {
        let filter = doc! {
            "app_user_id": app_user_id,
            "occurred_at_client": { "$gte": from, "$lte": to },
        };
        let opts = FindOptions::builder()
            .sort(doc! { "occurred_at_client": 1, "_id": 1 })
            .build();
        let mut cursor = self.coll.find(filter).with_options(opts).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }
}
