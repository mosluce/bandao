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

/// How an Org authenticates its App users. `Internal` (the default when the
/// field is absent) uses admin-created `app_users` + password hashing;
/// `ExternalDb` delegates credential verification to an external database via
/// `settings.external_auth`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrgAuthSource {
    Internal,
    ExternalDb,
}

/// Connection + query configuration for external-database App-user auth, stored
/// at `Org.settings.external_auth`. `password_encrypted` is the ciphertext of the
/// database connection password (never the plaintext, never returned by the API).
/// `query` is a parameterized template that MUST contain `@account` and
/// `@password` placeholders; `key_col` / `display_col` name the result columns
/// that map to the shadow user's `external_key` / `display_name`.
/// Transport encryption the driver negotiates with the external database.
/// Mirrors the MSSQL client's levels: `Off` (no TLS), `Optional` (encrypt when
/// the server supports it), `Required` (encryption mandatory). Non-secret
/// configuration. Defaults to `Optional` when absent — friendliest for the
/// legacy on-prem MSSQL instances common among target customers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EncryptMode {
    Off,
    #[default]
    Optional,
    Required,
}

fn default_trust_server_certificate() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAuthConfig {
    pub driver: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password_encrypted: String,
    pub query: String,
    pub key_col: String,
    pub display_col: String,
    /// Transport encryption level; absent in pre-existing documents → `Optional`.
    #[serde(default)]
    pub encrypt: EncryptMode,
    /// Trust an otherwise-invalid (e.g. self-signed) server certificate; absent
    /// in pre-existing documents → `true`. No effect when `encrypt == Off`.
    #[serde(default = "default_trust_server_certificate")]
    pub trust_server_certificate: bool,
}

impl Org {
    /// Read `Org.settings.auth_source`, defaulting to `Internal` when the field
    /// is absent (old Orgs predate external auth).
    pub fn auth_source(&self) -> OrgAuthSource {
        match self.settings.get_str("auth_source") {
            Ok("external_db") => OrgAuthSource::ExternalDb,
            _ => OrgAuthSource::Internal,
        }
    }

    /// Parse `Org.settings.external_auth` into a typed config, or `None` when it
    /// is absent or malformed.
    pub fn external_auth(&self) -> Option<ExternalAuthConfig> {
        let doc = self.settings.get_document("external_auth").ok()?;
        bson::from_document(doc.clone()).ok()
    }
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

/// How an AppUser authenticates. `Internal` users are admin-created and carry a
/// local `password_hash` + `username`. `External` users are just-in-time shadow
/// identities provisioned on first successful external-database login; they carry
/// an `external_key` instead and have no local password. Old AppUser docs predate
/// this field and default to `Internal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppUserAuthSource {
    Internal,
    External,
}

fn default_auth_source() -> AppUserAuthSource {
    AppUserAuthSource::Internal
}

/// Mobile-end-user identity. 1:1 with Org via immutable `org_id`.
///
/// Internal users are unique per Org on `(org_id, username_lower)` and carry a
/// local `password_hash`; `username_lower` is denormalized to make
/// case-insensitive uniqueness a plain unique-index check. External shadow users
/// are unique per Org on `(org_id, external_key)`, have no `username` /
/// `password_hash` / `created_by_dashboard_user_id`, and are provisioned by the
/// system on first external login rather than by an admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUser {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub org_id: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username_lower: Option<String>,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_hash: Option<String>,
    #[serde(default = "default_auth_source")]
    pub auth_source: AppUserAuthSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_key: Option<String>,
    pub status: AppUserStatus,
    pub needs_password_change: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_login_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by_dashboard_user_id: Option<ObjectId>,
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
    /// Imported by the `legacy_backfill` example script from a customer's
    /// legacy check-in system. Distinct from `App` so the audit trail is
    /// honest about where the row actually came from.
    LegacyBackfill,
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
    /// The legacy source document's `_id` when `source = LegacyBackfill`.
    /// Backs the partial unique index that makes the `legacy_backfill`
    /// script's import idempotent across re-runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_source_id: Option<ObjectId>,
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
    /// Server-side wall time on receipt. No TTL index runs against this
    /// collection (see `location-tracking` spec) — retention is unbounded
    /// pending a future rotation mechanism.
    pub occurred_at_server: DateTime,
    /// The legacy source document's `_id` when this ping was written by the
    /// `legacy_backfill` example script rather than submitted live. Backs
    /// the partial unique index that makes the script's import idempotent
    /// across re-runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_source_id: Option<ObjectId>,
}

// --- Org API tokens ---

/// Known, closed set of capabilities an API token can be scoped to. This is
/// deliberately NOT a free-text field: a mistyped scope would silently
/// produce a token no endpoint ever accepts, with no error until the first
/// failed call in production. New scopes are added here as new external
/// integrations need them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiTokenScope {
    /// Read-only checkin-events export (`checkin-export-zhengdan` and any
    /// future export consumer).
    #[serde(rename = "checkin:read")]
    CheckinRead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiTokenStatus {
    Active,
    Disabled,
}

/// Long-lived, Org-scoped credential for machine-to-machine API access
/// (scheduled scripts, external integrations) — distinct from dashboard
/// sessions (human, cookie-based) and AppUser sessions (mobile). Never
/// expires on its own; lifecycle is fully admin-driven (rotate / disable /
/// enable / delete). `token_hash` is a SHA-256 digest (base64-encoded) of
/// the full plaintext token — the plaintext itself is never stored and is
/// only ever returned once, at creation or rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgApiToken {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub org_id: ObjectId,
    pub name: String,
    pub token_hash: String,
    /// Short, non-reconstructable prefix of the plaintext token, kept only
    /// for UI recognizability (e.g. `bandao_at_xxxxxxxxxxxx`).
    pub token_prefix: String,
    pub scopes: Vec<ApiTokenScope>,
    pub status: ApiTokenStatus,
    pub created_at: DateTime,
    pub created_by: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotated_at: Option<DateTime>,
}

/// A `DashboardUser` password-reset token. `token_hash` is a SHA-256 digest
/// (base64-encoded, via `auth::api_token::hash_token`) of the raw token
/// emailed to the user — the raw value is never stored. Single-use
/// (`used_at`) and time-limited (`expires_at`); `created_at` also doubles as
/// the basis for the request-cooldown check on `POST /auth/forgot-password`
/// (no separate cooldown-marker collection).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetToken {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub user_id: ObjectId,
    pub token_hash: String,
    pub expires_at: DateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_at: Option<DateTime>,
    pub created_at: DateTime,
}
