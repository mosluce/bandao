mod common;

use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn fail_app_login(app: &TestApp, org_code: &str, username: &str) -> reqwest::Response {
    app.fresh_client()
        .post(app.url("/app/auth/login"))
        .json(&json!({
            "org_code": org_code,
            "username": username,
            "password": "wrongwrongwrong",
        }))
        .send()
        .await
        .unwrap()
}

/// Seed an admin + one internal AppUser, returning
/// `(admin_client, org_code, app_user_id, initial_password)`.
async fn seed_internal_app_user(app: &TestApp) -> (reqwest::Client, String, String, String) {
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
    let create_body = app.create_app_user(&admin, "alice", "Alice Chen").await;
    let app_user_id = create_body["user"]["id"].as_str().unwrap().to_string();
    let initial_password = create_body["initial_password"]
        .as_str()
        .unwrap()
        .to_string();
    (admin, org_code, app_user_id, initial_password)
}

#[tokio::test]
async fn internal_app_user_locks_after_threshold_failures() {
    let app = TestApp::spawn().await;
    let (_admin, org_code, app_user_id, _pw) = seed_internal_app_user(&app).await;
    let id = ObjectId::parse_str(&app_user_id).unwrap();

    for _ in 0..2 {
        let resp = fail_app_login(&app, &org_code, "alice").await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
    let user = app.db().app_users.find_by_id(id).await.unwrap().unwrap();
    assert_eq!(user.failed_login_attempts, 2);
    assert!(user.locked_until.is_none());

    let resp = fail_app_login(&app, &org_code, "alice").await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");

    let user = app.db().app_users.find_by_id(id).await.unwrap().unwrap();
    assert_eq!(user.failed_login_attempts, 3);
    assert!(user.locked_until.is_some());
}

#[tokio::test]
async fn locked_internal_app_user_rejects_correct_password_without_extending_lock() {
    let app = TestApp::spawn().await;
    let (_admin, org_code, app_user_id, initial_password) = seed_internal_app_user(&app).await;
    let id = ObjectId::parse_str(&app_user_id).unwrap();

    for _ in 0..3 {
        fail_app_login(&app, &org_code, "alice").await;
    }
    let locked = app.db().app_users.find_by_id(id).await.unwrap().unwrap();
    let locked_until = locked.locked_until.expect("should be locked");

    let resp = app
        .fresh_client()
        .post(app.url("/app/auth/login"))
        .json(&json!({
            "org_code": org_code,
            "username": "alice",
            "password": initial_password,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let still_locked = app.db().app_users.find_by_id(id).await.unwrap().unwrap();
    assert_eq!(still_locked.failed_login_attempts, 3);
    assert_eq!(still_locked.locked_until, Some(locked_until));
}

#[tokio::test]
async fn successful_app_login_resets_the_counter() {
    let app = TestApp::spawn().await;
    let (_admin, org_code, app_user_id, initial_password) = seed_internal_app_user(&app).await;
    let id = ObjectId::parse_str(&app_user_id).unwrap();

    fail_app_login(&app, &org_code, "alice").await;
    fail_app_login(&app, &org_code, "alice").await;

    let (_client, _body) = app.app_login(&org_code, "alice", &initial_password).await;

    let user = app.db().app_users.find_by_id(id).await.unwrap().unwrap();
    assert_eq!(user.failed_login_attempts, 0);
    assert!(user.locked_until.is_none());
}

#[tokio::test]
async fn admin_can_unlock_a_locked_app_user() {
    let app = TestApp::spawn().await;
    let (admin, org_code, app_user_id, initial_password) = seed_internal_app_user(&app).await;

    for _ in 0..3 {
        fail_app_login(&app, &org_code, "alice").await;
    }

    let unlock = admin
        .post(app.url(&format!("/app-users/{app_user_id}/unlock")))
        .send()
        .await
        .unwrap();
    assert_eq!(unlock.status(), StatusCode::NO_CONTENT);

    let (_client, _body) = app.app_login(&org_code, "alice", &initial_password).await;
}

#[tokio::test]
async fn unlocking_an_app_user_that_isnt_locked_is_a_no_op() {
    let app = TestApp::spawn().await;
    let (admin, _org_code, app_user_id, _pw) = seed_internal_app_user(&app).await;

    let unlock = admin
        .post(app.url(&format!("/app-users/{app_user_id}/unlock")))
        .send()
        .await
        .unwrap();
    assert_eq!(unlock.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn cross_org_app_user_unlock_rejected() {
    let app = TestApp::spawn().await;
    let (_admin_a, _org_code_a, app_user_id, _pw) = seed_internal_app_user(&app).await;

    let (admin_b, _body_b) = app.register_admin("b@example.com", "OrgB").await;
    let unlock = admin_b
        .post(app.url(&format!("/app-users/{app_user_id}/unlock")))
        .send()
        .await
        .unwrap();
    assert_eq!(unlock.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn member_cannot_unlock_app_user() {
    let app = TestApp::spawn().await;
    let (admin, org_code, app_user_id, _pw) = seed_internal_app_user(&app).await;
    let (member_client, _member_body) = app
        .register_member(&admin, "member@example.com", &org_code)
        .await;

    let unlock = member_client
        .post(app.url(&format!("/app-users/{app_user_id}/unlock")))
        .send()
        .await
        .unwrap();
    assert_eq!(unlock.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn app_users_list_reports_is_locked_without_leaking_raw_fields() {
    let app = TestApp::spawn().await;
    let (admin, org_code, _app_user_id, _pw) = seed_internal_app_user(&app).await;
    for _ in 0..3 {
        fail_app_login(&app, &org_code, "alice").await;
    }

    let list: Value = admin
        .get(app.url("/app-users"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let entries = list.as_array().unwrap();
    let alice = entries.iter().find(|u| u["username"] == "alice").unwrap();
    assert_eq!(alice["is_locked"], true);
    for entry in entries {
        assert!(entry.get("failed_login_attempts").is_none());
        assert!(entry.get("locked_until").is_none());
    }
}

#[tokio::test]
async fn external_auth_app_users_accumulate_no_lockout_state() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;

    // Point the Org at an external database that is never actually reached
    // (closed local port) — enough to prove no local lockout bookkeeping
    // happens for external_db Orgs, without booting a real MSSQL container.
    let configure = admin
        .post(app.url("/orgs/me/external-auth"))
        .json(&json!({
            "auth_source": "external_db",
            "external_auth": {
                "driver": "mssql",
                "host": "127.0.0.1",
                "port": 39999u16,
                "database": "staff",
                "username": "sa",
                "password": "unused",
                "query": "SELECT emp_id AS key_col, name AS display_col FROM staff WHERE acct = @account AND pwd = @password",
                "key_col": "key_col",
                "display_col": "display_col",
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(configure.status(), StatusCode::OK);
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();

    for _ in 0..5 {
        let resp = app
            .fresh_client()
            .post(app.url("/app/auth/login"))
            .json(&json!({
                "org_code": org_code,
                "username": "wang",
                "password": "secret",
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let err: Value = resp.json().await.unwrap();
        assert_eq!(err["error"]["code"], "EXTERNAL_AUTH_UNAVAILABLE");
    }

    // No shadow AppUser was ever provisioned, so there is nothing to lock —
    // the exemption holds trivially because the internal lockout path never
    // runs for this Org's auth_source.
    let users = app
        .db()
        .app_users
        .list_by_org(ObjectId::parse_str(body["current_org"]["id"].as_str().unwrap()).unwrap())
        .await
        .unwrap();
    assert!(users.is_empty());
}
