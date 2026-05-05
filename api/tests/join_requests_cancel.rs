//! `DELETE /me/join-requests/:id` — submitter cancels own pending.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn submit(app: &TestApp, client: &reqwest::Client, org_code: &str) -> String {
    let r = client
        .post(app.url("/me/join-requests"))
        .json(&json!({ "org_code": org_code }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let body: Value = r.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn cancel_own_pending_marks_cancelled() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    let bystander = app.fresh_client();
    bystander
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": "alice@example.com",
            "password": "hunter2hunter2",
            "org_name": "AliceOrg",
        }))
        .send()
        .await
        .unwrap();

    let id = submit(&app, &bystander, &code).await;

    let r = bystander
        .delete(app.url(&format!("/me/join-requests/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    let listed: Value = bystander
        .get(app.url("/me/join-requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr = listed.as_array().unwrap();
    let row = arr
        .iter()
        .find(|r| r["org"]["code"] == code)
        .expect("cancelled row visible");
    assert_eq!(row["status"], "cancelled");
    assert!(row["decided_at"].is_string());
}

#[tokio::test]
async fn cancel_someone_elses_returns_404() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    // Two unrelated identities each submit a request.
    let alice = app.fresh_client();
    alice
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": "alice@example.com",
            "password": "hunter2hunter2",
            "org_name": "AliceOrg",
        }))
        .send()
        .await
        .unwrap();
    let alice_id = submit(&app, &alice, &code).await;

    let bob = app.fresh_client();
    bob.post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": "bob@example.com",
            "password": "hunter2hunter2",
            "org_name": "BobOrg",
        }))
        .send()
        .await
        .unwrap();

    // Bob tries to cancel Alice's request.
    let r = bob
        .delete(app.url(&format!("/me/join-requests/{alice_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cancel_already_cancelled_returns_400() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    let alice = app.fresh_client();
    alice
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": "alice@example.com",
            "password": "hunter2hunter2",
            "org_name": "AliceOrg",
        }))
        .send()
        .await
        .unwrap();
    let id = submit(&app, &alice, &code).await;

    alice
        .delete(app.url(&format!("/me/join-requests/{id}")))
        .send()
        .await
        .unwrap();

    let r = alice
        .delete(app.url(&format!("/me/join-requests/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "INVALID_STATE");
}
