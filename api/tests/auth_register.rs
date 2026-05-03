mod common;

use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn register_create_mode_happy_path() {
    let app = TestApp::spawn().await;

    let resp = app
        .client
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": "founder@example.com",
            "password": "hunter2hunter2",
            "org_name": "Acme",
        }))
        .send()
        .await
        .expect("send register");

    assert_eq!(resp.status(), StatusCode::OK);
    let cookies: Vec<_> = resp.cookies().collect();
    assert!(
        cookies.iter().any(|c| c.name() == "argus_session"),
        "expected argus_session cookie, got {cookies:?}"
    );

    let body: Value = resp.json().await.expect("json body");
    assert_eq!(body["user"]["email"], "founder@example.com");
    // Identity payload no longer carries a role; role lives on memberships.
    assert!(body["user"].get("role").is_none() || body["user"]["role"].is_null());
    assert_eq!(body["current_org"]["name"], "Acme");
    assert_eq!(body["role"], "admin");
    let code = body["current_org"]["code"].as_str().expect("current_org.code");
    assert_eq!(code.chars().count(), 10);
    // Creator becomes the owner.
    assert_eq!(body["current_org"]["owner_id"], body["user"]["id"]);

    // memberships array is exactly one (org=current_org, role=admin).
    let memberships = body["memberships"].as_array().expect("memberships array");
    assert_eq!(memberships.len(), 1);
    assert_eq!(memberships[0]["org"]["id"], body["current_org"]["id"]);
    assert_eq!(memberships[0]["role"], "admin");

    // Membership row exists in the DB.
    let user_id = ObjectId::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();
    let org_id = ObjectId::parse_str(body["current_org"]["id"].as_str().unwrap()).unwrap();
    let m = app
        .db()
        .dashboard_memberships
        .find_by_user_and_org(user_id, org_id)
        .await
        .unwrap()
        .expect("membership row");
    assert!(matches!(m.role, argus_api::domain::Role::Admin));

    // /me returns the same shape.
    let me_resp = app
        .client
        .get(app.url("/me"))
        .send()
        .await
        .expect("send /me");
    assert_eq!(me_resp.status(), StatusCode::OK);
    let me: Value = me_resp.json().await.expect("me json");
    assert_eq!(me["user"]["email"], "founder@example.com");
    assert_eq!(me["current_org"]["code"], code);
    assert_eq!(me["role"], "admin");
}

#[tokio::test]
async fn register_join_mode_happy_path() {
    let app = TestApp::spawn().await;

    // Bootstrap an org with an admin to harvest its code.
    let (_creator, create_body) = app.register_admin("founder@example.com", "Acme").await;
    let org_code = create_body["current_org"]["code"].as_str().unwrap().to_string();
    let org_id = create_body["current_org"]["id"].as_str().unwrap().to_string();

    // Different client = empty cookie jar.
    let (joiner, join_body) = app.register_member("member@example.com", &org_code).await;
    assert_eq!(join_body["user"]["email"], "member@example.com");
    assert_eq!(join_body["role"], "member");
    assert_eq!(join_body["current_org"]["id"], org_id);
    let memberships = join_body["memberships"].as_array().unwrap();
    assert_eq!(memberships.len(), 1);
    assert_eq!(memberships[0]["role"], "member");

    // joiner can hit /me.
    let me = joiner.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::OK);
}

#[tokio::test]
async fn register_rejects_email_taken_create_mode() {
    let app = TestApp::spawn().await;

    let body = json!({
        "mode": "create",
        "email": "dup@example.com",
        "password": "hunter2hunter2",
        "org_name": "FirstOrg",
    });
    let first = app
        .client
        .post(app.url("/auth/register"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    // Strict separation: an existing identity cannot use register, even with
    // mode=create. They must log in and use /me/orgs.
    let second = app
        .fresh_client()
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": "dup@example.com",
            "password": "hunter2hunter2",
            "org_name": "SecondOrg",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
    let err: Value = second.json().await.unwrap();
    assert_eq!(err["error"]["code"], "EMAIL_TAKEN");
}

#[tokio::test]
async fn register_rejects_email_taken_join_mode() {
    let app = TestApp::spawn().await;

    // Bootstrap target org.
    let (_admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"].as_str().unwrap().to_string();

    // Bootstrap a second identity in a separate org.
    let (_other_admin, _) = app.register_admin("dup@example.com", "OtherOrg").await;

    // Try to register with the existing email via mode=join — strictly rejected.
    let resp = app
        .fresh_client()
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "dup@example.com",
            "password": "hunter2hunter2",
            "org_code": code,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "EMAIL_TAKEN");
}

#[tokio::test]
async fn register_join_rejects_bogus_code() {
    let app = TestApp::spawn().await;

    let resp = app
        .client
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "wanderer@example.com",
            "password": "hunter2hunter2",
            "org_code": "ZZZZZZZZZZ",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_ORG_CODE");
}
