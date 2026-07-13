mod common;

use bson::oid::ObjectId;
use common::{TestApp, user_id};
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn fail_login(app: &TestApp, email: &str) -> reqwest::Response {
    app.fresh_client()
        .post(app.url("/auth/login"))
        .json(&json!({ "email": email, "password": "wrongwrongwrong" }))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn failed_attempts_increment_and_lock_at_threshold() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let id = ObjectId::parse_str(user_id(&body)).unwrap();

    // Threshold defaults to 3. First two failures stay below it.
    for _ in 0..2 {
        let resp = fail_login(&app, "founder@example.com").await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
    let user = app
        .db()
        .dashboard_users
        .find_by_id(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.failed_login_attempts, 2);
    assert!(user.locked_until.is_none());

    // Third failure crosses the threshold and locks the account.
    let resp = fail_login(&app, "founder@example.com").await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");

    let user = app
        .db()
        .dashboard_users
        .find_by_id(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.failed_login_attempts, 3);
    assert!(user.locked_until.is_some());
}

#[tokio::test]
async fn locked_account_rejects_correct_password_without_extending_lock() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let id = ObjectId::parse_str(user_id(&body)).unwrap();

    for _ in 0..3 {
        fail_login(&app, "founder@example.com").await;
    }
    let locked = app
        .db()
        .dashboard_users
        .find_by_id(id)
        .await
        .unwrap()
        .unwrap();
    let locked_until = locked.locked_until.expect("should be locked");

    // Correct password is rejected identically to a wrong one while locked.
    let resp = app
        .fresh_client()
        .post(app.url("/auth/login"))
        .json(&json!({ "email": "founder@example.com", "password": "hunter2hunter2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");

    // Attempts and lock window are unchanged by the attempt made while locked.
    let still_locked = app
        .db()
        .dashboard_users
        .find_by_id(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still_locked.failed_login_attempts, 3);
    assert_eq!(still_locked.locked_until, Some(locked_until));
}

#[tokio::test]
async fn successful_login_resets_the_counter() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let id = ObjectId::parse_str(user_id(&body)).unwrap();

    // Two failures, then a correct login before hitting the threshold.
    fail_login(&app, "founder@example.com").await;
    fail_login(&app, "founder@example.com").await;
    let (_client, login_body) = app.login("founder@example.com", "hunter2hunter2").await;
    assert_eq!(login_body["role"], "admin");

    let user = app
        .db()
        .dashboard_users
        .find_by_id(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.failed_login_attempts, 0);
    assert!(user.locked_until.is_none());
}

#[tokio::test]
async fn admin_can_unlock_a_locked_account() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (_member_client, member_body) = app
        .register_member(&admin, "member@example.com", &code)
        .await;
    let member_id = user_id(&member_body);

    for _ in 0..3 {
        fail_login(&app, "member@example.com").await;
    }

    let unlock = admin
        .post(app.url(&format!("/dashboard-users/{member_id}/unlock")))
        .send()
        .await
        .unwrap();
    assert_eq!(unlock.status(), StatusCode::NO_CONTENT);

    // Immediately usable again with the correct password.
    let (_client, login_body) = app.login("member@example.com", "hunter2hunter2").await;
    assert_eq!(login_body["role"], "member");
}

#[tokio::test]
async fn unlocking_an_account_that_isnt_locked_is_a_no_op() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (_member_client, member_body) = app
        .register_member(&admin, "member@example.com", &code)
        .await;
    let member_id = user_id(&member_body);

    let unlock = admin
        .post(app.url(&format!("/dashboard-users/{member_id}/unlock")))
        .send()
        .await
        .unwrap();
    assert_eq!(unlock.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn cross_org_unlock_rejected() {
    let app = TestApp::spawn().await;
    let (_admin_a, body_a) = app.register_admin("a@example.com", "OrgA").await;
    let target_id = user_id(&body_a);

    let (admin_b, _body_b) = app.register_admin("b@example.com", "OrgB").await;

    let unlock = admin_b
        .post(app.url(&format!("/dashboard-users/{target_id}/unlock")))
        .send()
        .await
        .unwrap();
    assert_eq!(unlock.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn member_cannot_unlock() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (member_client, member_body) = app
        .register_member(&admin, "member@example.com", &code)
        .await;
    let member_id = user_id(&member_body);

    let unlock = member_client
        .post(app.url(&format!("/dashboard-users/{member_id}/unlock")))
        .send()
        .await
        .unwrap();
    assert_eq!(unlock.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn dashboard_users_list_reports_is_locked_without_leaking_raw_fields() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    app.register_member(&admin, "member@example.com", &code)
        .await;

    for _ in 0..3 {
        fail_login(&app, "member@example.com").await;
    }

    let list: Value = admin
        .get(app.url("/dashboard-users"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let entries = list.as_array().unwrap();
    let founder = entries
        .iter()
        .find(|u| u["email"] == "founder@example.com")
        .unwrap();
    let member = entries
        .iter()
        .find(|u| u["email"] == "member@example.com")
        .unwrap();
    assert_eq!(founder["is_locked"], false);
    assert_eq!(member["is_locked"], true);
    for entry in entries {
        assert!(entry.get("failed_login_attempts").is_none());
        assert!(entry.get("locked_until").is_none());
    }
}
