mod common;

use bson::oid::ObjectId;
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
async fn non_owner_member_can_self_leave() {
    let app = TestApp::spawn().await;
    let (_admin, admin_body) = register_admin(&app, "founder@example.com", "Acme").await;
    let code = admin_body["org"]["code"].as_str().unwrap().to_string();
    let org_id = ObjectId::parse_str(admin_body["org"]["id"].as_str().unwrap()).unwrap();

    let (member, member_body) = register_member(&app, "member@example.com", &code).await;
    let member_id = ObjectId::parse_str(member_body["user"]["id"].as_str().unwrap()).unwrap();

    let resp = member
        .post(app.url("/me/leave"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // User row gone.
    assert!(
        app.db()
            .dashboard_users
            .find_by_id(member_id)
            .await
            .unwrap()
            .is_none()
    );

    // Marker present with kind=left.
    let marker = app
        .db()
        .removed_memberships
        .find(org_id, "member@example.com")
        .await
        .unwrap()
        .expect("marker should exist");
    use argus_api::domain::RemovalKind;
    assert!(matches!(marker.removal_kind, RemovalKind::Left));

    // Subsequent /me with the same client returns 401 (cookie cleared / session deleted).
    let me = member.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn owner_cannot_self_leave() {
    let app = TestApp::spawn().await;
    let (admin, _) = register_admin(&app, "founder@example.com", "Acme").await;

    let resp = admin
        .post(app.url("/me/leave"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "OWNER_PROTECTED");
}
