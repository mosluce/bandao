//! `GET /orgs/me/checkin/events/export` — API-token-only, generic JSON
//! day-window export. See `openspec/specs/checkin-export-zhengdan/spec.md`.

mod common;

use bandao_api::domain::{
    CheckinEventType, EventInitiatorKind, EventLocation, EventSource, GeoPoint,
};
use bson::DateTime as BsonDateTime;
use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

fn loc() -> EventLocation {
    EventLocation {
        coordinates: GeoPoint {
            lat: 25.03,
            lng: 121.56,
        },
        accuracy_meters: None,
        region_name: None,
        manual_label: None,
    }
}

async fn seed(
    app: &TestApp,
    org_id: ObjectId,
    app_user_id: ObjectId,
    event_type: CheckinEventType,
    occurred_at_client: BsonDateTime,
) {
    app.db()
        .checkin_events
        .create(
            org_id,
            app_user_id,
            event_type,
            occurred_at_client,
            occurred_at_client,
            EventSource::App,
            EventInitiatorKind::AppUser,
            app_user_id,
            loc(),
            None,
        )
        .await
        .unwrap();
}

/// Registers an admin, creates one AppUser, and mints a `checkin:read`
/// token. Returns `(admin_client, org_id, app_user_id, token_secret)`.
async fn setup(app: &TestApp) -> (reqwest::Client, ObjectId, ObjectId, String) {
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = ObjectId::parse_str(body["current_org"]["id"].as_str().unwrap()).unwrap();
    let created = app.create_app_user(&admin, "alice", "Alice").await;
    let app_user_id = ObjectId::parse_str(created["user"]["id"].as_str().unwrap()).unwrap();

    let token_body: Value = admin
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "export token", "scopes": ["checkin:read"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let secret = token_body["secret"].as_str().unwrap().to_string();

    (admin, org_id, app_user_id, secret)
}

fn ms(millis: i64) -> BsonDateTime {
    BsonDateTime::from_millis(millis)
}

#[tokio::test]
async fn default_date_returns_todays_events_at_offset_and_excludes_transfers_and_yesterday() {
    let app = TestApp::spawn().await;
    let (_admin, org_id, app_user_id, secret) = setup(&app).await;

    let now_ms = bson::DateTime::now().timestamp_millis();
    let one_hour_ago = ms(now_ms - 60 * 60 * 1000);
    let thirty_hours_ago = ms(now_ms - 30 * 60 * 60 * 1000); // safely "yesterday" at any offset

    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockIn,
        one_hour_ago,
    )
    .await;
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::TransferOut,
        one_hour_ago,
    )
    .await;
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::TransferIn,
        one_hour_ago,
    )
    .await;
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockOut,
        thirty_hours_ago,
    )
    .await;

    let client = app.fresh_client();
    let resp = client
        .get(app.url("/orgs/me/checkin/events/export?utc_offset=%2B08:00"))
        .bearer_auth(&secret)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let events = body["events"].as_array().unwrap();
    assert_eq!(
        events.len(),
        1,
        "expected only today's clock_in, got {body}"
    );
    assert_eq!(events[0]["event_type"], "clock_in");
    assert_eq!(events[0]["app_user_display_name"], "Alice");
}

#[tokio::test]
async fn utc_offset_boundary_is_half_open() {
    let app = TestApp::spawn().await;
    let (_admin, org_id, app_user_id, secret) = setup(&app).await;

    // 2026-07-10 in UTC. At +08:00, local day 07-10 is [UTC 07-09 16:00, UTC 07-10 16:00).
    let just_inside = BsonDateTime::builder()
        .year(2026)
        .month(7)
        .day(10)
        .hour(15)
        .minute(59)
        .second(59)
        .build()
        .unwrap();
    let just_outside = BsonDateTime::builder()
        .year(2026)
        .month(7)
        .day(10)
        .hour(16)
        .minute(0)
        .second(0)
        .build()
        .unwrap();

    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockIn,
        just_inside,
    )
    .await;
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockOut,
        just_outside,
    )
    .await;

    let client = app.fresh_client();
    let resp = client
        .get(app.url("/orgs/me/checkin/events/export?date=2026-07-10&utc_offset=%2B08:00"))
        .bearer_auth(&secret)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "clock_in");
}

#[tokio::test]
async fn default_offset_is_plain_utc_day() {
    let app = TestApp::spawn().await;
    let (_admin, org_id, app_user_id, secret) = setup(&app).await;

    let inside_utc_day = BsonDateTime::builder()
        .year(2026)
        .month(7)
        .day(10)
        .hour(23)
        .minute(59)
        .second(59)
        .build()
        .unwrap();
    let outside_utc_day = BsonDateTime::builder()
        .year(2026)
        .month(7)
        .day(11)
        .hour(0)
        .minute(0)
        .second(0)
        .build()
        .unwrap();

    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockIn,
        inside_utc_day,
    )
    .await;
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockOut,
        outside_utc_day,
    )
    .await;

    let client = app.fresh_client();
    // No utc_offset query param at all — defaults to +00:00.
    let resp = client
        .get(app.url("/orgs/me/checkin/events/export?date=2026-07-10"))
        .bearer_auth(&secret)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["utc_offset"], "+00:00");
    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "clock_in");
}

#[tokio::test]
async fn requires_a_bearer_token() {
    let app = TestApp::spawn().await;
    let (_admin, _org_id, _app_user_id, _secret) = setup(&app).await;

    let client = app.fresh_client();
    let resp = client
        .get(app.url("/orgs/me/checkin/events/export"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn dashboard_session_cookie_is_not_accepted() {
    let app = TestApp::spawn().await;
    let (admin, _org_id, _app_user_id, _secret) = setup(&app).await;

    // `admin` carries a valid dashboard session cookie, no bearer token.
    let resp = admin
        .get(app.url("/orgs/me/checkin/events/export"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_without_checkin_read_scope_is_forbidden() {
    let app = TestApp::spawn().await;
    let (admin, _org_id, _app_user_id, _secret) = setup(&app).await;

    // A token scoped to nothing relevant is impossible to create today
    // (only one scope exists) — instead simulate "wrong scope" by minting a
    // token, then confirming a token that legitimately carries checkin:read
    // works, and that stripping it via direct DB manipulation is refused.
    // Simpler and equally faithful to the spec: disable the token and
    // confirm the generic 401 path (scope enforcement itself is unit-level
    // covered by `ApiTokenAuthContext::require_scope` in
    // `add-org-api-tokens`'s own test suite). Cross-checked here via a
    // token whose scopes field is manually emptied at the DB layer.
    let token_body: Value = admin
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "scopeless", "scopes": ["checkin:read"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let secret = token_body["secret"].as_str().unwrap().to_string();
    let token_id = ObjectId::parse_str(token_body["token"]["id"].as_str().unwrap()).unwrap();

    // Directly clear the scopes on the stored row — the only way to get a
    // token with zero scopes since the create endpoint requires >=1.
    app.db()
        .database
        .collection::<bson::Document>("org_api_tokens")
        .update_one(
            bson::doc! { "_id": token_id },
            bson::doc! { "$set": { "scopes": [] } },
        )
        .await
        .unwrap();

    let client = app.fresh_client();
    let resp = client
        .get(app.url("/orgs/me/checkin/events/export"))
        .bearer_auth(&secret)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn malformed_date_and_offset_are_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _org_id, _app_user_id, secret) = setup(&app).await;
    let client = app.fresh_client();

    let bad_date = client
        .get(app.url("/orgs/me/checkin/events/export?date=not-a-date"))
        .bearer_auth(&secret)
        .send()
        .await
        .unwrap();
    assert_eq!(bad_date.status(), StatusCode::BAD_REQUEST);

    let bad_offset = client
        .get(app.url("/orgs/me/checkin/events/export?utc_offset=bogus"))
        .bearer_auth(&secret)
        .send()
        .await
        .unwrap();
    assert_eq!(bad_offset.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn explicit_past_date_returns_that_days_data() {
    let app = TestApp::spawn().await;
    let (_admin, org_id, app_user_id, secret) = setup(&app).await;

    let past_day = BsonDateTime::builder()
        .year(2020)
        .month(1)
        .day(15)
        .hour(3)
        .minute(0)
        .second(0)
        .build()
        .unwrap();
    seed(
        &app,
        org_id,
        app_user_id,
        CheckinEventType::ClockIn,
        past_day,
    )
    .await;

    let client = app.fresh_client();
    let resp = client
        .get(app.url("/orgs/me/checkin/events/export?date=2020-01-15"))
        .bearer_auth(&secret)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["date"], "2020-01-15");
    assert_eq!(body["events"].as_array().unwrap().len(), 1);
}
