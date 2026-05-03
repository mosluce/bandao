mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn seed_logged_in(app: &TestApp) -> (reqwest::Client, String, String, String) {
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
    let create = app.create_app_user(&admin, "alice", "Alice Chen").await;
    let initial_password = create["initial_password"].as_str().unwrap().to_string();
    let (client, login) = app.app_login(&org_code, "alice", &initial_password).await;
    let token = login["token"].as_str().unwrap().to_string();
    (client, token, initial_password, org_code)
}

#[tokio::test]
async fn forced_change_clears_flag_and_keeps_token_alive() {
    let app = TestApp::spawn().await;
    let (_client, token, initial_password, _org_code) = seed_logged_in(&app).await;

    // Sanity: initial flag is true.
    let me: Value = app
        .fresh_client()
        .get(app.url("/app/me"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["needs_password_change"], true);

    // Change password using the (still-valid) bearer token.
    let resp = app
        .fresh_client()
        .post(app.url("/app/me/password"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "current_password": initial_password,
            "new_password": "newhunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Flag is cleared; same token still works.
    let me: Value = app
        .fresh_client()
        .get(app.url("/app/me"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["needs_password_change"], false);
}

#[tokio::test]
async fn voluntary_change_works_after_flag_already_cleared() {
    let app = TestApp::spawn().await;
    let (_client, token, initial_password, _org_code) = seed_logged_in(&app).await;

    // First change clears the flag.
    let resp = app
        .fresh_client()
        .post(app.url("/app/me/password"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "current_password": initial_password,
            "new_password": "first-new-pw",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Second voluntary change still works (flag is already clear).
    let resp = app
        .fresh_client()
        .post(app.url("/app/me/password"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "current_password": "first-new-pw",
            "new_password": "second-new-pw",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn wrong_current_password_returns_invalid_password() {
    let app = TestApp::spawn().await;
    let (_client, token, _pw, _org_code) = seed_logged_in(&app).await;

    let resp = app
        .fresh_client()
        .post(app.url("/app/me/password"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "current_password": "WRONGWRONGWRONG",
            "new_password": "abcdefgh1",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_PASSWORD");
}

#[tokio::test]
async fn too_short_new_password_returns_validation() {
    let app = TestApp::spawn().await;
    let (_client, token, initial_password, _org_code) = seed_logged_in(&app).await;

    let resp = app
        .fresh_client()
        .post(app.url("/app/me/password"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "current_password": initial_password,
            "new_password": "short1",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "VALIDATION");
}

#[tokio::test]
async fn change_password_works_while_needs_password_change_is_set() {
    let app = TestApp::spawn().await;
    let (_client, token, initial_password, _org_code) = seed_logged_in(&app).await;
    // Initial state has the flag set; ensure the endpoint is reachable
    // (i.e. the 423 gate does NOT apply to /app/me/password).
    let resp = app
        .fresh_client()
        .post(app.url("/app/me/password"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "current_password": initial_password,
            "new_password": "abcdefgh1",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}
