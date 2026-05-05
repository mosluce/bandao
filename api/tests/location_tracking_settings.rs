//! `Org.settings.checkin.location_tracking_enabled` toggle behavior + the
//! state-lock unification with `transfer_enabled`.

mod common;

use common::TestApp;
use common::ts;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn patch_settings(
    app: &TestApp,
    admin: &reqwest::Client,
    body: Value,
) -> reqwest::Response {
    admin
        .patch(app.url("/orgs/me/settings"))
        .json(&body)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn location_tracking_defaults_to_disabled() {
    let app = TestApp::spawn().await;
    let (admin, _code, _id, _app_client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let r = admin.get(app.url("/me")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(
        body["current_org"]["checkin"]["location_tracking_enabled"],
        false
    );
}

#[tokio::test]
async fn admin_can_enable_when_nobody_on_duty() {
    let app = TestApp::spawn().await;
    let (admin, _code, _id, _app_client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let r = patch_settings(&app, &admin, json!({ "location_tracking_enabled": true })).await;
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["checkin"]["location_tracking_enabled"], true);
}

#[tokio::test]
async fn state_locked_when_someone_on_duty() {
    let app = TestApp::spawn().await;
    let (admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    // Put alice on shift.
    let r = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);

    // Now flipping location_tracking_enabled is locked.
    let r = patch_settings(&app, &admin, json!({ "location_tracking_enabled": true })).await;
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "STATE_LOCKED");
    assert_eq!(body["error"]["on_duty_count"], 1);

    // transfer_enabled flip likewise locked (existing behavior, regression check).
    let r = patch_settings(&app, &admin, json!({ "transfer_enabled": false })).await;
    assert_eq!(r.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn timezone_only_change_not_blocked_by_lock() {
    let app = TestApp::spawn().await;
    let (admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let _ = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;

    // Timezone-only patch passes even with someone on duty.
    let r = patch_settings(&app, &admin, json!({ "timezone": "America/Los_Angeles" })).await;
    assert_eq!(r.status(), StatusCode::OK);
}

#[tokio::test]
async fn timezone_plus_toggle_falls_under_lock() {
    let app = TestApp::spawn().await;
    let (admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let _ = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;

    let r = patch_settings(
        &app,
        &admin,
        json!({ "timezone": "UTC", "location_tracking_enabled": true }),
    )
    .await;
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "STATE_LOCKED");
}
