//! Connects to a customer's legacy MongoDB, fetches one AppUser's historical
//! check-in documents by identity field, maps them into `CheckinEvent` rows
//! per the Org's declarative `LegacyBackfillConfig`, and inserts them —
//! bypassing the live `/app/checkin/events` state-machine/ordering validators
//! (see design D7: this is an offline/background write, not a live user
//! action; historical data is kept for record-keeping, not re-validated
//! against today's business rules).
//!
//! No customer-specific code lives here — every quirk (field names, the
//! action vocabulary) is data on `LegacyBackfillConfig`, so a different
//! customer's differently-shaped legacy system needs only a different config,
//! never a code change.

use std::time::Duration;

use bson::oid::ObjectId;
use bson::{Bson, DateTime};
use mongodb::options::ClientOptions;
use mongodb::{Client, bson::doc};

use crate::auth::secret_box::SecretBox;
use crate::db::Db;
use crate::domain::{
    AppUserCheckinStatus, CheckinEventType, EventInitiatorKind, EventLocation, EventSource,
    GeoPoint, LegacyBackfillConfig,
};
use crate::handlers::app_checkin::{MANUAL_LABEL_MAX, MANUAL_LABEL_MIN};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Diagnostic for a failed backfill attempt. Stored verbatim as a job's
/// `last_error` — this is an internal background-worker error, not an
/// HTTP-facing `ApiError` (mirrors `startup::repair_one`, which for the same
/// reason also avoids `ApiError`).
#[derive(Debug, Clone)]
pub struct LegacyBackfillError(pub String);

impl std::fmt::Display for LegacyBackfillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Validate the admin-supplied field-mapping shape before saving. Does NOT
/// touch the network — connectivity is checked via the preview endpoint.
pub fn validate_config(
    identity_field: &str,
    timestamp_field: &str,
    lat_field: &str,
    lng_field: &str,
    action_field: &str,
) -> Result<(), String> {
    if identity_field.trim().is_empty() {
        return Err("identity_field must not be empty".to_string());
    }
    if timestamp_field.trim().is_empty() {
        return Err("timestamp_field must not be empty".to_string());
    }
    if lat_field.trim().is_empty() {
        return Err("lat_field must not be empty".to_string());
    }
    if lng_field.trim().is_empty() {
        return Err("lng_field must not be empty".to_string());
    }
    if action_field.trim().is_empty() {
        return Err("action_field must not be empty".to_string());
    }
    Ok(())
}

/// One legacy document mapped into our shape, ready to become a
/// `CheckinEvent`. Not yet inserted. Also reused as the preview endpoint's
/// sample shape (`pub` fields, no duplicate struct needed) via
/// `PreviewOutcome::sample`.
pub struct MappedEvent {
    pub event_type: CheckinEventType,
    pub occurred_at_client: DateTime,
    pub lat: f64,
    pub lng: f64,
    pub region_name: Option<String>,
    pub manual_label: Option<String>,
}

/// Tallies from one backfill attempt, for logging. Not persisted structurally
/// — folded into a human-readable summary on success.
#[derive(Debug, Default)]
pub struct BackfillOutcome {
    pub inserted: usize,
    pub skipped_unmapped_action: usize,
    pub skipped_unparseable: usize,
    pub sequence_anomalies: usize,
}

/// Read a dot-path (e.g. `"signer.username"`) out of a raw BSON document.
/// `bson::Document` has no built-in nested-path accessor (dot-paths are a
/// Mongo *query* concept, not a document-reading one), so this walks it by
/// hand.
fn get_by_path<'a>(doc: &'a bson::Document, path: &str) -> Option<&'a Bson> {
    let mut parts = path.split('.');
    let mut current = doc.get(parts.next()?)?;
    for part in parts {
        current = current.as_document()?.get(part)?;
    }
    Some(current)
}

fn as_str_path(doc: &bson::Document, path: &str) -> Option<String> {
    match get_by_path(doc, path)? {
        Bson::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn as_f64_path(doc: &bson::Document, path: &str) -> Option<f64> {
    match get_by_path(doc, path)? {
        Bson::Double(v) => Some(*v),
        Bson::Int32(v) => Some(*v as f64),
        Bson::Int64(v) => Some(*v as f64),
        _ => None,
    }
}

/// Legacy timestamps are trusted as-is (design D4 — no cross-check, no
/// AM/PM-style correction). Accepts either a native BSON date or an RFC3339
/// string, since sloppier legacy systems sometimes store dates as text.
fn as_datetime_path(doc: &bson::Document, path: &str) -> Option<DateTime> {
    match get_by_path(doc, path)? {
        Bson::DateTime(dt) => Some(*dt),
        Bson::String(s) => DateTime::parse_rfc3339_str(s).ok(),
        _ => None,
    }
}

/// Connect (read-only) to the configured legacy database and fetch + map the
/// documents matching `username` on `identity_field`. Building the query
/// filter directly from `identity_field` works even though it's a dot-path
/// string — MongoDB's server-side query matcher interprets dots as nested
/// fields natively, unlike reading a document client-side (see
/// `get_by_path`).
async fn fetch_and_map(
    cfg: &LegacyBackfillConfig,
    connection_string: &str,
    username: &str,
    limit: Option<i64>,
) -> Result<(Vec<MappedEvent>, BackfillOutcome), LegacyBackfillError> {
    let mut options = ClientOptions::parse(connection_string)
        .await
        .map_err(|e| LegacyBackfillError(format!("invalid connection string: {e}")))?;
    options.connect_timeout = Some(CONNECT_TIMEOUT);
    options.server_selection_timeout = Some(CONNECT_TIMEOUT);
    let client = Client::with_options(options)
        .map_err(|e| LegacyBackfillError(format!("cannot construct client: {e}")))?;

    let coll: mongodb::Collection<bson::Document> =
        client.database(&cfg.database).collection(&cfg.collection);

    let mut find = coll.find(doc! { cfg.identity_field.as_str(): username });
    if let Some(n) = limit {
        find = find.limit(n);
    }
    let mut cursor = find
        .await
        .map_err(|e| LegacyBackfillError(format!("query failed: {e}")))?;

    let mut mapped = Vec::new();
    let mut outcome = BackfillOutcome::default();

    loop {
        let advanced = cursor
            .advance()
            .await
            .map_err(|e| LegacyBackfillError(format!("cursor read failed: {e}")))?;
        if !advanced {
            break;
        }
        let raw = match cursor.deserialize_current() {
            Ok(d) => d,
            Err(_) => {
                outcome.skipped_unparseable += 1;
                continue;
            }
        };

        let Some(action) = as_str_path(&raw, &cfg.action_field) else {
            outcome.skipped_unparseable += 1;
            continue;
        };
        let Some(&event_type) = cfg.action_map.get(&action) else {
            outcome.skipped_unmapped_action += 1;
            continue;
        };
        let Some(occurred_at_client) = as_datetime_path(&raw, &cfg.timestamp_field) else {
            outcome.skipped_unparseable += 1;
            continue;
        };
        let (Some(lat), Some(lng)) = (
            as_f64_path(&raw, &cfg.lat_field),
            as_f64_path(&raw, &cfg.lng_field),
        ) else {
            outcome.skipped_unparseable += 1;
            continue;
        };
        let region_name = cfg
            .region_name_field
            .as_deref()
            .and_then(|f| as_str_path(&raw, f));
        let manual_label = cfg
            .manual_label_field
            .as_deref()
            .and_then(|f| as_str_path(&raw, f))
            .map(|s| s.trim().to_string())
            .filter(|s| {
                let len = s.chars().count();
                (MANUAL_LABEL_MIN..=MANUAL_LABEL_MAX).contains(&len)
            });

        mapped.push(MappedEvent {
            event_type,
            occurred_at_client,
            lat,
            lng,
            region_name,
            manual_label,
        });
    }

    Ok((mapped, outcome))
}

/// Run one AppUser's full backfill: fetch + map, insert (bypassing the live
/// validators, logging sequence anomalies rather than blocking on them per
/// D7), then derive `checkin_user_status` by reusing the startup drift-repair
/// logic (D8) — the post-backfill situation (events exist, no status row)
/// lands in that function's already-handled `(None, Some(latest))` branch.
pub async fn run_backfill(
    db: &Db,
    secret: &SecretBox,
    cfg: &LegacyBackfillConfig,
    org_id: ObjectId,
    app_user_id: ObjectId,
    username: &str,
) -> Result<BackfillOutcome, LegacyBackfillError> {
    let connection_string = secret
        .decrypt(&cfg.connection_string_encrypted)
        .map_err(|_| {
            LegacyBackfillError("stored connection string could not be decrypted".to_string())
        })?;

    let (mut mapped, mut outcome) = fetch_and_map(cfg, &connection_string, username, None).await?;
    mapped.sort_by_key(|e| e.occurred_at_client);

    // Sequence-anomaly check: log only, never block the insert (D7). Historical
    // data is for record-keeping, not re-validated against today's live rules.
    let mut implied = AppUserCheckinStatus::OffDuty;
    for event in &mapped {
        match event.event_type.next_status(implied) {
            Some(next) => implied = next,
            None => outcome.sequence_anomalies += 1,
        }
    }

    for event in &mapped {
        let location = EventLocation {
            coordinates: GeoPoint {
                lat: event.lat,
                lng: event.lng,
            },
            accuracy_meters: None,
            region_name: event.region_name.clone(),
            manual_label: event.manual_label.clone(),
        };
        db.checkin_events
            .create(
                org_id,
                app_user_id,
                event.event_type,
                event.occurred_at_client,
                DateTime::now(),
                EventSource::LegacyBackfill,
                EventInitiatorKind::AppUser,
                app_user_id,
                location,
                None,
            )
            .await
            .map_err(|e| LegacyBackfillError(format!("insert failed: {e}")))?;
        outcome.inserted += 1;
    }

    crate::startup::repair_one(db, app_user_id, org_id)
        .await
        .map_err(|e| LegacyBackfillError(format!("status reconciliation failed: {e}")))?;

    Ok(outcome)
}

/// Result of a config-time preview: a small mapped sample, never written
/// anywhere. Used by `POST /orgs/me/legacy-backfill/preview`.
pub struct PreviewOutcome {
    pub sample: Vec<MappedEvent>,
    pub skipped_unmapped_action: usize,
    pub skipped_unparseable: usize,
}

/// Connect using `cfg` (built fresh from the admin's submitted, possibly
/// unsaved, input) and fetch + map up to `limit` documents for `username`.
/// Read-only: no insert, no AppUser mutation, no `legacy_backfill_done_at`
/// change (design D10).
pub async fn preview_mapped(
    cfg: &LegacyBackfillConfig,
    secret: &SecretBox,
    username: &str,
    limit: usize,
) -> Result<PreviewOutcome, LegacyBackfillError> {
    let connection_string = secret
        .decrypt(&cfg.connection_string_encrypted)
        .map_err(|_| {
            LegacyBackfillError("stored connection string could not be decrypted".to_string())
        })?;
    let (sample, outcome) =
        fetch_and_map(cfg, &connection_string, username, Some(limit as i64)).await?;
    Ok(PreviewOutcome {
        sample,
        skipped_unmapped_action: outcome.skipped_unmapped_action,
        skipped_unparseable: outcome.skipped_unparseable,
    })
}
