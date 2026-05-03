mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::Value;

#[tokio::test]
async fn app_me_returns_user_org_and_flag_with_needs_password_change_set() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let org_id = body["current_org"]["id"].as_str().unwrap().to_string();
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
    let create = app.create_app_user(&admin, "alice", "Alice Chen").await;
    let initial_password = create["initial_password"].as_str().unwrap().to_string();

    let (_client, login) = app.app_login(&org_code, "alice", &initial_password).await;
    let token = login["token"].as_str().unwrap().to_string();

    let resp = app
        .fresh_client()
        .get(app.url("/app/me"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let me: Value = resp.json().await.unwrap();
    assert_eq!(me["user"]["username"], "alice");
    assert_eq!(me["user"]["display_name"], "Alice Chen");
    assert_eq!(me["org"]["id"], org_id);
    assert_eq!(me["needs_password_change"], true);
}

#[tokio::test]
async fn app_me_unknown_token_returns_401() {
    let app = TestApp::spawn().await;

    let resp = app
        .fresh_client()
        .get(app.url("/app/me"))
        .header("Authorization", "Bearer total-nonsense-token")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn app_me_missing_authorization_header_returns_401() {
    let app = TestApp::spawn().await;

    let resp = app
        .fresh_client()
        .get(app.url("/app/me"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
