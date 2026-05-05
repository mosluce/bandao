mod common;

use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn reset_returns_new_initial_password_kills_sessions_and_changes_hash() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();

    // Original create + login → produce a session row.
    let create = app.create_app_user(&admin, "alice", "Alice").await;
    let id = create["user"]["id"].as_str().unwrap().to_string();
    let original_password = create["initial_password"].as_str().unwrap().to_string();

    // Change the password to clear `needs_password_change` so the reset
    // flow shows the flag is being deliberately re-set.
    let (_client, login) = app.app_login(&org_code, "alice", &original_password).await;
    let token = login["token"].as_str().unwrap().to_string();
    let pw = app
        .fresh_client()
        .post(app.url("/app/me/password"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "current_password": original_password,
            "new_password": "newhunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(pw.status(), StatusCode::NO_CONTENT);

    // Snapshot the hash before the reset so we can prove it changed.
    let before = app
        .db()
        .app_users
        .find_by_id(ObjectId::parse_str(&id).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert!(!before.needs_password_change);
    let hash_before = before.password_hash.clone();

    // Reset.
    let resp = admin
        .post(app.url(&format!("/app-users/{id}/password-reset")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let new_password = body["initial_password"].as_str().unwrap().to_string();
    assert_eq!(new_password.chars().count(), 12);
    assert_eq!(body["user"]["needs_password_change"], true);

    // Hash differs from before.
    let after = app
        .db()
        .app_users
        .find_by_id(ObjectId::parse_str(&id).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_ne!(after.password_hash, hash_before);
    assert!(after.needs_password_change);

    // Existing token is gone.
    let me = app
        .fresh_client()
        .get(app.url("/app/me"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);

    // Old password no longer works.
    let resp = app
        .fresh_client()
        .post(app.url("/app/auth/login"))
        .json(&json!({
            "org_code": org_code,
            "username": "alice",
            "password": "newhunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // The new initial_password works and `needs_password_change` is true.
    let (_client2, login2) = app.app_login(&org_code, "alice", &new_password).await;
    assert_eq!(login2["needs_password_change"], true);
}

#[tokio::test]
async fn member_cannot_reset_password() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let create = app.create_app_user(&admin, "alice", "Alice").await;
    let id = create["user"]["id"].as_str().unwrap().to_string();

    let (member, _) = app.register_member("member@example.com", &code).await;
    let resp = member
        .post(app.url(&format!("/app-users/{id}/password-reset")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cross_org_reset_returns_not_found() {
    let app = TestApp::spawn().await;
    let (admin_a, _) = app.register_admin("a@example.com", "OrgA").await;
    let (admin_b, _) = app.register_admin("b@example.com", "OrgB").await;
    let create_b = app.create_app_user(&admin_b, "carol", "Carol").await;
    let carol_id = create_b["user"]["id"].as_str().unwrap().to_string();

    let resp = admin_a
        .post(app.url(&format!("/app-users/{carol_id}/password-reset")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
