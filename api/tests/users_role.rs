mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn admin_promotes_member_to_admin() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (_member, member_body) = app
        .register_member(&admin, "member@example.com", &code)
        .await;
    let member_id = member_body["user"]["id"].as_str().unwrap().to_string();

    let resp = admin
        .patch(app.url(&format!("/dashboard-users/{member_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["role"], "admin");
    assert_eq!(body["id"], member_id);
}

#[tokio::test]
async fn member_cannot_change_roles() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (member, member_body) = app
        .register_member(&admin, "member@example.com", &code)
        .await;
    let member_id = member_body["user"]["id"].as_str().unwrap().to_string();

    let resp = member
        .patch(app.url(&format!("/dashboard-users/{member_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cross_org_target_returns_not_found() {
    let app = TestApp::spawn().await;
    let (admin_a, _) = app.register_admin("alpha@example.com", "OrgA").await;
    let (_admin_b, body_b) = app.register_admin("beta@example.com", "OrgB").await;
    // Identity B has no membership in OrgA → cross-Org probe collapses to NOT_FOUND.
    let outsider_id = body_b["user"]["id"].as_str().unwrap().to_string();

    let resp = admin_a
        .patch(app.url(&format!("/dashboard-users/{outsider_id}/role")))
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn demoting_owner_is_rejected() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let owner_id = body["user"]["id"].as_str().unwrap().to_string();

    let resp = admin
        .patch(app.url(&format!("/dashboard-users/{owner_id}/role")))
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "OWNER_PROTECTED");
}

#[tokio::test]
async fn demoting_non_owner_admin_succeeds() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (_member, member_body) = app
        .register_member(&admin, "member@example.com", &code)
        .await;
    let member_id = member_body["user"]["id"].as_str().unwrap().to_string();

    // Promote first.
    let promote = admin
        .patch(app.url(&format!("/dashboard-users/{member_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(promote.status(), StatusCode::OK);

    // Demote the non-owner admin: must succeed (owner remains admin).
    let demote = admin
        .patch(app.url(&format!("/dashboard-users/{member_id}/role")))
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap();
    assert_eq!(demote.status(), StatusCode::OK);
    let body: Value = demote.json().await.unwrap();
    assert_eq!(body["role"], "member");
}

#[tokio::test]
async fn promoting_owner_is_a_noop() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let owner_id = body["user"]["id"].as_str().unwrap().to_string();

    let resp = admin
        .patch(app.url(&format!("/dashboard-users/{owner_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["role"], "admin");
}
