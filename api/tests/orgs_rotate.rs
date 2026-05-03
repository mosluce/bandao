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
    let code = body["org"]["code"].as_str().unwrap().to_string();
    (client, code)
}

async fn register_member(app: &TestApp, email: &str, org_code: &str) -> reqwest::Client {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let resp = client
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": email,
            "password": "hunter2hunter2",
            "org_code": org_code,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    client
}

#[tokio::test]
async fn admin_can_rotate_org_code_and_old_code_no_longer_joins() {
    let app = TestApp::spawn().await;
    let (admin, original_code) = register_admin(&app, "founder@example.com", "Acme").await;

    let rotate = admin
        .post(app.url("/orgs/me/code/rotate"))
        .send()
        .await
        .unwrap();
    assert_eq!(rotate.status(), StatusCode::OK);
    let body: Value = rotate.json().await.unwrap();
    let new_code = body["code"].as_str().unwrap().to_string();
    assert_ne!(new_code, original_code);
    assert_eq!(new_code.chars().count(), 10);

    // Old code must no longer be a valid join credential.
    let stranger = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let join_with_old = stranger
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "late@example.com",
            "password": "hunter2hunter2",
            "org_code": original_code,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(join_with_old.status(), StatusCode::BAD_REQUEST);
    let err: Value = join_with_old.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_ORG_CODE");

    // Joining with the new code still works.
    let arrival = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let join_with_new = arrival
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "new@example.com",
            "password": "hunter2hunter2",
            "org_code": new_code,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(join_with_new.status(), StatusCode::OK);
}

#[tokio::test]
async fn member_cannot_rotate_org_code() {
    let app = TestApp::spawn().await;
    let (_admin, code) = register_admin(&app, "founder@example.com", "Acme").await;
    let member = register_member(&app, "member@example.com", &code).await;

    let resp = member
        .post(app.url("/orgs/me/code/rotate"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "FORBIDDEN");
}
