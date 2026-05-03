//! Section 9.8 — geocoding failures collapse to `region_name = null` and
//! the event still records. Successful lookups populate the field.
//!
//! These tests substitute the production Nominatim impl with the
//! `StaticReverseGeocoder` stub via `TestApp::spawn_with_geocoder`.

mod common;

use argus_api::services::reverse_geocoder::StaticReverseGeocoder;
use common::TestApp;
use common::ts;
use reqwest::StatusCode;
use serde_json::Value;


#[tokio::test]
async fn geocode_none_records_null_region_name() {
    let app = TestApp::spawn_with_geocoder(StaticReverseGeocoder::new(None)).await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let r = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    let body: Value = r.json().await.unwrap();
    let region = body["event"]["location"].get("region_name");
    assert!(
        region.is_none() || region.unwrap().is_null(),
        "expected region_name absent or null, got {region:?}"
    );
}

#[tokio::test]
async fn geocode_some_populates_region_name() {
    let app =
        TestApp::spawn_with_geocoder(StaticReverseGeocoder::new(Some("Taipei City".to_string())))
            .await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let r = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["event"]["location"]["region_name"], "Taipei City");
}

#[tokio::test]
async fn manual_label_preserved_independent_of_geocode_outcome() {
    let app = TestApp::spawn_with_geocoder(StaticReverseGeocoder::new(None)).await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let resp = app
        .submit_checkin_event_with(
            &app_client,
            &token,
            serde_json::json!({
                "event_type": "clock_in",
                "lat": 25.04,
                "lng": 121.56,
                "manual_label": "公司門口",
                "occurred_at_client": ts(0),
            }),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["event"]["location"]["manual_label"], "公司門口");
}
