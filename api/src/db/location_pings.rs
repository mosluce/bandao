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

    /// Idempotent insert for the `legacy_backfill` example script. Upserts
    /// keyed on `ping.legacy_source_id` (the partial unique index on that
    /// field is what makes this safe): re-running the script against the
    /// same legacy document is a no-op. Returns `true` when the document was
    /// newly inserted, `false` when it already existed.
    ///
    /// # Panics
    /// Panics if `ping.legacy_source_id` is `None` — this method is only for
    /// legacy-imported rows, never for live-submitted pings.
    pub async fn upsert_legacy(&self, ping: &LocationPing) -> ApiResult<bool> {
        let legacy_source_id = ping
            .legacy_source_id
            .expect("upsert_legacy requires legacy_source_id");
        let to_insert = bson::to_document(ping)?;
        let result = self
            .coll
            .update_one(
                doc! { "legacy_source_id": legacy_source_id },
                doc! { "$setOnInsert": to_insert },
            )
            .upsert(true)
            .await?;
        Ok(result.upserted_id.is_some())
    }

    /// Cursor pagination, newest-first by `occurred_at_client`. `before` is
    /// the boundary timestamp (exclusive) supplied by the client; the
    /// `_id` desc tie-break keeps duplicate client times stable. `from` and
    /// `to` are optional inclusive-from / exclusive-to range bounds and
    /// compose with `before` via AND.
    pub async fn list_by_app_user_paginated(
        &self,
        app_user_id: ObjectId,
        before: Option<DateTime>,
        from: Option<DateTime>,
        to: Option<DateTime>,
        limit: i64,
    ) -> ApiResult<Vec<LocationPing>> {
        let mut filter = doc! { "app_user_id": app_user_id };
        // `before` (cursor) and `to` (range) both express "<", combine to the
        // tighter of the two so mongo gets a single $lt clause.
        let upper = match (before, to) {
            (Some(a), Some(b)) => Some(if a < b { a } else { b }),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
        let mut occurred_clauses = doc! {};
        if let Some(t) = upper {
            occurred_clauses.insert("$lt", t);
        }
        if let Some(t) = from {
            occurred_clauses.insert("$gte", t);
        }
        if !occurred_clauses.is_empty() {
            filter.insert("occurred_at_client", occurred_clauses);
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
