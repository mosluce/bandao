//! Section 9.5 — `Org.settings.checkin.transfer_enabled` gates transfer
//! events; clock_in/clock_out unaffected; toggling back unblocks.

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
async fn transfer_disabled_blocks_transfer_events() {
    let app = TestApp::spawn().await;
    let (admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    // Flip to transfer_enabled=false. Nobody is on shift yet, so no
    // STATE_LOCKED.
    let r = patch_settings(&app, &admin, json!({ "transfer_enabled": false })).await;
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["checkin"]["transfer_enabled"], false);

    // clock_in still works.
    let r = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);

    // transfer_out blocked.
    let r = app
        .submit_checkin_event(&app_client, &token, "transfer_out", 25.04, 121.56, &ts(1))
        .await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "TRANSFER_DISABLED");

    // clock_out still works.
    let r = app
        .submit_checkin_event(&app_client, &token, "clock_out", 25.04, 121.56, &ts(2))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn toggling_back_to_true_unblocks_transfer() {
    let app = TestApp::spawn().await;
    let (admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let _ = patch_settings(&app, &admin, json!({ "transfer_enabled": false })).await;
    let _ = patch_settings(&app, &admin, json!({ "transfer_enabled": true })).await;

    let r = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    let r = app
        .submit_checkin_event(&app_client, &token, "transfer_out", 25.04, 121.56, &ts(1))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn default_org_has_transfer_enabled_true() {
    let app = TestApp::spawn().await;
    // Bare bones: just register a fresh admin.
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let _org_id = body["current_org"]["id"].as_str().unwrap().to_string();

    // Echo the settings back via no-op PATCH.
    let r = patch_settings(&app, &_admin, json!({})).await;
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["checkin"]["transfer_enabled"], true);
    assert_eq!(body["timezone"], "Asia/Taipei");
}
