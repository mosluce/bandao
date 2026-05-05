mod common;

use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn admin_updates_display_name_only() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;
    let body = app.create_app_user(&admin, "alice", "Old Name").await;
    let id = body["user"]["id"].as_str().unwrap().to_string();

    let resp = admin
        .patch(app.url(&format!("/app-users/{id}")))
        .json(&json!({ "display_name": "Brand New" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["display_name"], "Brand New");
    assert_eq!(body["status"], "active");
    assert_eq!(body["username"], "alice");
}

#[tokio::test]
async fn disable_kills_all_sessions_re_enable_preserves_password_and_flag() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
    let create = app.create_app_user(&admin, "alice", "Alice").await;
    let id = create["user"]["id"].as_str().unwrap().to_string();
    let initial_password = create["initial_password"].as_str().unwrap().to_string();

    // Two sessions on different "devices".
    let (_phone, phone) = app.app_login(&org_code, "alice", &initial_password).await;
    let phone_token = phone["token"].as_str().unwrap().to_string();
    let (_tablet, tablet) = app.app_login(&org_code, "alice", &initial_password).await;
    let tablet_token = tablet["token"].as_str().unwrap().to_string();

    // Disable.
    let resp = admin
        .patch(app.url(&format!("/app-users/{id}")))
        .json(&json!({ "status": "disabled" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "disabled");

    // Both sessions are gone.
    for token in [phone_token.as_str(), tablet_token.as_str()] {
        let me = app
            .fresh_client()
            .get(app.url("/app/me"))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap();
        assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
    }
    let session_count = app
        .db()
        .database
        .collection::<bson::Document>("app_sessions")
        .count_documents(bson::doc! {
            "app_user_id": ObjectId::parse_str(&id).unwrap()
        })
        .await
        .unwrap();
    assert_eq!(session_count, 0);

    // Re-enable preserves password (login still works with the original
    // initial_password) and `needs_password_change` is unchanged (still
    // true since we never changed it).
    let resp = admin
        .patch(app.url(&format!("/app-users/{id}")))
        .json(&json!({ "status": "active" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "active");
    assert_eq!(body["needs_password_change"], true);

    let (_client, login) = app.app_login(&org_code, "alice", &initial_password).await;
    assert_eq!(login["needs_password_change"], true);
}

#[tokio::test]
async fn member_cannot_update_app_user() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let create = app.create_app_user(&admin, "alice", "Alice").await;
    let id = create["user"]["id"].as_str().unwrap().to_string();

    let (member, _) = app.register_member("member@example.com", &code).await;
    let resp = member
        .patch(app.url(&format!("/app-users/{id}")))
        .json(&json!({ "display_name": "Hijacked" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cross_org_update_returns_not_found() {
    let app = TestApp::spawn().await;
    let (admin_a, _) = app.register_admin("a@example.com", "OrgA").await;
    let (admin_b, _) = app.register_admin("b@example.com", "OrgB").await;
    let create_b = app.create_app_user(&admin_b, "carol", "Carol").await;
    let carol_id = create_b["user"]["id"].as_str().unwrap().to_string();

    let resp = admin_a
        .patch(app.url(&format!("/app-users/{carol_id}")))
        .json(&json!({ "display_name": "From A" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "NOT_FOUND");
}
