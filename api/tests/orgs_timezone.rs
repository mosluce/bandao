//! Section 9.12 — `Org.timezone` defaults to `Asia/Taipei`, accepts valid
//! IANA names, rejects garbage, gates on admin role, and never moves stored
//! timestamps.

mod common;

use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn default_org_timezone_is_asia_taipei() {
    let app = TestApp::spawn().await;
    let (admin, _body) = app.register_admin("admin@example.com", "Acme").await;
    // GET-equivalent via no-op PATCH.
    let r = admin
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["timezone"], "Asia/Taipei");
}

#[tokio::test]
async fn admin_can_update_timezone() {
    let app = TestApp::spawn().await;
    let (admin, _body) = app.register_admin("admin@example.com", "Acme").await;
    let r = admin
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({ "timezone": "America/Los_Angeles" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["timezone"], "America/Los_Angeles");
}

#[tokio::test]
async fn invalid_timezone_rejected() {
    let app = TestApp::spawn().await;
    let (admin, _body) = app.register_admin("admin@example.com", "Acme").await;
    for bad in ["Mars/Olympus", "GMT+8", "asia/taipei", ""] {
        let r = admin
            .patch(app.url("/orgs/me/settings"))
            .json(&json!({ "timezone": bad }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            r.status(),
            StatusCode::BAD_REQUEST,
            "expected INVALID_TIMEZONE for `{bad}`"
        );
        let body: Value = r.json().await.unwrap();
        assert_eq!(body["error"]["code"], "INVALID_TIMEZONE");
    }
}

#[tokio::test]
async fn member_cannot_change_timezone() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let (member, _) = app.register_member("member@example.com", &code).await;

    let r = member
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({ "timezone": "America/Los_Angeles" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn timezone_change_does_not_touch_stored_timestamps() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id_str = body["current_org"]["id"].as_str().unwrap().to_string();
    let org_id = ObjectId::parse_str(&org_id_str).unwrap();

    // Snapshot Org.created_at before the TZ change.
    let before = app
        .db()
        .orgs
        .find_by_id(org_id)
        .await
        .unwrap()
        .expect("org row");

    let r = admin
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({ "timezone": "America/Los_Angeles" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    let after = app
        .db()
        .orgs
        .find_by_id(org_id)
        .await
        .unwrap()
        .expect("org row");
    assert_eq!(
        before.created_at.timestamp_millis(),
        after.created_at.timestamp_millis()
    );
    // updated_at SHOULD bump (it's part of the patch). But created_at must
    // NOT — that's the point of the assertion.
}
