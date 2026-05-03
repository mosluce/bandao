mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn register_admin(app: &TestApp, email: &str, org_name: &str) -> (reqwest::Client, String) {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let resp = client
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": email,
            "password": "hunter2hunter2",
            "org_name": org_name,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let org_id = body["org"]["id"].as_str().unwrap().to_string();
    (client, org_id)
}

#[tokio::test]
async fn register_by_active_slug_joins_same_org() {
    let app = TestApp::spawn().await;
    let (admin, org_id) = register_admin(&app, "founder@example.com", "Acme").await;

    let set_resp = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acme" }))
        .send()
        .await
        .unwrap();
    assert_eq!(set_resp.status(), StatusCode::OK);
    let set_body: Value = set_resp.json().await.unwrap();
    assert_eq!(set_body["slug"], "acme");

    let joiner = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let join = joiner
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "member@example.com",
            "password": "hunter2hunter2",
            "org_code": "acme",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(join.status(), StatusCode::OK);
    let body: Value = join.json().await.unwrap();
    assert_eq!(body["org"]["id"], org_id);
    assert_eq!(body["user"]["role"], "member");
}
