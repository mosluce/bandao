mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn admin_can_rotate_org_code_and_old_code_no_longer_joins() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let original_code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();

    let rotate = admin
        .post(app.url("/orgs/me/code/rotate"))
        .send()
        .await
        .unwrap();
    assert_eq!(rotate.status(), StatusCode::OK);
    let body: Value = rotate.json().await.unwrap();
    let new_code = body["code"].as_str().unwrap().to_string();
    assert_ne!(new_code, original_code);
    assert_eq!(new_code.chars().count(), 10);

    // Old code must no longer be a valid join credential.
    let stranger = app.fresh_client();
    let join_with_old = stranger
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "late@example.com",
            "password": "hunter2hunter2",
            "org_code": original_code,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(join_with_old.status(), StatusCode::BAD_REQUEST);
    let err: Value = join_with_old.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_ORG_CODE");

    // Joining with the new code still works (pending-then-approve flow).
    let (_arrival, _) = app
        .register_member(&admin, "new@example.com", &new_code)
        .await;
}

#[tokio::test]
async fn member_cannot_rotate_org_code() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (member, _) = app
        .register_member(&admin, "member@example.com", &code)
        .await;

    let resp = member
        .post(app.url("/orgs/me/code/rotate"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "FORBIDDEN");
}
