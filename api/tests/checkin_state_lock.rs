//! Section 9.6 — state-lock semantics on `PATCH /orgs/me/settings`.
//! `transfer_enabled` blocked while AppUsers are on shift; `timezone` is
//! always changeable.

mod common;

use common::TestApp;
use common::ts;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn transfer_enabled_patch_succeeds_when_all_off_duty() {
    let app = TestApp::spawn().await;
    let (admin, _code, _id, _app_client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let r = admin
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({ "transfer_enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["checkin"]["transfer_enabled"], false);
}

#[tokio::test]
async fn transfer_enabled_patch_rejected_when_someone_on_shift() {
    let app = TestApp::spawn().await;
    let (admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let r = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);

    let r = admin
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({ "transfer_enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "STATE_LOCKED");
    assert_eq!(body["error"]["on_duty_count"], 1);
}

#[tokio::test]
async fn timezone_patch_not_blocked_by_state_lock() {
    let app = TestApp::spawn().await;
    let (admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let _ = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;

    // While on shift — timezone change still allowed.
    let r = admin
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({ "timezone": "America/Los_Angeles" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["timezone"], "America/Los_Angeles");
    assert_eq!(body["checkin"]["transfer_enabled"], true);
}

#[tokio::test]
async fn member_cannot_patch_settings() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let (member, _) = app.register_member("member@example.com", &code).await;

    let r = member
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({ "transfer_enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}
