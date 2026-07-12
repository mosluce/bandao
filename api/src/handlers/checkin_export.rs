//! `GET /orgs/me/checkin/events/export` — the `checkin-export-zhengdan`
//! capability's generic day-window export. API-token-only (no dashboard
//! session accepted); returns structured JSON, not any vendor-specific text
//! format — see `openspec/specs/checkin-export-zhengdan/spec.md`. The
//! accompanying Zhengdan PowerShell client (`integrations/zhengdan-checkin-export/`)
//! is the one that renders this into the fixed-width text file.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Query, State};
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use time::UtcOffset;

use crate::auth::api_token::ApiTokenAuthContext;
use crate::domain::{ApiTokenScope, CheckinEventType};
use crate::error::ApiResult;
use crate::services::utc_offset::{day_window_utc, parse_date, parse_offset, today_at_offset};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    /// `+HH:MM` / `-HH:MM`. Defaults to `+00:00` (plain UTC day) when absent.
    utc_offset: Option<String>,
    /// `YYYY-MM-DD`. Defaults to "today" at `utc_offset`, computed from the
    /// server's own UTC clock — never trusts a caller-supplied date.
    date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExportEventDto {
    pub app_user_display_name: String,
    pub event_type: CheckinEventType,
    pub occurred_at_client: String,
}

#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub date: String,
    pub utc_offset: String,
    pub events: Vec<ExportEventDto>,
}

/// `GET /orgs/me/checkin/events/export` — `checkin:read`-scoped API token
/// only. Returns every `clock_in`/`clock_out` event for the token's Org
/// whose `occurred_at_client` falls in the requested day window.
pub async fn export(
    State(state): State<AppState>,
    token: ApiTokenAuthContext,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Json<ExportResponse>> {
    token.require_scope(ApiTokenScope::CheckinRead)?;

    let offset_raw = q.utc_offset.as_deref().unwrap_or("+00:00");
    let offset: UtcOffset = parse_offset(offset_raw)?;
    let date = match q.date.as_deref() {
        Some(raw) => parse_date(raw)?,
        None => today_at_offset(offset),
    };
    let (day_start, day_end) = day_window_utc(date, offset);

    let events = state
        .db
        .checkin_events
        .list_by_org_in_range_for_export(token.org_id, day_start, day_end)
        .await?;

    // Single batch fetch for the whole Org's roster rather than one query
    // per event — a day's event count for one Org is small, but N+1 is
    // still N+1.
    let app_users = state.db.app_users.list_by_org(token.org_id).await?;
    let names: HashMap<ObjectId, String> = app_users
        .into_iter()
        .map(|u| (u.id, u.display_name))
        .collect();

    let out_events = events
        .into_iter()
        .map(|e| ExportEventDto {
            app_user_display_name: names
                .get(&e.app_user_id)
                .cloned()
                .unwrap_or_else(|| e.app_user_id.to_hex()),
            event_type: e.event_type,
            occurred_at_client: e
                .occurred_at_client
                .try_to_rfc3339_string()
                .unwrap_or_default(),
        })
        .collect();

    Ok(Json(ExportResponse {
        date: format!("{date}"),
        utc_offset: offset_raw.to_string(),
        events: out_events,
    }))
}
