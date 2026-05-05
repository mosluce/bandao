mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn register_by_active_slug_joins_same_org() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let org_id = admin_body["current_org"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let set_resp = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acme" }))
        .send()
        .await
        .unwrap();
    assert_eq!(set_resp.status(), StatusCode::OK);
    let set_body: Value = set_resp.json().await.unwrap();
    assert_eq!(set_body["slug"], "acme");

    // Slug-based register-join now produces a pending join_request and a
    // zero-org session. Verify the pending request was filed against the
    // correct org.
    let (_joiner, join_body) = app
        .register_member_pending("member@example.com", "acme")
        .await;
    assert!(join_body["current_org"].is_null());
    assert!(join_body["role"].is_null());
    let pending: Value = admin
        .get(app.url("/orgs/me/join-requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        pending
            .as_array()
            .map(|a| a.iter().any(|r| r["email"] == "member@example.com"))
            .unwrap_or(false),
        "expected pending request for slug-based join, got {pending:?}"
    );
    let _ = org_id;
}
