//! `GET /checkin/users/:id/events?from=&to=` — Org-member history range filter.
//!
//! The admin trajectory page fetches one calendar day of events. Before the
//! range filter existed it over-fetched a newest-first page and filtered in
//! the browser, which silently returned nothing for any date beyond the
//! page's reach — including every `legacy_backfill`-imported day. The
//! `retrieves_an_old_day_behind_more_than_100_newer_events` test below is the
//! regression guard for exactly that.

mod common;

use bandao_api::domain::{
    CheckinEventType, EventInitiatorKind, EventLocation, EventSource, GeoPoint,
};
use bson::DateTime;
use bson::oid::ObjectId;
use common::{TestApp, ts};
use reqwest::StatusCode;
use serde_json::Value;

const MINUTE_MS: i64 = 60 * 1000;
const DAY_MS: i64 = 24 * 60 * MINUTE_MS;

async fn seed_four_events(app: &TestApp, client: &reqwest::Client, token: &str) {
    for (i, minute) in [-40i64, -30, -20, -10].iter().enumerate() {
        let event_type = if i % 2 == 0 { "clock_in" } else { "clock_out" };
        let r = app
            .submit_checkin_event(client, token, event_type, 25.0, 121.0, &ts(*minute))
            .await;
        assert_eq!(r.status(), StatusCode::CREATED, "seed event {i} failed");
    }
}

/// Insert an event straight through the repository. Bypasses the live
/// state-machine and ordering validation on purpose: this seeds bulk history
/// and back-dated rows that the HTTP path would (correctly) refuse, the same
/// way `legacy_backfill` writes them.
async fn insert_event_at(
    app: &TestApp,
    org_id: ObjectId,
    app_user_id: ObjectId,
    event_type: CheckinEventType,
    occurred_at_client: DateTime,
    source: EventSource,
) {
    app.db()
        .checkin_events
        .create(
            org_id,
            app_user_id,
            event_type,
            occurred_at_client,
            DateTime::now(),
            source,
            EventInitiatorKind::AppUser,
            app_user_id,
            EventLocation {
                coordinates: GeoPoint {
                    lat: 25.0,
                    lng: 121.0,
                },
                accuracy_meters: Some(10.0),
                region_name: None,
                manual_label: None,
            },
            None,
        )
        .await
        .expect("insert event fixture");
}

fn rfc3339(millis: i64) -> String {
    DateTime::from_millis(millis)
        .try_to_rfc3339_string()
        .expect("rfc3339")
}

#[tokio::test]
async fn admin_from_to_filters_to_range() {
    let app = TestApp::spawn().await;
    let (admin, _code, alice_id, alice_client, alice_token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    seed_four_events(&app, &alice_client, &alice_token).await;

    let from = ts(-35);
    let to = ts(-15);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{alice_id}/events?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let mut prev: Option<String> = None;
    for e in arr {
        let t = e["occurred_at_client"].as_str().unwrap().to_string();
        assert!(t >= from && t < to);
        if let Some(prev_t) = prev {
            assert!(t < prev_t, "expected newest-first");
        }
        prev = Some(t);
    }
}

#[tokio::test]
async fn member_sees_the_same_ranged_page_as_admin() {
    let app = TestApp::spawn().await;
    let (admin, code, alice_id, alice_client, alice_token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    seed_four_events(&app, &alice_client, &alice_token).await;
    let (member, _) = app
        .register_member(&admin, "member@example.com", &code)
        .await;

    let from = ts(-35);
    let to = ts(-15);
    let path = format!("/checkin/users/{alice_id}/events?from={from}&to={to}");

    let admin_body: Value = admin
        .get(app.url(&path))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let member_resp = member.get(app.url(&path)).send().await.unwrap();
    assert_eq!(member_resp.status(), StatusCode::OK);
    let member_body: Value = member_resp.json().await.unwrap();
    assert_eq!(
        admin_body, member_body,
        "member's ranged page must match admin's exactly"
    );
}

/// The reason the range filter exists. An AppUser with a 200-day-old day and
/// well over a page of newer events must still surface that old day when it
/// is asked for by range.
#[tokio::test]
async fn retrieves_an_old_day_behind_more_than_100_newer_events() {
    let app = TestApp::spawn().await;
    let (admin, _code, alice_id, _alice_client, _alice_token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let alice_oid = ObjectId::parse_str(&alice_id).unwrap();
    let org_id = app
        .db()
        .app_users
        .find_by_id(alice_oid)
        .await
        .unwrap()
        .expect("alice exists")
        .org_id;

    let now_ms = DateTime::now().timestamp_millis();
    let old_day_start = now_ms - 200 * DAY_MS;

    // Two events on the old day: a clock_in and a clock_out, as
    // `legacy_backfill` would have written them.
    insert_event_at(
        &app,
        org_id,
        alice_oid,
        CheckinEventType::ClockIn,
        DateTime::from_millis(old_day_start),
        EventSource::LegacyBackfill,
    )
    .await;
    insert_event_at(
        &app,
        org_id,
        alice_oid,
        CheckinEventType::ClockOut,
        DateTime::from_millis(old_day_start + 8 * 60 * MINUTE_MS),
        EventSource::LegacyBackfill,
    )
    .await;

    // 110 newer events — more than the 100 the clients used to over-fetch.
    for i in 0..110i64 {
        let event_type = if i % 2 == 0 {
            CheckinEventType::ClockIn
        } else {
            CheckinEventType::ClockOut
        };
        insert_event_at(
            &app,
            org_id,
            alice_oid,
            event_type,
            DateTime::from_millis(now_ms - (110 - i) * MINUTE_MS),
            EventSource::App,
        )
        .await;
    }

    // Unranged: the old day is nowhere in the default page.
    let unranged: Value = admin
        .get(app.url(&format!("/checkin/users/{alice_id}/events?limit=100")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let old_day_iso = rfc3339(old_day_start);
    assert!(
        !unranged
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["occurred_at_client"].as_str().unwrap() <= old_day_iso.as_str()),
        "precondition: the old day should be crowded out of an unranged page"
    );

    // Ranged: the old day comes back.
    let from = rfc3339(old_day_start - MINUTE_MS);
    let to = rfc3339(old_day_start + DAY_MS);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{alice_id}/events?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2, "the old day's two events must be returned");
    for e in arr {
        assert_eq!(e["source"].as_str().unwrap(), "legacy_backfill");
    }
}

#[tokio::test]
async fn admin_span_over_90_days_rejected() {
    let app = TestApp::spawn().await;
    let (admin, _code, alice_id, _c, _t, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let from = ts(-91 * 24 * 60);
    let to = ts(0);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{alice_id}/events?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"].as_str().unwrap(), "INVALID_RANGE");
}

#[tokio::test]
async fn admin_to_before_from_rejected() {
    let app = TestApp::spawn().await;
    let (admin, _code, alice_id, _c, _t, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let from = ts(0);
    let to = ts(-24 * 60);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{alice_id}/events?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"].as_str().unwrap(), "INVALID_RANGE");
}

/// Org scoping is checked before range validation, so a cross-Org id must
/// collapse to NOT_FOUND even when the range itself is invalid — the caller
/// learns nothing about whether that AppUser exists.
#[tokio::test]
async fn cross_org_target_is_not_found_even_with_an_invalid_range() {
    let app = TestApp::spawn().await;
    let (_admin_a, _code_a, alice_id, _c, _t, _pw) = app
        .seed_app_user_ready_to_checkin("a@example.com", "OrgA", "alice", "Alice")
        .await;
    let (admin_b, _code_b) = {
        let (client, body) = app.register_admin("b@example.com", "OrgB").await;
        (client, common::current_org_code(&body))
    };

    // Invalid range (inverted) plus a foreign AppUser id.
    let from = ts(0);
    let to = ts(-24 * 60);
    let r = admin_b
        .get(app.url(&format!(
            "/checkin/users/{alice_id}/events?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::NOT_FOUND,
        "org scoping must be decided before the range"
    );
}
