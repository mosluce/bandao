//! Admin endpoints for legacy check-in backfill: `POST /orgs/me/legacy-backfill`
//! (save connection + field-mapping config), `POST /orgs/me/legacy-backfill/preview`
//! (dry-run — connect + map a sample, no writes), and
//! `GET /orgs/me/legacy-backfill/jobs` (read-only job status list). All
//! admin-only, scoped to `current_org`.

use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::auth::extractor::RequireAdmin;
use crate::auth::secret_box::SecretBox;
use crate::domain::{CheckinEventType, LegacyBackfillConfig, LegacyBackfillJobStatus, Org};
use crate::error::{ApiError, ApiResult};
use crate::services::legacy_backfill::{provider, validate_config};
use crate::state::AppState;

/// Connection + field-mapping settings as submitted by an admin.
/// `connection_string` is write-only: `Some` (non-empty) replaces the stored
/// value; `None`/absent keeps the existing one (so editing other fields
/// doesn't require re-entering the connection string).
#[derive(Debug, Deserialize)]
pub struct LegacyBackfillInput {
    #[serde(default)]
    pub connection_string: Option<String>,
    pub database: String,
    pub collection: String,
    pub identity_field: String,
    pub timestamp_field: String,
    pub lat_field: String,
    pub lng_field: String,
    #[serde(default)]
    pub region_name_field: Option<String>,
    #[serde(default)]
    pub manual_label_field: Option<String>,
    pub action_field: String,
    #[serde(default)]
    pub action_map: HashMap<String, CheckinEventType>,
}

/// Secret-free view of `Org.settings.legacy_backfill` — the connection string
/// is never returned, only whether one is set.
#[derive(Debug, Serialize)]
pub struct LegacyBackfillSummaryDto {
    pub database: String,
    pub collection: String,
    pub identity_field: String,
    pub timestamp_field: String,
    pub lat_field: String,
    pub lng_field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_name_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_label_field: Option<String>,
    pub action_field: String,
    pub action_map: HashMap<String, CheckinEventType>,
    pub connection_configured: bool,
}

impl LegacyBackfillSummaryDto {
    pub fn from_config(cfg: &LegacyBackfillConfig) -> Self {
        Self {
            database: cfg.database.clone(),
            collection: cfg.collection.clone(),
            identity_field: cfg.identity_field.clone(),
            timestamp_field: cfg.timestamp_field.clone(),
            lat_field: cfg.lat_field.clone(),
            lng_field: cfg.lng_field.clone(),
            region_name_field: cfg.region_name_field.clone(),
            manual_label_field: cfg.manual_label_field.clone(),
            action_field: cfg.action_field.clone(),
            action_map: cfg
                .action_map
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            connection_configured: !cfg.connection_string_encrypted.is_empty(),
        }
    }
}

/// `POST /orgs/me/legacy-backfill` — save the Org's legacy backfill config.
pub async fn configure(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Json(input): Json<LegacyBackfillInput>,
) -> ApiResult<Json<LegacyBackfillSummaryDto>> {
    let org = state
        .db
        .orgs
        .find_by_id(active.org_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let cfg = build_config(&state, &org, input)?;
    let updated = state
        .db
        .orgs
        .set_legacy_backfill(active.org_id, &cfg)
        .await?;
    let saved = updated.legacy_backfill().ok_or(ApiError::Internal)?;
    Ok(Json(LegacyBackfillSummaryDto::from_config(&saved)))
}

#[derive(Debug, Deserialize)]
pub struct PreviewRequest {
    pub legacy_backfill: LegacyBackfillInput,
    pub test_username: String,
    #[serde(default = "default_preview_limit")]
    pub limit: usize,
}

fn default_preview_limit() -> usize {
    5
}

#[derive(Debug, Serialize)]
pub struct PreviewEventDto {
    pub event_type: CheckinEventType,
    pub occurred_at_client: String,
    pub lat: f64,
    pub lng: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PreviewResponse {
    pub connected: bool,
    pub sample: Vec<PreviewEventDto>,
    pub skipped_unmapped_action: usize,
    pub skipped_unparseable: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `POST /orgs/me/legacy-backfill/preview` — connect using the submitted
/// (possibly unsaved) config and fetch + map a small sample for
/// `test_username`. Never writes: no `checkin_events`, no AppUser mutation,
/// no `legacy_backfill_done_at` change.
pub async fn preview(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
    Json(req): Json<PreviewRequest>,
) -> ApiResult<Json<PreviewResponse>> {
    let org = state
        .db
        .orgs
        .find_by_id(active.org_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let cfg = build_config(&state, &org, req.legacy_backfill)?;
    let secret = state
        .config
        .secret_key
        .map(|k| SecretBox::from_key_bytes(&k))
        .ok_or(ApiError::LegacyBackfillUnavailable)?;

    match provider::preview_mapped(&cfg, &secret, &req.test_username, req.limit).await {
        Ok(preview) => Ok(Json(PreviewResponse {
            connected: true,
            sample: preview
                .sample
                .into_iter()
                .map(|e| PreviewEventDto {
                    event_type: e.event_type,
                    occurred_at_client: e
                        .occurred_at_client
                        .try_to_rfc3339_string()
                        .unwrap_or_default(),
                    lat: e.lat,
                    lng: e.lng,
                    region_name: e.region_name,
                    manual_label: e.manual_label,
                })
                .collect(),
            skipped_unmapped_action: preview.skipped_unmapped_action,
            skipped_unparseable: preview.skipped_unparseable,
            error: None,
        })),
        Err(err) => Ok(Json(PreviewResponse {
            connected: false,
            sample: Vec::new(),
            skipped_unmapped_action: 0,
            skipped_unparseable: 0,
            error: Some(err.to_string()),
        })),
    }
}

#[derive(Debug, Serialize)]
pub struct LegacyBackfillJobDto {
    pub id: String,
    pub app_user_id: String,
    pub status: LegacyBackfillJobStatus,
    pub attempts: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// `GET /orgs/me/legacy-backfill/jobs` — read-only job status list, newest
/// first. No manual-retry mutation in this iteration (design D11).
pub async fn list_jobs(
    State(state): State<AppState>,
    RequireAdmin(active): RequireAdmin,
) -> ApiResult<Json<Vec<LegacyBackfillJobDto>>> {
    let jobs = state
        .db
        .legacy_backfill_jobs
        .list_by_org(active.org_id)
        .await?;
    let mut out: Vec<LegacyBackfillJobDto> = jobs
        .iter()
        .map(|j| LegacyBackfillJobDto {
            id: j.id.to_hex(),
            app_user_id: j.app_user_id.to_hex(),
            status: j.status,
            attempts: j.attempts,
            last_error: j.last_error.clone(),
            created_at: j.created_at.try_to_rfc3339_string().unwrap_or_default(),
            updated_at: j.updated_at.try_to_rfc3339_string().unwrap_or_default(),
        })
        .collect();
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(Json(out))
}

/// Validate + resolve a [`LegacyBackfillConfig`], encrypting a freshly-supplied
/// connection string or reusing the stored ciphertext.
fn build_config(
    state: &AppState,
    org: &Org,
    input: LegacyBackfillInput,
) -> ApiResult<LegacyBackfillConfig> {
    validate_config(
        &input.identity_field,
        &input.timestamp_field,
        &input.lat_field,
        &input.lng_field,
        &input.action_field,
    )
    .map_err(ApiError::Validation)?;

    let connection_string_encrypted = match input.connection_string {
        Some(s) if !s.trim().is_empty() => {
            let secret = state
                .config
                .secret_key
                .map(|k| SecretBox::from_key_bytes(&k))
                .ok_or(ApiError::LegacyBackfillUnavailable)?;
            secret.encrypt(&s)?
        }
        _ => org
            .legacy_backfill()
            .map(|c| c.connection_string_encrypted)
            .filter(|c| !c.is_empty())
            .ok_or_else(|| ApiError::Validation("connection_string is required".to_string()))?,
    };

    Ok(LegacyBackfillConfig {
        connection_string_encrypted,
        database: input.database,
        collection: input.collection,
        identity_field: input.identity_field,
        timestamp_field: input.timestamp_field,
        lat_field: input.lat_field,
        lng_field: input.lng_field,
        region_name_field: input.region_name_field,
        manual_label_field: input.manual_label_field,
        action_field: input.action_field,
        action_map: input.action_map,
    })
}
