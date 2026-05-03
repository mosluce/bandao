mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn register_admin(app: &TestApp, email: &str, org_name: &str) -> (reqwest::Client, Value) {
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
    (client, body)
}

async fn register_member(app: &TestApp, email: &str, org_code: &str) -> (reqwest::Client, Value) {
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
    let body: Value = resp.json().await.unwrap();
    (client, body)
}

#[tokio::test]
async fn list_cooldowns_returns_only_callers_org() {
    let app = TestApp::spawn().await;

    // OrgA: kick a member.
    let (admin_a, body_a) = register_admin(&app, "alpha-owner@example.com", "OrgA").await;
    let code_a = body_a["org"]["code"].as_str().unwrap().to_string();
    let (_m_a, member_a) = register_member(&app, "transient-a@example.com", &code_a).await;
    let id_a = member_a["user"]["id"].as_str().unwrap().to_string();
    admin_a
        .delete(app.url(&format!("/dashboard-users/{id_a}")))
        .send()
        .await
        .unwrap();

    // OrgB: kick a member.
    let (admin_b, body_b) = register_admin(&app, "beta-owner@example.com", "OrgB").await;
    let code_b = body_b["org"]["code"].as_str().unwrap().to_string();
    let (_m_b, member_b) = register_member(&app, "transient-b@example.com", &code_b).await;
    let id_b = member_b["user"]["id"].as_str().unwrap().to_string();
    admin_b
        .delete(app.url(&format!("/dashboard-users/{id_b}")))
        .send()
        .await
        .unwrap();

    // Admin A only sees A's marker.
    let resp = admin_a
        .get(app.url("/dashboard-users/cooldowns"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list: Value = resp.json().await.unwrap();
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["email"], "transient-a@example.com");
    assert_eq!(arr[0]["removal_kind"], "kicked");
}

#[tokio::test]
async fn clear_cooldown_for_missing_marker_returns_204() {
    let app = TestApp::spawn().await;
    let (admin, _) = register_admin(&app, "founder@example.com", "Acme").await;

    let resp = admin
        .delete(app.url("/dashboard-users/cooldowns/never-existed@example.com"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn member_cannot_call_cooldown_endpoints() {
    let app = TestApp::spawn().await;
    let (_admin, admin_body) = register_admin(&app, "founder@example.com", "Acme").await;
    let code = admin_body["org"]["code"].as_str().unwrap().to_string();
    let (member, _member_body) = register_member(&app, "member@example.com", &code).await;

    let list = member
        .get(app.url("/dashboard-users/cooldowns"))
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::FORBIDDEN);

    let clear = member
        .delete(app.url("/dashboard-users/cooldowns/anybody@example.com"))
        .send()
        .await
        .unwrap();
    assert_eq!(clear.status(), StatusCode::FORBIDDEN);
}
