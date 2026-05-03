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

async fn register_member(
    app: &TestApp,
    email: &str,
    org_code: &str,
) -> (reqwest::Client, reqwest::Response) {
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
    (client, resp)
}

#[tokio::test]
async fn rejoin_during_cooldown_is_blocked() {
    let app = TestApp::spawn().await;
    let (admin, body) = register_admin(&app, "founder@example.com", "Acme").await;
    let code = body["org"]["code"].as_str().unwrap().to_string();

    // Member joins, then admin removes them — marker is created.
    let (_member, join1) = register_member(&app, "transient@example.com", &code).await;
    assert_eq!(join1.status(), StatusCode::OK);
    let join1_body: Value = join1.json().await.unwrap();
    let member_id = join1_body["user"]["id"].as_str().unwrap().to_string();

    let removed = admin
        .delete(app.url(&format!("/dashboard-users/{member_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);

    // Same email tries to rejoin — should be blocked.
    let (_again, retry) = register_member(&app, "transient@example.com", &code).await;
    assert_eq!(retry.status(), StatusCode::CONFLICT);
    let err: Value = retry.json().await.unwrap();
    assert_eq!(err["error"]["code"], "EMAIL_IN_COOLDOWN");
}

#[tokio::test]
async fn rejoin_to_different_org_during_cooldown_succeeds() {
    let app = TestApp::spawn().await;
    let (admin_a, body_a) = register_admin(&app, "alpha-owner@example.com", "OrgA").await;
    let (_admin_b, body_b) = register_admin(&app, "beta-owner@example.com", "OrgB").await;

    let code_a = body_a["org"]["code"].as_str().unwrap().to_string();
    let code_b = body_b["org"]["code"].as_str().unwrap().to_string();

    // Member joins OrgA, then is kicked → cooldown for (OrgA, member email).
    let (_member, join_a) = register_member(&app, "wanderer@example.com", &code_a).await;
    assert_eq!(join_a.status(), StatusCode::OK);
    let join_a_body: Value = join_a.json().await.unwrap();
    let member_id = join_a_body["user"]["id"].as_str().unwrap().to_string();

    let kick = admin_a
        .delete(app.url(&format!("/dashboard-users/{member_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(kick.status(), StatusCode::NO_CONTENT);

    // Same email joins OrgB — different org, so no cooldown applies.
    let (_w2, join_b) = register_member(&app, "wanderer@example.com", &code_b).await;
    assert_eq!(join_b.status(), StatusCode::OK);
}

#[tokio::test]
async fn rejoin_with_mixed_case_email_matches_lowercased_marker() {
    let app = TestApp::spawn().await;
    let (admin, body) = register_admin(&app, "founder@example.com", "Acme").await;
    let code = body["org"]["code"].as_str().unwrap().to_string();

    let (_member, join1) = register_member(&app, "transient@example.com", &code).await;
    assert_eq!(join1.status(), StatusCode::OK);
    let join1_body: Value = join1.json().await.unwrap();
    let member_id = join1_body["user"]["id"].as_str().unwrap().to_string();

    let removed = admin
        .delete(app.url(&format!("/dashboard-users/{member_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);

    // Mixed-case rejoin must hit the same marker.
    let (_retry, retry_resp) =
        register_member(&app, "Transient@Example.COM", &code).await;
    assert_eq!(retry_resp.status(), StatusCode::CONFLICT);
    let err: Value = retry_resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "EMAIL_IN_COOLDOWN");
}

#[tokio::test]
async fn rejoin_after_admin_clears_cooldown_succeeds() {
    let app = TestApp::spawn().await;
    let (admin, body) = register_admin(&app, "founder@example.com", "Acme").await;
    let code = body["org"]["code"].as_str().unwrap().to_string();

    let (_member, join1) = register_member(&app, "transient@example.com", &code).await;
    assert_eq!(join1.status(), StatusCode::OK);
    let join1_body: Value = join1.json().await.unwrap();
    let member_id = join1_body["user"]["id"].as_str().unwrap().to_string();

    admin
        .delete(app.url(&format!("/dashboard-users/{member_id}")))
        .send()
        .await
        .unwrap();

    // Admin clears the cooldown.
    let cleared = admin
        .delete(app.url("/dashboard-users/cooldowns/transient@example.com"))
        .send()
        .await
        .unwrap();
    assert_eq!(cleared.status(), StatusCode::NO_CONTENT);

    // Now rejoin succeeds.
    let (_again, retry) = register_member(&app, "transient@example.com", &code).await;
    assert_eq!(retry.status(), StatusCode::OK);
}
