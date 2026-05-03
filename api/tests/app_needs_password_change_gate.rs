//! 423 gate behavior — `needs_password_change == true` blocks every
//! `/app/*` endpoint EXCEPT the trio (`GET /app/me`, `POST /app/me/password`,
//! `POST /app/auth/logout`). The MVP for this change introduces only those
//! three routes — there's no second gated `/app/*` endpoint to assert
//! against from an integration test.
//!
//! This file therefore covers:
//!   1. The allow-listed trio is reachable while the flag is set.
//!   2. After a successful password change, the flag flips to false (the
//!      gate is "lifted" — future gated routes will not block).
//!
//! The corresponding 423 rejection is exercised at the extractor level by
//! the `RequireAppUser` impl (see unit test
//! `auth::app_extractor::tests::require_app_user_extractor_rejects_when_flag_set`),
//! and the `ApiError::NeedsPasswordChange` variant is wired with status 423
//! and code `NEEDS_PASSWORD_CHANGE`. The first downstream change that adds
//! a real gated `/app/*` endpoint (e.g. checkin) will exercise the wire-level
//! 423 path end-to-end.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn seed_with_flag_set(app: &TestApp) -> (String, String) {
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
    let create = app.create_app_user(&admin, "alice", "Alice Chen").await;
    let initial_password = create["initial_password"].as_str().unwrap().to_string();
    let (_client, login) = app.app_login(&org_code, "alice", &initial_password).await;
    assert_eq!(login["needs_password_change"], true);
    let token = login["token"].as_str().unwrap().to_string();
    (token, initial_password)
}

#[tokio::test]
async fn allow_listed_endpoints_reachable_while_flag_is_set() {
    let app = TestApp::spawn().await;
    let (token, initial_password) = seed_with_flag_set(&app).await;

    // GET /app/me — must succeed.
    let me = app
        .fresh_client()
        .get(app.url("/app/me"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    let me_body: Value = me.json().await.unwrap();
    assert_eq!(me_body["needs_password_change"], true);

    // POST /app/me/password — must succeed (flag-clearing flow).
    let pw = app
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
    assert_eq!(pw.status(), StatusCode::NO_CONTENT);

    // POST /app/auth/logout — must succeed.
    let logout = app
        .fresh_client()
        .post(app.url("/app/auth/logout"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn flag_is_cleared_after_successful_password_change() {
    let app = TestApp::spawn().await;
    let (token, initial_password) = seed_with_flag_set(&app).await;

    // Change password -> flag clears.
    let pw = app
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
    assert_eq!(pw.status(), StatusCode::NO_CONTENT);

    // /app/me reports the cleared flag — i.e. future gated routes (when
    // they exist) will no longer 423 for this caller.
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
