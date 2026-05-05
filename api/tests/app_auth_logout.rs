mod common;

use common::TestApp;
use reqwest::StatusCode;

async fn seed_with_two_sessions(app: &TestApp) -> (String, String, String, String) {
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
    let create = app.create_app_user(&admin, "alice", "Alice Chen").await;
    let initial_password = create["initial_password"].as_str().unwrap().to_string();

    // Phone session.
    let (_phone, phone_body) = app.app_login(&org_code, "alice", &initial_password).await;
    let phone_token = phone_body["token"].as_str().unwrap().to_string();

    // Tablet session — separate login, separate token row.
    let (_tablet, tablet_body) = app.app_login(&org_code, "alice", &initial_password).await;
    let tablet_token = tablet_body["token"].as_str().unwrap().to_string();

    (org_code, initial_password, phone_token, tablet_token)
}

#[tokio::test]
async fn logout_deletes_only_caller_session_other_devices_survive() {
    let app = TestApp::spawn().await;
    let (_org_code, _pw, phone_token, tablet_token) = seed_with_two_sessions(&app).await;

    // Phone logs out.
    let resp = app
        .fresh_client()
        .post(app.url("/app/auth/logout"))
        .header("Authorization", format!("Bearer {phone_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Phone token now rejects.
    let me = app
        .fresh_client()
        .get(app.url("/app/me"))
        .header("Authorization", format!("Bearer {phone_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);

    // Tablet token still works.
    let me = app
        .fresh_client()
        .get(app.url("/app/me"))
        .header("Authorization", format!("Bearer {tablet_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
}

#[tokio::test]
async fn logout_works_while_needs_password_change_is_set() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
    let create = app.create_app_user(&admin, "alice", "Alice Chen").await;
    let initial_password = create["initial_password"].as_str().unwrap().to_string();
    let (_client, login_body) = app.app_login(&org_code, "alice", &initial_password).await;
    assert_eq!(login_body["needs_password_change"], true);
    let token = login_body["token"].as_str().unwrap().to_string();

    let resp = app
        .fresh_client()
        .post(app.url("/app/auth/logout"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}
