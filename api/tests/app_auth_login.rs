mod common;

use bson::oid::ObjectId;
use bson::{DateTime, doc};
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Tiny builder: register a dashboard admin + create one AppUser.
/// Returns `(admin_client, org_code, org_id_hex, app_user_id_hex, initial_password, username)`.
async fn seed_app_user(app: &TestApp) -> (reqwest::Client, String, String, String, String, String) {
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
    let org_id = body["current_org"]["id"].as_str().unwrap().to_string();
    let create_body = app.create_app_user(&admin, "alice", "Alice Chen").await;
    let app_user_id = create_body["user"]["id"].as_str().unwrap().to_string();
    let initial_password = create_body["initial_password"]
        .as_str()
        .unwrap()
        .to_string();
    (
        admin,
        org_code,
        org_id,
        app_user_id,
        initial_password,
        "alice".to_string(),
    )
}

#[tokio::test]
async fn app_login_happy_path_issues_token_and_returns_context() {
    let app = TestApp::spawn().await;
    let (_admin, org_code, org_id, app_user_id, initial_password, username) =
        seed_app_user(&app).await;

    let (_client, body) = app.app_login(&org_code, &username, &initial_password).await;
    assert!(body["token"].as_str().unwrap().len() >= 40);
    assert!(body["expires_at"].is_string());
    assert_eq!(body["user"]["id"], app_user_id);
    assert_eq!(body["user"]["username"], "alice");
    assert_eq!(body["user"]["display_name"], "Alice Chen");
    assert_eq!(body["user"]["status"], "active");
    assert_eq!(body["org"]["id"], org_id);
    assert_eq!(body["needs_password_change"], true);

    // last_login_at is bumped on the AppUser row.
    let user_row = app
        .db()
        .app_users
        .find_by_id(ObjectId::parse_str(&app_user_id).unwrap())
        .await
        .unwrap()
        .expect("app user row");
    assert!(
        user_row.last_login_at.is_some(),
        "last_login_at should be set"
    );

    // app_sessions row exists for the issued token.
    let token = body["token"].as_str().unwrap();
    let sess = app
        .db()
        .app_sessions
        .find_by_token(token)
        .await
        .unwrap()
        .expect("app session row");
    assert_eq!(sess.app_user_id.to_hex(), app_user_id);
}

#[tokio::test]
async fn app_login_unknown_org_code_collapses_to_invalid_credentials() {
    let app = TestApp::spawn().await;
    let (_admin, _org_code, _org_id, _id, _pw, _u) = seed_app_user(&app).await;

    let resp = app
        .fresh_client()
        .post(app.url("/app/auth/login"))
        .json(&json!({
            "org_code": "NOTANORG12",
            "username": "alice",
            "password": "irrelevant",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn app_login_unknown_username_collapses_to_invalid_credentials() {
    let app = TestApp::spawn().await;
    let (_admin, org_code, _org_id, _id, _pw, _u) = seed_app_user(&app).await;

    let resp = app
        .fresh_client()
        .post(app.url("/app/auth/login"))
        .json(&json!({
            "org_code": org_code,
            "username": "ghost",
            "password": "irrelevant",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn app_login_wrong_password_returns_invalid_credentials() {
    let app = TestApp::spawn().await;
    let (_admin, org_code, _org_id, _id, _pw, username) = seed_app_user(&app).await;

    let resp = app
        .fresh_client()
        .post(app.url("/app/auth/login"))
        .json(&json!({
            "org_code": org_code,
            "username": username,
            "password": "WRONGWRONGWR",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn app_login_disabled_user_indistinguishable_from_wrong_password() {
    let app = TestApp::spawn().await;
    let (admin, org_code, _org_id, app_user_id, initial_password, username) =
        seed_app_user(&app).await;

    // Disable.
    let resp = admin
        .patch(app.url(&format!("/app-users/{app_user_id}")))
        .json(&json!({ "status": "disabled" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Login with the CORRECT password — must still see INVALID_CREDENTIALS.
    let resp = app
        .fresh_client()
        .post(app.url("/app/auth/login"))
        .json(&json!({
            "org_code": org_code,
            "username": username,
            "password": initial_password,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn app_login_username_lookup_is_case_insensitive() {
    let app = TestApp::spawn().await;
    let (_admin, org_code, _org_id, _id, initial_password, _u) = seed_app_user(&app).await;

    // AppUser was created with username="alice" → username_lower="alice".
    // Login with mixed-case "ALICE" must still match.
    let (_client, body) = app.app_login(&org_code, "ALICE", &initial_password).await;
    assert_eq!(body["user"]["username"], "alice");
}

#[tokio::test]
async fn app_login_via_active_slug_succeeds() {
    let app = TestApp::spawn().await;
    let (admin, _org_code, _org_id, _id, initial_password, username) = seed_app_user(&app).await;

    // Set a slug on the Org.
    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acme" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    let (_client, body) = app.app_login("acme", &username, &initial_password).await;
    assert_eq!(body["user"]["username"], "alice");
}

#[tokio::test]
async fn app_login_via_grace_slug_still_works() {
    let app = TestApp::spawn().await;
    let (admin, _org_code, org_id, _id, initial_password, username) = seed_app_user(&app).await;

    // First slug.
    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acme" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // Backdate slug_changed_at so the second change is allowed.
    let oid = ObjectId::parse_str(&org_id).unwrap();
    let backdated = DateTime::from_millis(DateTime::now().timestamp_millis() - 35 * DAY_MS);
    app.db()
        .database
        .collection::<bson::Document>("orgs")
        .update_one(
            doc! { "_id": oid },
            doc! { "$set": { "slug_changed_at": backdated } },
        )
        .await
        .unwrap();

    // Switch to a new slug; "acme" lands in grace.
    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acmecorp" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // Old slug "acme" still resolves while it's in grace.
    let (_client, body) = app.app_login("acme", &username, &initial_password).await;
    assert_eq!(body["user"]["username"], "alice");
}
