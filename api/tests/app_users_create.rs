mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

const INITIAL_PASSWORD_RE_LEN: usize = 12;

fn assert_initial_password_format(pw: &str) {
    assert_eq!(
        pw.chars().count(),
        INITIAL_PASSWORD_RE_LEN,
        "initial_password length should be {INITIAL_PASSWORD_RE_LEN}: got {pw}"
    );
    let allowed: &str = "23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    assert!(
        pw.chars().all(|c| allowed.contains(c)),
        "initial_password contains chars outside the alphabet: {pw}"
    );
}

#[tokio::test]
async fn admin_creates_app_user_returns_initial_password_once() {
    let app = TestApp::spawn().await;
    let (admin, _body) = app.register_admin("admin@example.com", "Acme").await;

    let resp = admin
        .post(app.url("/app-users"))
        .json(&json!({ "username": "alice123", "display_name": "Alice Chen" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = resp.json().await.unwrap();

    let pw = body["initial_password"].as_str().expect("initial_password");
    assert_initial_password_format(pw);

    let user = &body["user"];
    assert_eq!(user["username"], "alice123");
    assert_eq!(user["display_name"], "Alice Chen");
    assert_eq!(user["status"], "active");
    assert_eq!(user["needs_password_change"], true);
    assert!(
        user["last_login_at"].is_null() || !user.as_object().unwrap().contains_key("last_login_at")
    );

    // The matching `checkin_user_status` row must exist with status=off_duty,
    // ready for the AppUser's first clock-in.
    let app_user_id = bson::oid::ObjectId::parse_str(user["id"].as_str().unwrap()).unwrap();
    let status_row = app
        .db()
        .checkin_user_status
        .find(app_user_id)
        .await
        .unwrap()
        .expect("checkin_user_status row should exist after AppUser create");
    assert_eq!(
        bson::to_bson(&status_row.status).unwrap().as_str(),
        Some("off_duty"),
    );
    assert!(status_row.last_event_id.is_none());
    assert!(status_row.current_shift_started_at.is_none());
}

#[tokio::test]
async fn member_cannot_create_app_user() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let (member, _) = app
        .register_member(&admin, "member@example.com", &code)
        .await;

    let resp = member
        .post(app.url("/app-users"))
        .json(&json!({ "username": "alice", "display_name": "Alice" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn invalid_username_format_is_rejected() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;

    for bad in ["a", &"x".repeat(33), "alice@home", "hi there", "*-bad-*"] {
        let resp = admin
            .post(app.url("/app-users"))
            .json(&json!({ "username": bad, "display_name": "Whoever" }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "expected 400 for bad username `{bad}`"
        );
        let err: Value = resp.json().await.unwrap();
        assert_eq!(err["error"]["code"], "INVALID_USERNAME_FORMAT");
    }
}

#[tokio::test]
async fn case_insensitive_duplicate_username_is_taken() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;

    let _ = app.create_app_user(&admin, "alice", "First Alice").await;

    let resp = admin
        .post(app.url("/app-users"))
        .json(&json!({ "username": "ALICE", "display_name": "Second Alice" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "USERNAME_TAKEN");
}

#[tokio::test]
async fn same_username_in_different_org_is_allowed() {
    let app = TestApp::spawn().await;

    let (admin_a, _) = app.register_admin("a@example.com", "OrgA").await;
    let _ = app.create_app_user(&admin_a, "alice", "Alice in A").await;

    let (admin_b, _) = app.register_admin("b@example.com", "OrgB").await;
    let body = app.create_app_user(&admin_b, "alice", "Alice in B").await;
    assert_eq!(body["user"]["username"], "alice");
    assert_eq!(body["user"]["display_name"], "Alice in B");
}
