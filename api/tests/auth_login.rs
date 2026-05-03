mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn register_user(app: &TestApp, email: &str, password: &str) {
    let resp = app
        .client
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": email,
            "password": password,
            "org_name": "Acme",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn login_happy_path() {
    let app = TestApp::spawn().await;
    register_user(&app, "founder@example.com", "hunter2hunter2").await;

    // Use a fresh client to ensure we are exercising login, not the register cookie.
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let resp = client
        .post(app.url("/auth/login"))
        .json(&json!({
            "email": "founder@example.com",
            "password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.cookies().any(|c| c.name() == "argus_session"));

    let me = client.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::OK);
}

#[tokio::test]
async fn login_wrong_password_returns_invalid_credentials() {
    let app = TestApp::spawn().await;
    register_user(&app, "founder@example.com", "hunter2hunter2").await;

    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(app.url("/auth/login"))
        .json(&json!({
            "email": "founder@example.com",
            "password": "wrongwrongwrong",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn login_unknown_email_returns_invalid_credentials() {
    let app = TestApp::spawn().await;
    register_user(&app, "founder@example.com", "hunter2hunter2").await;

    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(app.url("/auth/login"))
        .json(&json!({
            "email": "ghost@example.com",
            "password": "anythinganything",
        }))
        .send()
        .await
        .unwrap();
    // Same generic error: must not leak whether the email exists.
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");
}
