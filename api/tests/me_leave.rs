mod common;

use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::Value;

#[tokio::test]
async fn non_owner_member_can_self_leave() {
    let app = TestApp::spawn().await;
    let (_admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let org_id = ObjectId::parse_str(admin_body["current_org"]["id"].as_str().unwrap()).unwrap();

    let (member, member_body) = app.register_member("member@example.com", &code).await;
    let member_id = ObjectId::parse_str(member_body["user"]["id"].as_str().unwrap()).unwrap();

    let resp = member.post(app.url("/me/leave")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Identity SURVIVES — only the membership and current-org session are gone.
    let user = app
        .db()
        .dashboard_users
        .find_by_id(member_id)
        .await
        .unwrap();
    assert!(user.is_some(), "identity should survive self-leave");

    // Membership row gone.
    let m = app
        .db()
        .dashboard_memberships
        .find_by_user_and_org(member_id, org_id)
        .await
        .unwrap();
    assert!(m.is_none(), "membership for current_org should be gone");

    // Marker present with kind=left.
    let marker = app
        .db()
        .removed_memberships
        .find(org_id, "member@example.com")
        .await
        .unwrap()
        .expect("marker should exist");
    use argus_api::domain::RemovalKind;
    assert!(matches!(marker.removal_kind, RemovalKind::Left));

    // Subsequent /me with the same client returns 401 (cookie cleared / session deleted).
    let me = member.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn owner_cannot_self_leave() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("founder@example.com", "Acme").await;

    let resp = admin.post(app.url("/me/leave")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "OWNER_PROTECTED");
}

#[tokio::test]
async fn leave_without_active_org_is_no_active_org() {
    let app = TestApp::spawn().await;
    // Set up an identity that holds no membership: register, transfer
    // ownership, then self-leave. The freshly issued login session has
    // current_org_id = null, so /me/leave must return NO_ACTIVE_ORG.
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (_, second_body) = app.register_member("second@example.com", &code).await;
    let second_id = second_body["user"]["id"].as_str().unwrap().to_string();
    admin
        .patch(app.url(&format!("/dashboard-users/{second_id}/role")))
        .json(&serde_json::json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    admin
        .post(app.url("/orgs/me/owner"))
        .json(&serde_json::json!({
            "new_owner_user_id": second_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    admin.post(app.url("/me/leave")).send().await.unwrap();

    // Re-login: zero memberships.
    let (zero, _) = app.login("founder@example.com", "hunter2hunter2").await;
    let resp = zero.post(app.url("/me/leave")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "NO_ACTIVE_ORG");
}
