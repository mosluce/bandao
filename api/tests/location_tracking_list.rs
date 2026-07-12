//! `GET /checkin/users/:id/locations` — admin pagination, cross-org guard,
//! and member role gating.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn enable_tracking(app: &TestApp, admin: &reqwest::Client) {
    let r = admin
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({ "location_tracking_enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
}

fn iso_offset(seconds: i64) -> String {
    let t = ::time::OffsetDateTime::now_utc() + ::time::Duration::seconds(seconds);
    t.format(&::time::format_description::well_known::Rfc3339)
        .unwrap()
}

async fn seed_pings(app: &TestApp, app_client: &reqwest::Client, token: &str, count: usize) {
    let pings: Vec<Value> = (0..count)
        .map(|i| {
            json!({
                "lat": 25.0,
                "lng": 121.0 + (i as f64) * 0.001,
                "occurred_at_client": iso_offset(-(count as i64 - i as i64) * 10),
            })
        })
        .collect();
    let r = app
        .app_post(app_client, token, "/app/checkin/locations")
        .json(&json!({ "pings": pings }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED, "seed ping batch failed");
}

#[tokio::test]
async fn admin_lists_pings_newest_first() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &app_client, &token, 5).await;

    let r = admin
        .get(app.url(&format!("/checkin/users/{app_user_id}/locations")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 5);

    // Verify descending by occurred_at_client.
    let mut prev: Option<String> = None;
    for p in arr {
        let t = p["occurred_at_client"].as_str().unwrap().to_string();
        if let Some(prev_t) = prev {
            assert!(t < prev_t, "expected descending");
        }
        prev = Some(t);
    }
}

#[tokio::test]
async fn before_cursor_filters_results() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &app_client, &token, 10).await;

    let r1 = admin
        .get(app.url(&format!("/checkin/users/{app_user_id}/locations?limit=5")))
        .send()
        .await
        .unwrap();
    let body1: Value = r1.json().await.unwrap();
    let first_page = body1.as_array().unwrap();
    assert_eq!(first_page.len(), 5);
    let oldest = first_page.last().unwrap()["occurred_at_client"]
        .as_str()
        .unwrap()
        .to_string();

    let r2 = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations?limit=5&before={oldest}"
        )))
        .send()
        .await
        .unwrap();
    let body2: Value = r2.json().await.unwrap();
    let second_page = body2.as_array().unwrap();
    // The remaining 5 pings are all strictly older than `oldest`.
    assert_eq!(second_page.len(), 5);
    for p in second_page {
        let t = p["occurred_at_client"].as_str().unwrap();
        assert!(t < oldest.as_str(), "second page should be strictly older");
    }
}

#[tokio::test]
async fn cross_org_app_user_id_returns_404() {
    let app = TestApp::spawn().await;
    let (admin_a, _code_a, _id_a, _client_a, _token_a, _pw_a) = app
        .seed_app_user_ready_to_checkin("admin-a@example.com", "AcmeA", "alice", "Alice")
        .await;
    let (_admin_b, _code_b, app_user_b, _client_b, _token_b, _pw_b) = app
        .seed_app_user_ready_to_checkin("admin-b@example.com", "AcmeB", "bob", "Bob")
        .await;

    let r = admin_a
        .get(app.url(&format!("/checkin/users/{app_user_b}/locations")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn from_to_filters_to_range() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;

    // Seed 5 pings with timestamps -50, -40, -30, -20, -10 seconds.
    seed_pings(&app, &app_client, &token, 5).await;

    // Range covering only the middle 3 (offsets -40 .. -20 inclusive).
    let from = iso_offset(-45);
    let to = iso_offset(-15);

    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 3, "expected 3 pings inside the range");

    // All in newest-first order, all within [from, to).
    let mut prev: Option<String> = None;
    for p in arr {
        let t = p["occurred_at_client"].as_str().unwrap().to_string();
        assert!(t >= from);
        assert!(t < to);
        if let Some(prev_t) = prev {
            assert!(t < prev_t);
        }
        prev = Some(t);
    }
}

#[tokio::test]
async fn from_only_lower_bound() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &app_client, &token, 5).await;

    // from at -25s → 2 pings expected (-20, -10).
    let from = iso_offset(-25);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations?from={from}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
}

#[tokio::test]
async fn to_only_upper_bound() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &app_client, &token, 5).await;

    // to at -25s → 3 pings expected (-50, -40, -30).
    let to = iso_offset(-25);
    let r = admin
        .get(app.url(&format!("/checkin/users/{app_user_id}/locations?to={to}")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 3);
}

#[tokio::test]
async fn invalid_range_to_before_from() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, _app_client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;

    let from = iso_offset(0);
    let to = iso_offset(-3600);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"].as_str().unwrap(), "INVALID_RANGE");
}

#[tokio::test]
async fn invalid_range_span_too_large() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, _app_client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;

    // 91 days span.
    let from = iso_offset(-91 * 24 * 3600);
    let to = iso_offset(0);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"].as_str().unwrap(), "INVALID_RANGE");
}

#[tokio::test]
async fn from_older_than_90_days_is_allowed_when_span_fits() {
    // `location_pings` no longer has a 90-day TTL (see `location-tracking`
    // spec) — legacy-imported pings can be arbitrarily old, so a `from`
    // more than 90 days in the past must not be rejected on that basis
    // alone. The span cap (`to - from <= 90 days`) still applies and is
    // covered separately by `invalid_range_span_too_large`.
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, _app_client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;

    // from 91 days ago, to 30 days ago — span is 61 days, well within the
    // cap, but `from` alone would have violated the old 90-day floor.
    let from = iso_offset(-91 * 24 * 3600);
    let to = iso_offset(-30 * 24 * 3600);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
}

#[tokio::test]
async fn invalid_range_parse_failure() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, _app_client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;

    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations?from=not-a-date"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"].as_str().unwrap(), "INVALID_RANGE");
}

#[tokio::test]
async fn member_lists_pings_identically_to_admin() {
    let app = TestApp::spawn().await;
    let (admin, code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &app_client, &token, 5).await;

    let admin_body: Value = admin
        .get(app.url(&format!("/checkin/users/{app_user_id}/locations")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let (member_client, _) = app
        .register_member(&admin, "member@example.com", &code)
        .await;

    let r = member_client
        .get(app.url(&format!("/checkin/users/{app_user_id}/locations")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let member_body: Value = r.json().await.unwrap();
    assert_eq!(
        admin_body, member_body,
        "member's /locations response should be byte-for-byte identical to admin's"
    );
}

#[tokio::test]
async fn member_cross_org_app_user_id_still_returns_404() {
    let app = TestApp::spawn().await;
    let (admin_a, code_a, _id_a, _client_a, _token_a, _pw_a) = app
        .seed_app_user_ready_to_checkin("admin-a@example.com", "AcmeA", "alice", "Alice")
        .await;
    let (_admin_b, _code_b, app_user_b, _client_b, _token_b, _pw_b) = app
        .seed_app_user_ready_to_checkin("admin-b@example.com", "AcmeB", "bob", "Bob")
        .await;
    let (member_a, _) = app
        .register_member(&admin_a, "member-a@example.com", &code_a)
        .await;

    let r = member_a
        .get(app.url(&format!("/checkin/users/{app_user_b}/locations")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}
