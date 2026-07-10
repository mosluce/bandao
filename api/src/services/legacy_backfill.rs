//! Pure parsing/routing/build logic for the `legacy_backfill` example
//! script (`api/examples/legacy_backfill.rs`). Kept in the library crate —
//! not the example binary itself — so it's unit-testable via `cargo test`.
//!
//! The legacy system's `checkin_events`-shaped MongoDB documents look like:
//!
//! ```json
//! {
//!   "_id": ObjectId(...),
//!   "action": "上班",
//!   "at": ISODate(...),
//!   "domain": ObjectId(...),
//!   "signer": { "displayName": "...", "username": "fang" },
//!   "comment": "office",
//!   "geo": { "lat": 22.58, "lng": 120.36 },
//!   "address": "高雄市鳳山區頂庄路"
//! }
//! ```
//!
//! Field shape is deliberately hardcoded, not made configurable — see
//! `add-legacy-backfill-windows-and-pings` design.md decision 1: there is
//! currently one known customer/shape, and a generic field-mapping layer is
//! exactly the complexity the previous (rejected) design over-invested in.

use std::collections::HashMap;

use bson::doc;
use bson::oid::ObjectId;
use serde::Deserialize;

use crate::domain::{
    AppUser, CheckinEvent, CheckinEventType, EventInitiatorKind, EventLocation, EventSource,
    GeoPoint, LocationPing,
};

/// Raw shape of one legacy `checkin_events` document. Unknown fields
/// (`addressMeta`, `createdAt`, `updatedAt`, `signer.displayName`, ...) are
/// ignored by serde's default behavior — we only declare what we use.
#[derive(Debug, Clone, Deserialize)]
pub struct LegacyCheckinDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub action: String,
    pub at: bson::DateTime,
    pub domain: ObjectId,
    pub signer: LegacySigner,
    #[serde(default)]
    pub comment: Option<String>,
    pub geo: LegacyGeo,
    #[serde(default)]
    pub address: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LegacySigner {
    /// Absent on some real-world legacy documents (observed in KLCC's
    /// `sbsigns` collection — e.g. system-generated `路徑` pings with no
    /// resolved identity). A missing username can never match a bandao
    /// AppUser, so it's treated the same as an unmatched username rather
    /// than failing the whole document's deserialization.
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LegacyGeo {
    pub lat: f64,
    pub lng: f64,
}

/// Where a legacy document's `action` routes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutedAction {
    /// `上班` / `下班` / `轉出` / `轉入` — goes to `checkin_events`.
    Checkin(CheckinEventType),
    /// `路徑` — goes to `location_pings`.
    Path,
    /// Anything else — skipped, not written anywhere.
    Unrecognized,
}

/// Classify a legacy document's raw `action` string. See
/// `legacy-checkin-backfill` spec, "Legacy records are routed by action into
/// checkin_events or location_pings".
pub fn route_action(action: &str) -> RoutedAction {
    match action {
        "上班" => RoutedAction::Checkin(CheckinEventType::ClockIn),
        "下班" => RoutedAction::Checkin(CheckinEventType::ClockOut),
        "轉出" => RoutedAction::Checkin(CheckinEventType::TransferOut),
        "轉入" => RoutedAction::Checkin(CheckinEventType::TransferIn),
        "路徑" => RoutedAction::Path,
        _ => RoutedAction::Unrecognized,
    }
}

/// Build the `signer.username → app_user_id` lookup the import loop matches
/// legacy documents against. An AppUser's identity string is its `username`
/// (internal auth) OR — when absent — its `external_key` (external-database
/// auth shadow users, which carry no `username` at all). Real KLCC AppUsers
/// are all external-auth shadow users, so matching on `username` alone
/// leaves the map empty and every legacy document unmatched; `external_key`
/// holds the same ERP account identifier (`USERNO`) the legacy system's
/// `signer.username` was populated from.
pub fn build_identity_map(app_users: Vec<AppUser>) -> HashMap<String, ObjectId> {
    app_users
        .into_iter()
        .filter_map(|u| {
            let key = u.username.or(u.external_key)?;
            Some((key, u.id))
        })
        .collect()
}

/// Build the MongoDB filter for reading the legacy collection: scoped to one
/// legacy `domain`, a `since` lower bound (inclusive) on `at`, AND
/// `signer.username` in the given set of known identity keys (see
/// `build_identity_map`). Pushing the identity filter into the query
/// itself — rather than reading every document in the window and skipping
/// the ones that don't match client-side — matters at real scale: KLCC's
/// legacy collection has ~978K documents, the overwhelming majority of
/// which belong to people who were never onboarded into bandao and never
/// will be. Every re-run (this script is meant to be re-run repeatedly
/// during cutover) would otherwise re-scan and re-discard all of them.
pub fn legacy_query_filter(
    domain: ObjectId,
    since: bson::DateTime,
    known_identities: &[String],
) -> bson::Document {
    doc! {
        "domain": domain,
        "at": { "$gte": since },
        "signer.username": { "$in": known_identities },
    }
}

/// Build a `checkin_events` row from a legacy document already routed to a
/// `CheckinEventType`. `occurred_at_server` is set to the legacy document's
/// own `at` (not "now") — this is a faithful historical import, not a live
/// submission, so there is no real "server receipt time" to record.
pub fn build_checkin_event(
    doc: &LegacyCheckinDoc,
    org_id: ObjectId,
    app_user_id: ObjectId,
    event_type: CheckinEventType,
) -> CheckinEvent {
    CheckinEvent {
        id: ObjectId::new(),
        org_id,
        app_user_id,
        event_type,
        occurred_at_client: doc.at,
        occurred_at_server: doc.at,
        source: EventSource::LegacyBackfill,
        initiated_by_kind: EventInitiatorKind::AppUser,
        initiated_by_id: app_user_id,
        location: EventLocation {
            coordinates: GeoPoint {
                lat: doc.geo.lat,
                lng: doc.geo.lng,
            },
            accuracy_meters: None,
            region_name: doc.address.clone(),
            manual_label: doc.comment.clone(),
        },
        reason: None,
        legacy_source_id: Some(doc.id),
    }
}

/// Build a `location_pings` row from a legacy `路徑` document. Both
/// timestamps are set to the legacy document's own `at` — with no TTL on
/// `location_pings` (see `location-tracking` spec), there is no reason to
/// substitute "now" for `occurred_at_server` the way a live-submission
/// handler would.
pub fn build_location_ping(
    doc: &LegacyCheckinDoc,
    org_id: ObjectId,
    app_user_id: ObjectId,
) -> LocationPing {
    LocationPing {
        id: ObjectId::new(),
        org_id,
        app_user_id,
        lat: doc.geo.lat,
        lng: doc.geo.lng,
        accuracy_meters: None,
        occurred_at_client: doc.at,
        occurred_at_server: doc.at,
        legacy_source_id: Some(doc.id),
    }
}

/// Run summary counters, printed by the example script at the end of every
/// run (dry-run or real). Counting is identical in both modes — dry-run just
/// skips the actual upsert calls; see `legacy-checkin-backfill` spec,
/// "Dry-run mode reports without writing".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunSummary {
    pub clock_in: u64,
    pub clock_out: u64,
    pub transfer_out: u64,
    pub transfer_in: u64,
    pub location_pings: u64,
    pub skipped_unmatched_username: u64,
    pub skipped_unrecognized_action: u64,
    /// Documents that failed to deserialize into `LegacyCheckinDoc` at all
    /// (missing/malformed `action`, `at`, `domain`, or `geo` — a missing
    /// `signer.username` alone does NOT land here, see `LegacySigner`).
    /// Counted rather than only logged so large-scale data-quality issues
    /// are visible in the summary, not just scrolled past in stderr.
    pub skipped_malformed_document: u64,
}

impl RunSummary {
    pub fn record_checkin(&mut self, event_type: CheckinEventType) {
        match event_type {
            CheckinEventType::ClockIn => self.clock_in += 1,
            CheckinEventType::ClockOut => self.clock_out += 1,
            CheckinEventType::TransferOut => self.transfer_out += 1,
            CheckinEventType::TransferIn => self.transfer_in += 1,
        }
    }

    pub fn total_imported(&self) -> u64 {
        self.clock_in + self.clock_out + self.transfer_out + self.transfer_in + self.location_pings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AppUserAuthSource, AppUserStatus};

    fn fake_app_user(username: Option<&str>, external_key: Option<&str>) -> AppUser {
        let now = bson::DateTime::now();
        AppUser {
            id: ObjectId::new(),
            org_id: ObjectId::new(),
            username: username.map(str::to_string),
            username_lower: username.map(|u| u.to_lowercase()),
            display_name: "Test User".to_string(),
            password_hash: None,
            auth_source: if external_key.is_some() {
                AppUserAuthSource::External
            } else {
                AppUserAuthSource::Internal
            },
            external_key: external_key.map(str::to_string),
            status: AppUserStatus::Active,
            needs_password_change: false,
            last_login_at: None,
            created_by_dashboard_user_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn build_identity_map_uses_username_for_internal_auth() {
        let user = fake_app_user(Some("fang"), None);
        let id = user.id;
        let map = build_identity_map(vec![user]);
        assert_eq!(map.get("fang"), Some(&id));
    }

    #[test]
    fn build_identity_map_falls_back_to_external_key_for_shadow_users() {
        let user = fake_app_user(None, Some("mosluce"));
        let id = user.id;
        let map = build_identity_map(vec![user]);
        assert_eq!(map.get("mosluce"), Some(&id));
    }

    #[test]
    fn build_identity_map_skips_app_users_with_neither() {
        let user = fake_app_user(None, None);
        let map = build_identity_map(vec![user]);
        assert!(map.is_empty());
    }

    /// Real KLCC `sbsigns` documents (e.g. system-generated `路徑` pings)
    /// sometimes have `signer` present but no `username` sub-field. In
    /// production this class of document is now excluded by
    /// `legacy_query_filter`'s `signer.username: { $in: [...] }` clause
    /// before it's ever fetched — this test covers the type-level
    /// tolerance as defense-in-depth for any query that doesn't filter
    /// this way (see `tests/legacy_backfill_import.rs::documents_missing_signer_username_are_excluded_by_the_identity_scoped_query`).
    #[test]
    fn deserialize_tolerates_missing_signer_username() {
        let raw = doc! {
            "_id": ObjectId::new(),
            "action": "路徑",
            "at": bson::DateTime::now(),
            "domain": ObjectId::new(),
            "signer": { "displayName": "System" },
            "geo": { "lat": 22.6, "lng": 120.3 },
        };
        let parsed: LegacyCheckinDoc = bson::from_document(raw).expect("deserialize");
        assert!(parsed.signer.username.is_none());
    }

    #[test]
    fn route_action_maps_known_actions() {
        assert_eq!(
            route_action("上班"),
            RoutedAction::Checkin(CheckinEventType::ClockIn)
        );
        assert_eq!(
            route_action("下班"),
            RoutedAction::Checkin(CheckinEventType::ClockOut)
        );
        assert_eq!(
            route_action("轉出"),
            RoutedAction::Checkin(CheckinEventType::TransferOut)
        );
        assert_eq!(
            route_action("轉入"),
            RoutedAction::Checkin(CheckinEventType::TransferIn)
        );
        assert_eq!(route_action("路徑"), RoutedAction::Path);
    }

    #[test]
    fn route_action_unrecognized_falls_through() {
        assert_eq!(route_action("午休"), RoutedAction::Unrecognized);
        assert_eq!(route_action(""), RoutedAction::Unrecognized);
    }

    #[test]
    fn legacy_query_filter_shape() {
        let domain = ObjectId::new();
        let since = bson::DateTime::now();
        let identities = vec!["fang".to_string(), "mosluce".to_string()];
        let filter = legacy_query_filter(domain, since, &identities);
        assert_eq!(filter.get_object_id("domain").unwrap(), domain);
        assert_eq!(
            filter
                .get_document("at")
                .unwrap()
                .get_datetime("$gte")
                .unwrap(),
            &since
        );
        let in_clause = filter
            .get_document("signer.username")
            .unwrap()
            .get_array("$in")
            .unwrap();
        assert_eq!(in_clause.len(), 2);
    }

    #[test]
    fn build_checkin_event_carries_legacy_source_id_and_location() {
        let legacy_id = ObjectId::new();
        let at = bson::DateTime::now();
        let org_id = ObjectId::new();
        let app_user_id = ObjectId::new();
        let doc = LegacyCheckinDoc {
            id: legacy_id,
            action: "上班".to_string(),
            at,
            domain: ObjectId::new(),
            signer: LegacySigner {
                username: Some("fang".to_string()),
            },
            comment: Some("office".to_string()),
            geo: LegacyGeo {
                lat: 22.588,
                lng: 120.362,
            },
            address: Some("高雄市鳳山區頂庄路".to_string()),
        };

        let event = build_checkin_event(&doc, org_id, app_user_id, CheckinEventType::ClockIn);

        assert_eq!(event.legacy_source_id, Some(legacy_id));
        assert_eq!(event.source, EventSource::LegacyBackfill);
        assert_eq!(event.occurred_at_client, at);
        assert_eq!(event.occurred_at_server, at);
        assert_eq!(event.location.coordinates.lat, 22.588);
        assert_eq!(event.location.coordinates.lng, 120.362);
        assert_eq!(
            event.location.region_name.as_deref(),
            Some("高雄市鳳山區頂庄路")
        );
        assert_eq!(event.location.manual_label.as_deref(), Some("office"));
    }

    #[test]
    fn build_location_ping_carries_legacy_source_id() {
        let legacy_id = ObjectId::new();
        let at = bson::DateTime::now();
        let org_id = ObjectId::new();
        let app_user_id = ObjectId::new();
        let doc = LegacyCheckinDoc {
            id: legacy_id,
            action: "路徑".to_string(),
            at,
            domain: ObjectId::new(),
            signer: LegacySigner {
                username: Some("fang".to_string()),
            },
            comment: None,
            geo: LegacyGeo {
                lat: 22.6,
                lng: 120.3,
            },
            address: None,
        };

        let ping = build_location_ping(&doc, org_id, app_user_id);

        assert_eq!(ping.legacy_source_id, Some(legacy_id));
        assert_eq!(ping.occurred_at_client, at);
        assert_eq!(ping.occurred_at_server, at);
        assert_eq!(ping.lat, 22.6);
        assert_eq!(ping.lng, 120.3);
    }
}
