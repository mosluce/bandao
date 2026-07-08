//! Wire shapes for the `/app/checkin/*` (mobile) and `/checkin/*` (admin)
//! handlers, plus `PATCH /orgs/me/settings`. Both surfaces share the
//! `CheckinEventDto` and `CheckinUserStatusDto` shapes.

use serde::{Deserialize, Serialize};

use crate::domain::{
    AppUser, AppUserCheckinStatus, CheckinEvent, CheckinEventType, CheckinUserStatus,
    EventInitiatorKind, EventLocation, EventSource, GeoPoint, Org,
};

/// One hour in milliseconds — boundary for the skew-warning flag.
pub const SKEW_WARNING_THRESHOLD_MS: i64 = 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize)]
pub struct GeoPointDto {
    pub lat: f64,
    pub lng: f64,
}

impl GeoPointDto {
    pub fn from_geo(g: &GeoPoint) -> Self {
        Self {
            lat: g.lat,
            lng: g.lng,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EventLocationDto {
    pub coordinates: GeoPointDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy_meters: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_label: Option<String>,
}

impl EventLocationDto {
    pub fn from_location(loc: &EventLocation) -> Self {
        Self {
            coordinates: GeoPointDto::from_geo(&loc.coordinates),
            accuracy_meters: loc.accuracy_meters,
            region_name: loc.region_name.clone(),
            manual_label: loc.manual_label.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckinEventDto {
    pub id: String,
    pub app_user_id: String,
    pub event_type: CheckinEventType,
    pub occurred_at_client: String,
    pub occurred_at_server: String,
    pub source: EventSource,
    pub initiated_by_kind: EventInitiatorKind,
    pub initiated_by_id: String,
    pub location: EventLocationDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// `true` when `|client - server| > 1 hour`. Computed server-side and
    /// surfaced once so admin-web doesn't have to recompute.
    pub has_skew_warning: bool,
}

impl CheckinEventDto {
    pub fn from_event(e: &CheckinEvent) -> Self {
        let skew_ms = (e.occurred_at_client.timestamp_millis()
            - e.occurred_at_server.timestamp_millis())
        .abs();
        Self {
            id: e.id.to_hex(),
            app_user_id: e.app_user_id.to_hex(),
            event_type: e.event_type,
            occurred_at_client: e
                .occurred_at_client
                .try_to_rfc3339_string()
                .unwrap_or_default(),
            occurred_at_server: e
                .occurred_at_server
                .try_to_rfc3339_string()
                .unwrap_or_default(),
            source: e.source,
            initiated_by_kind: e.initiated_by_kind,
            initiated_by_id: e.initiated_by_id.to_hex(),
            location: EventLocationDto::from_location(&e.location),
            reason: e.reason.clone(),
            has_skew_warning: skew_ms > SKEW_WARNING_THRESHOLD_MS,
        }
    }
}

/// Mobile-side status. The optional `last_event` is hydrated from the
/// `last_event_id` reference whenever present.
#[derive(Debug, Clone, Serialize)]
pub struct CheckinUserStatusDto {
    pub app_user_id: String,
    pub status: AppUserCheckinStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_shift_started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event: Option<CheckinEventDto>,
    /// Convenience flag for admin-web — same logic as on the event itself,
    /// but computed once on the latest event.
    pub has_skew_warning: bool,
}

impl CheckinUserStatusDto {
    pub fn from_status(status: &CheckinUserStatus, last_event: Option<&CheckinEvent>) -> Self {
        let dto_event = last_event.map(CheckinEventDto::from_event);
        let has_skew_warning = dto_event
            .as_ref()
            .map(|e| e.has_skew_warning)
            .unwrap_or(false);
        Self {
            app_user_id: status.app_user_id.to_hex(),
            status: status.status,
            current_shift_started_at: status
                .current_shift_started_at
                .and_then(|d| d.try_to_rfc3339_string().ok()),
            last_event: dto_event,
            has_skew_warning,
        }
    }
}

/// Admin live-board row: AppUser + status + skew warning.
#[derive(Debug, Clone, Serialize)]
pub struct CheckinUserBoardRowDto {
    pub user: BoardAppUserDto,
    pub status: AppUserCheckinStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_shift_started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event: Option<CheckinEventDto>,
    pub has_skew_warning: bool,
}

/// Trimmed AppUser shape — admin-web doesn't need the full DTO on the
/// live board, just enough to render the row label.
#[derive(Debug, Clone, Serialize)]
pub struct BoardAppUserDto {
    pub id: String,
    pub username: String,
    pub display_name: String,
}

impl BoardAppUserDto {
    pub fn from_app_user(u: &AppUser) -> Self {
        Self {
            id: u.id.to_hex(),
            // External shadow users have no username; fall back to their
            // external_key so the board still shows a stable identifier.
            username: u
                .username
                .clone()
                .or_else(|| u.external_key.clone())
                .unwrap_or_default(),
            display_name: u.display_name.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SubmitCheckinEventRequest {
    pub event_type: CheckinEventType,
    pub lat: f64,
    pub lng: f64,
    #[serde(default)]
    pub accuracy: Option<f64>,
    #[serde(default)]
    pub manual_label: Option<String>,
    /// RFC3339 timestamp from the AppUser device.
    pub occurred_at_client: String,
}

#[derive(Debug, Serialize)]
pub struct SubmitCheckinEventResponse {
    pub event: CheckinEventDto,
    pub status: CheckinUserStatusDto,
}

#[derive(Debug, Deserialize, Default)]
pub struct ForceCheckoutRequest {
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateOrgSettingsRequest {
    #[serde(default)]
    pub transfer_enabled: Option<bool>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub location_tracking_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct OrgSettingsDto {
    pub timezone: String,
    pub checkin: OrgCheckinSettingsDto,
}

#[derive(Debug, Serialize)]
pub struct OrgCheckinSettingsDto {
    pub transfer_enabled: bool,
    pub location_tracking_enabled: bool,
}

impl OrgSettingsDto {
    pub fn from_org(org: &Org) -> Self {
        Self {
            timezone: org.timezone.clone(),
            checkin: OrgCheckinSettingsDto {
                transfer_enabled: org.checkin_transfer_enabled(),
                location_tracking_enabled: org.checkin_location_tracking_enabled(),
            },
        }
    }
}

/// Cursor query for the events list endpoints.
#[derive(Debug, Deserialize)]
pub struct EventsCursorQuery {
    /// `occurred_at_client` of the last item from the previous page (RFC3339).
    /// When absent, the first page is returned.
    #[serde(default)]
    pub before: Option<String>,
    /// Optional override for the page size; capped server-side.
    #[serde(default)]
    pub limit: Option<i64>,
}
