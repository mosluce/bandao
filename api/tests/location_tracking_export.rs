//! `GET /checkin/users/:id/locations/export` — xlsx body, range validation,
//! cross-org guard.

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

#[tokio::test]
async fn export_happy_path_returns_xlsx() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;

    // Seed 3 pings.
    let _r = app
        .app_post(&app_client, &token, "/app/checkin/locations")
        .json(&json!({
            "pings": [
                { "lat": 25.0, "lng": 121.0, "occurred_at_client": iso_offset(-300) },
                { "lat": 25.001, "lng": 121.001, "occurred_at_client": iso_offset(-200) },
                { "lat": 25.002, "lng": 121.002, "occurred_at_client": iso_offset(-100) },
            ]
        }))
        .send()
        .await
        .unwrap();

    // Range covering the seeded pings.
    let from = iso_offset(-3600);
    let to = iso_offset(0);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations/export?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let ct = r
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        ct.contains("spreadsheetml") || ct.contains("xlsx"),
        "unexpected content-type: {ct}"
    );
    let cd = r
        .headers()
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        cd.contains("attachment"),
        "expected attachment disposition: {cd}"
    );
    assert!(cd.contains(".xlsx"), "expected .xlsx filename: {cd}");

    // Body is a non-empty xlsx (zip header `PK`).
    let bytes = r.bytes().await.unwrap();
    assert!(bytes.len() > 100, "xlsx body seems empty");
    assert_eq!(&bytes[..2], b"PK", "expected PKZIP header");
}

#[tokio::test]
async fn missing_from_returns_invalid_range() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, _client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let to = iso_offset(0);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations/export?to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "INVALID_RANGE");
}

#[tokio::test]
async fn missing_to_returns_invalid_range() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, _client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let from = iso_offset(-3600);
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations/export?from={from}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "INVALID_RANGE");
}

#[tokio::test]
async fn span_over_90_days_returns_invalid_range() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, _client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    // from = now - 1 day, to = now + 90 days  → span is 91 days.
    let from = iso_offset(-86_400);
    let to_t = ::time::OffsetDateTime::now_utc() + ::time::Duration::days(90);
    let to = to_t
        .format(&::time::format_description::well_known::Rfc3339)
        .unwrap();
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations/export?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "INVALID_RANGE");
}

#[tokio::test]
async fn from_older_than_90_days_is_allowed_when_span_fits() {
    // `location_pings` no longer has a 90-day TTL (see `location-tracking`
    // spec) — legacy-imported pings can be arbitrarily old, so a `from`
    // more than 90 days in the past must not be rejected on that basis
    // alone. The span cap (`to - from <= 90 days`) still applies and is
    // covered separately by `span_over_90_days_returns_invalid_range`.
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, _client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    // from 100 days ago, to 95 days ago — 5-day span, well within the cap,
    // but `from` alone would have violated the old 90-day floor.
    let from_t = ::time::OffsetDateTime::now_utc() - ::time::Duration::days(100);
    let from = from_t
        .format(&::time::format_description::well_known::Rfc3339)
        .unwrap();
    let to_t = ::time::OffsetDateTime::now_utc() - ::time::Duration::days(95);
    let to = to_t
        .format(&::time::format_description::well_known::Rfc3339)
        .unwrap();
    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations/export?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
}

#[tokio::test]
async fn export_cross_org_returns_404() {
    let app = TestApp::spawn().await;
    let (admin_a, _code_a, _id_a, _client_a, _token_a, _pw_a) = app
        .seed_app_user_ready_to_checkin("admin-a@example.com", "AcmeA", "alice", "Alice")
        .await;
    let (_admin_b, _code_b, app_user_b, _client_b, _token_b, _pw_b) = app
        .seed_app_user_ready_to_checkin("admin-b@example.com", "AcmeB", "bob", "Bob")
        .await;

    let from = iso_offset(-3600);
    let to = iso_offset(0);
    let r = admin_a
        .get(app.url(&format!(
            "/checkin/users/{app_user_b}/locations/export?from={from}&to={to}"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}
