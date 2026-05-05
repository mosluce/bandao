use bson::DateTime;
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Member,
}

/// IANA timezone default for new Orgs. Display-only — DB stores absolute UTC
/// regardless of this value. Validation happens via `chrono_tz` on writes.
pub const DEFAULT_ORG_TIMEZONE: &str = "Asia/Taipei";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Org {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub name: String,
    pub code: String,
    pub owner_id: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug_changed_at: Option<DateTime>,
    /// IANA timezone identifier (`Asia/Taipei` etc.). Default `Asia/Taipei`.
    /// Display-only.
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Free-form settings sub-document. Currently holds `checkin.transfer_enabled`.
    /// Old Org docs may be missing this entirely; readers MUST tolerate that
    /// and fall back to defaults — see `Org::checkin_transfer_enabled`.
    #[serde(default)]
    pub settings: bson::Document,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

fn default_timezone() -> String {
    DEFAULT_ORG_TIMEZONE.to_string()
}

impl Org {
    /// Read `Org.settings.checkin.transfer_enabled`, defaulting to `true` when
    /// the sub-document or the field is absent (old Orgs predate this change).
    pub fn checkin_transfer_enabled(&self) -> bool {
        self.settings
            .get_document("checkin")
            .ok()
            .and_then(|d| d.get_bool("transfer_enabled").ok())
            .unwrap_or(true)
    }

    /// Read `Org.settings.checkin.location_tracking_enabled`, defaulting to
    /// `false` when the sub-document or the field is absent. Privacy-default
    /// — no Org collects location pings without an explicit admin opt-in.
    pub fn checkin_location_tracking_enabled(&self) -> bool {
        self.settings
            .get_document("checkin")
            .ok()
            .and_then(|d| d.get_bool("location_tracking_enabled").ok())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgSlugReservation {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub slug: String,
    pub org_id: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime>,
    pub created_at: DateTime,
}

/// Pure identity record. The user's Org affiliations live in
/// `dashboard_memberships`, not on this row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardUser {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

/// One row per (user, org) pair. Carries the user's role in that org.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Membership {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub user_id: ObjectId,
    pub org_id: ObjectId,
    pub role: Role,
    pub joined_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JoinRequestStatus {
    Pending,
    Approved,
    Rejected,
    Cancelled,
}

/// One row per join attempt. Pending rows gate `dashboard_memberships`
/// creation — see `org-join-requests` capability spec. Terminal states
/// (`approved`/`rejected`/`cancelled`) are retained for audit; the
/// `(user_id, org_id, status)` partial unique index covers only `pending`
/// so a rejected user can re-apply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRequest {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub user_id: ObjectId,
    pub org_id: ObjectId,
    pub status: JoinRequestStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    pub requested_at: DateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<ObjectId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSession {
    #[serde(rename = "_id")]
    pub token: String,
    pub user_id: ObjectId,
    /// The Org this session is currently scoped to. Mutable across the session
    /// lifetime via `POST /me/current-org`. May be `None` for users with zero
    /// memberships, or whose memberships were all removed mid-session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_org_id: Option<ObjectId>,
    pub expires_at: DateTime,
    pub created_at: DateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RemovalKind {
    Kicked,
    Left,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedMembership {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub org_id: ObjectId,
    pub email: String,
    pub removed_at: DateTime,
    pub cooldown_until: DateTime,
    pub removal_kind: RemovalKind,
}

/// AppUser status. Soft-disable preserves history (FK target for future
/// checkin records, traces, etc.) while gating new logins. Re-enable just
/// flips back to `active` without touching `password_hash`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppUserStatus {
    Active,
    Disabled,
}

/// Mobile-end-user identity. 1:1 with Org via immutable `org_id`. Identifiers
/// are unique per Org (`(org_id, username_lower)` index). Created by an admin;
/// no self-registration. `username_lower` is denormalized to make case-insensitive
/// uniqueness a plain unique-index check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUser {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub org_id: ObjectId,
    pub username: String,
    pub username_lower: String,
    pub display_name: String,
    pub password_hash: String,
    pub status: AppUserStatus,
    pub needs_password_change: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_login_at: Option<DateTime>,
    pub created_by_dashboard_user_id: ObjectId,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

/// Mobile-side session. Token in `_id`, opaque random base64. TTL on
/// `expires_at` (Mongo TTL index). Sliding refresh on every authenticated
/// request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSession {
    #[serde(rename = "_id")]
    pub token: String,
    pub app_user_id: ObjectId,
    pub expires_at: DateTime,
    pub created_at: DateTime,
}

// --- Checkin events ---

/// The four AppUser-driven event types. `transfer_in` means "arrived at the
/// next worksite", not "back at the original primary".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckinEventType {
    ClockIn,
    ClockOut,
    TransferOut,
    TransferIn,
}

/// Three-state machine. `on_site` covers any current worksite; `in_transit`
/// covers movement between sites. Multi-site shifts cycle on_site ↔ in_transit
/// without round-tripping to off_duty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppUserCheckinStatus {
    OffDuty,
    OnSite,
    InTransit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    /// Submitted by the AppUser themselves via `/app/checkin/events`.
    App,
    /// Synthesised by an admin via `/checkin/users/:id/force-checkout`.
    AdminForce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventInitiatorKind {
    /// `initiated_by_id` points at an `app_users` row.
    AppUser,
    /// `initiated_by_id` points at a `dashboard_users` row.
    DashboardUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lng: f64,
}

/// Captured per-event location. `coordinates` is required; everything else is
/// optional so future event sources (admin-force, geocode failures) stay
/// representable without `Option<Location>` gymnastics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLocation {
    pub coordinates: GeoPoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy_meters: Option<f64>,
    /// Server-set via the `ReverseGeocoder` trait. `None` on geocode failure
    /// (timeout, non-2xx, parse error). The event still records normally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region_name: Option<String>,
    /// Free-text label from the AppUser ("公司門口", "工地A門口"). 1..=120 chars
    /// when present. Server replaces this with `"管理員強制收班"` on force-checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_label: Option<String>,
}

/// Append-only event row. One per state transition. Force-checkout writes
/// here too, with `source = AdminForce` and `initiated_by_kind = DashboardUser`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckinEvent {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub org_id: ObjectId,
    pub app_user_id: ObjectId,
    pub event_type: CheckinEventType,
    /// AppUser-supplied wall time. Trusted for ordering/display. May be in
    /// the past (offline sync) or future (clock skew).
    pub occurred_at_client: DateTime,
    /// Server-side wall time on receipt. Used only for skew warning + audit.
    pub occurred_at_server: DateTime,
    pub source: EventSource,
    pub initiated_by_kind: EventInitiatorKind,
    pub initiated_by_id: ObjectId,
    pub location: EventLocation,
    /// Free-text reason — only set by force-checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Denormalised current-state row, one per AppUser. Updated atomically with
/// each successful event so the admin live-board is one index hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckinUserStatus {
    /// `_id` is the AppUser id — uniqueness is intrinsic to the row.
    #[serde(rename = "_id")]
    pub app_user_id: ObjectId,
    pub org_id: ObjectId,
    pub status: AppUserCheckinStatus,
    /// Set when `status` becomes `on_site` from `off_duty`. Carried across
    /// transfers (on_site ↔ in_transit). Reset to `null` on `clock_out`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_shift_started_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event_id: Option<ObjectId>,
    pub updated_at: DateTime,
}

impl CheckinEventType {
    /// Apply the transition table. Returns the new status when the
    /// `(prior, event)` pair is legal; `None` otherwise.
    pub fn next_status(self, prior: AppUserCheckinStatus) -> Option<AppUserCheckinStatus> {
        use AppUserCheckinStatus::*;
        use CheckinEventType::*;
        match (prior, self) {
            (OffDuty, ClockIn) => Some(OnSite),
            (OnSite, ClockOut) => Some(OffDuty),
            (OnSite, TransferOut) => Some(InTransit),
            (InTransit, TransferIn) => Some(OnSite),
            (InTransit, ClockOut) => Some(OffDuty),
            _ => None,
        }
    }

    pub fn is_transfer(self) -> bool {
        matches!(
            self,
            CheckinEventType::TransferIn | CheckinEventType::TransferOut
        )
    }
}

// --- Location tracking ---

/// One periodic GPS sample submitted by an AppUser during their shift.
/// Sparse compared to checkin events: a 100m client-side distance filter and
/// a 60s minimum interval mean a typical shift produces tens to hundreds of
/// pings, not thousands. Pings carry no reverse-geocoded label by design —
/// volume rules out per-ping Nominatim calls; the admin map renders raw
/// coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationPing {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub org_id: ObjectId,
    pub app_user_id: ObjectId,
    pub lat: f64,
    pub lng: f64,
    /// Horizontal accuracy radius in meters (CoreLocation / FusedLocationProvider
    /// 68% confidence). May be absent if the OS didn't supply one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy_meters: Option<f64>,
    /// AppUser-supplied wall time at the moment the OS callback fired.
    pub occurred_at_client: DateTime,
    /// Server-side wall time on receipt. The 90-day TTL index runs against
    /// THIS field, not `occurred_at_client`, so a forward-jumped client clock
    /// can't extend the retention window.
    pub occurred_at_server: DateTime,
}
