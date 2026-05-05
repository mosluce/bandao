//! `POST /me/join-requests` + `GET /me/join-requests`. Submitter side
//! of the new approval flow.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn submit_creates_pending_request() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    // Different identity, then call the new endpoint.
    let (member, member_body) = app
        .register_member_pending("alice@example.com", &code)
        .await;
    assert_eq!(member_body["current_org"], Value::Null);

    // Already-pending should reject the explicit submit too.
    let r = member
        .post(app.url("/me/join-requests"))
        .json(&json!({ "org_code": code }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "JOIN_REQUEST_PENDING");
}

#[tokio::test]
async fn submit_with_application_message_persists() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    // Create a fresh identity (via register_create) to keep the user but skip
    // the auto-pending in register_member_pending.
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

    let r = bystander
        .post(app.url("/me/join-requests"))
        .json(&json!({
            "org_code": code,
            "application_message": "我是承包商小王",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["application_message"], "我是承包商小王");

    // Admin sees it with email + message.
    let listed: Value = admin
        .get(app.url("/orgs/me/join-requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr = listed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["email"], "alice@example.com");
    assert_eq!(arr[0]["application_message"], "我是承包商小王");
}

#[tokio::test]
async fn submit_oversized_message_rejected() {
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

    let huge = "x".repeat(501);
    let r = bystander
        .post(app.url("/me/join-requests"))
        .json(&json!({ "org_code": code, "application_message": huge }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn submit_to_already_member_rejected() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    // Approve a member.
    let (member, _) = app
        .register_member_approved(&admin, "alice@example.com", &code)
        .await;

    // Now submit again.
    let r = member
        .post(app.url("/me/join-requests"))
        .json(&json!({ "org_code": code }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "ALREADY_MEMBER");
}

#[tokio::test]
async fn list_mine_returns_own_requests_newest_first() {
    let app = TestApp::spawn().await;
    let (_admin_a, body_a) = app.register_admin("admin-a@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let (_admin_b, body_b) = app.register_admin("admin-b@example.com", "OrgB").await;
    let code_b = body_b["current_org"]["code"].as_str().unwrap().to_string();

    // Identity that submits to both Orgs in sequence.
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
    bystander
        .post(app.url("/me/join-requests"))
        .json(&json!({ "org_code": code_a }))
        .send()
        .await
        .unwrap();
    bystander
        .post(app.url("/me/join-requests"))
        .json(&json!({ "org_code": code_b }))
        .send()
        .await
        .unwrap();

    let listed: Value = bystander
        .get(app.url("/me/join-requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr = listed.as_array().unwrap();
    // Plus the user's own org from register_create — that's a membership,
    // not a join_request, so it doesn't appear here.
    assert_eq!(arr.len(), 2);
    // Newest-first: OrgB then OrgA.
    assert_eq!(arr[0]["org"]["code"], code_b);
    assert_eq!(arr[1]["org"]["code"], code_a);
}
