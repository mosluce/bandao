//! `POST /auth/forgot-password` / `POST /auth/reset-password` — see the
//! `dashboard-auth` spec's password-reset requirements.

mod common;

use std::sync::Arc;

use bandao_api::services::email::RecordingEmailSender;
use bson::oid::ObjectId;
use bson::{DateTime, doc};
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

/// Pull the raw reset token out of the recorded email's HTML body — the link
/// is `.../reset-password?token=<raw>"`.
fn extract_token(html_body: &str) -> String {
    let marker = "token=";
    let start = html_body.find(marker).expect("no token= in email body") + marker.len();
    let rest = &html_body[start..];
    let end = rest.find('"').expect("unterminated href attribute");
    rest[..end].to_string()
}

async fn spawn_with_recorder() -> (TestApp, Arc<RecordingEmailSender>) {
    let recorder = Arc::new(RecordingEmailSender::default());
    let app = TestApp::spawn_with_email_sender(recorder.clone()).await;
    (app, recorder)
}

async fn backdate_token_created_at(app: &TestApp, user_id: ObjectId, seconds_ago: i64) {
    let past = DateTime::from_millis(DateTime::now().timestamp_millis() - seconds_ago * 1000);
    app.state
        .db
        .database
        .collection::<bson::Document>("password_reset_tokens")
        .update_one(
            doc! { "user_id": user_id },
            doc! { "$set": { "created_at": past } },
        )
        .await
        .unwrap();
}

async fn expire_token(app: &TestApp, token_hash: &str) {
    let past = DateTime::from_millis(DateTime::now().timestamp_millis() - 1000);
    app.state
        .db
        .database
        .collection::<bson::Document>("password_reset_tokens")
        .update_one(
            doc! { "token_hash": token_hash },
            doc! { "$set": { "expires_at": past } },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn existing_email_creates_token_and_sends_email() {
    let (app, recorder) = spawn_with_recorder().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let user_id = ObjectId::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();

    let resp = app
        .fresh_client()
        .post(app.url("/auth/forgot-password"))
        .json(&json!({ "email": "admin@example.com" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let token_row = app
        .db()
        .password_reset_tokens
        .find_latest_for_user(user_id)
        .await
        .unwrap();
    assert!(token_row.is_some(), "expected a token row to be created");

    let sent = recorder.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, "admin@example.com");
    assert!(sent[0].html_body.contains("token="));
}

#[tokio::test]
async fn nonexistent_email_returns_identical_204_with_no_token_or_email() {
    let (app, recorder) = spawn_with_recorder().await;

    let resp = app
        .fresh_client()
        .post(app.url("/auth/forgot-password"))
        .json(&json!({ "email": "nobody@example.com" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(recorder.sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn repeated_request_within_cooldown_does_not_issue_a_second_token() {
    let (app, recorder) = spawn_with_recorder().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let user_id = ObjectId::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();

    for _ in 0..2 {
        let resp = app
            .fresh_client()
            .post(app.url("/auth/forgot-password"))
            .json(&json!({ "email": "admin@example.com" }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    assert_eq!(
        recorder.sent.lock().unwrap().len(),
        1,
        "second request within the cooldown window must not send another email"
    );

    // Only one token row exists — the second request didn't insert another.
    let count = app
        .state
        .db
        .database
        .collection::<bson::Document>("password_reset_tokens")
        .count_documents(doc! { "user_id": user_id })
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn request_after_cooldown_window_issues_a_new_token() {
    let (app, recorder) = spawn_with_recorder().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let user_id = ObjectId::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();

    let resp = app
        .fresh_client()
        .post(app.url("/auth/forgot-password"))
        .json(&json!({ "email": "admin@example.com" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Backdate the just-issued token past the 60s cooldown window.
    backdate_token_created_at(&app, user_id, 61).await;

    let resp = app
        .fresh_client()
        .post(app.url("/auth/forgot-password"))
        .json(&json!({ "email": "admin@example.com" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    assert_eq!(recorder.sent.lock().unwrap().len(), 2);
    let count = app
        .state
        .db
        .database
        .collection::<bson::Document>("password_reset_tokens")
        .count_documents(doc! { "user_id": user_id })
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn valid_token_resets_password_and_kills_existing_sessions() {
    let (app, recorder) = spawn_with_recorder().await;
    let (admin, _body) = app.register_admin("admin@example.com", "Acme").await;

    // The pre-reset session must stop working after the reset.
    let me_before = admin.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me_before.status(), StatusCode::OK);

    app.fresh_client()
        .post(app.url("/auth/forgot-password"))
        .json(&json!({ "email": "admin@example.com" }))
        .send()
        .await
        .unwrap();
    let token = {
        let sent = recorder.sent.lock().unwrap();
        extract_token(&sent[0].html_body)
    };

    let resp = app
        .fresh_client()
        .post(app.url("/auth/reset-password"))
        .json(&json!({ "token": token, "new_password": "brandnewpassword123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Old session is dead.
    let me_after = admin.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me_after.status(), StatusCode::UNAUTHORIZED);

    // Old password no longer works; new one does.
    let old_login = app
        .fresh_client()
        .post(app.url("/auth/login"))
        .json(&json!({ "email": "admin@example.com", "password": "hunter2hunter2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);

    let new_login = app
        .fresh_client()
        .post(app.url("/auth/login"))
        .json(&json!({ "email": "admin@example.com", "password": "brandnewpassword123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(new_login.status(), StatusCode::OK);
}

#[tokio::test]
async fn expired_token_is_rejected() {
    let (app, recorder) = spawn_with_recorder().await;
    app.register_admin("admin@example.com", "Acme").await;

    app.fresh_client()
        .post(app.url("/auth/forgot-password"))
        .json(&json!({ "email": "admin@example.com" }))
        .send()
        .await
        .unwrap();
    let token = {
        let sent = recorder.sent.lock().unwrap();
        extract_token(&sent[0].html_body)
    };
    let token_hash = bandao_api::auth::api_token::hash_token(&token);
    expire_token(&app, &token_hash).await;

    let resp = app
        .fresh_client()
        .post(app.url("/auth/reset-password"))
        .json(&json!({ "token": token, "new_password": "brandnewpassword123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "INVALID_RESET_TOKEN");
}

#[tokio::test]
async fn already_used_token_is_rejected_on_replay() {
    let (app, recorder) = spawn_with_recorder().await;
    app.register_admin("admin@example.com", "Acme").await;

    app.fresh_client()
        .post(app.url("/auth/forgot-password"))
        .json(&json!({ "email": "admin@example.com" }))
        .send()
        .await
        .unwrap();
    let token = {
        let sent = recorder.sent.lock().unwrap();
        extract_token(&sent[0].html_body)
    };

    let first = app
        .fresh_client()
        .post(app.url("/auth/reset-password"))
        .json(&json!({ "token": token, "new_password": "brandnewpassword123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::NO_CONTENT);

    let second = app
        .fresh_client()
        .post(app.url("/auth/reset-password"))
        .json(&json!({ "token": token, "new_password": "yetanotherpassword456" }))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::BAD_REQUEST);
    let body: Value = second.json().await.unwrap();
    assert_eq!(body["error"]["code"], "INVALID_RESET_TOKEN");
}

#[tokio::test]
async fn unknown_token_is_rejected() {
    let app = TestApp::spawn().await;

    let resp = app
        .fresh_client()
        .post(app.url("/auth/reset-password"))
        .json(&json!({ "token": "not-a-real-token", "new_password": "brandnewpassword123" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "INVALID_RESET_TOKEN");
}

#[tokio::test]
async fn reset_password_rejects_too_short_new_password() {
    let (app, recorder) = spawn_with_recorder().await;
    app.register_admin("admin@example.com", "Acme").await;

    app.fresh_client()
        .post(app.url("/auth/forgot-password"))
        .json(&json!({ "email": "admin@example.com" }))
        .send()
        .await
        .unwrap();
    let token = {
        let sent = recorder.sent.lock().unwrap();
        extract_token(&sent[0].html_body)
    };

    let resp = app
        .fresh_client()
        .post(app.url("/auth/reset-password"))
        .json(&json!({ "token": token, "new_password": "short" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "VALIDATION");
}
