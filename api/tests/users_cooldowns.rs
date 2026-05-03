mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::Value;

#[tokio::test]
async fn list_cooldowns_returns_only_callers_org() {
    let app = TestApp::spawn().await;

    // OrgA: kick a member.
    let (admin_a, body_a) = app.register_admin("alpha-owner@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let (_m_a, member_a) = app.register_member("transient-a@example.com", &code_a).await;
    let id_a = member_a["user"]["id"].as_str().unwrap().to_string();
    admin_a
        .delete(app.url(&format!("/dashboard-users/{id_a}")))
        .send()
        .await
        .unwrap();

    // OrgB: kick a member.
    let (admin_b, body_b) = app.register_admin("beta-owner@example.com", "OrgB").await;
    let code_b = body_b["current_org"]["code"].as_str().unwrap().to_string();
    let (_m_b, member_b) = app.register_member("transient-b@example.com", &code_b).await;
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
    let (admin, _) = app.register_admin("founder@example.com", "Acme").await;

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
    let (_admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"].as_str().unwrap().to_string();
    let (member, _member_body) = app.register_member("member@example.com", &code).await;

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
