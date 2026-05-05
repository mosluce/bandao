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

    let (_joiner, join_body) = app.register_member("member@example.com", "acme").await;
    assert_eq!(join_body["current_org"]["id"], org_id);
    assert_eq!(join_body["role"], "member");
}
