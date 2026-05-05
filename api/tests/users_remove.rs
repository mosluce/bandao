mod common;

use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn admin_removes_member_succeeds_membership_only() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let org_id = ObjectId::parse_str(admin_body["current_org"]["id"].as_str().unwrap()).unwrap();

    let (member, member_body) = app.register_member("member@example.com", &code).await;
    let member_id_hex = member_body["user"]["id"].as_str().unwrap().to_string();
    let member_id = ObjectId::parse_str(&member_id_hex).unwrap();

    // Sanity: member's session is currently active.
    let me = member.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::OK);

    let resp = admin
        .delete(app.url(&format!("/dashboard-users/{member_id_hex}")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Identity SURVIVES — only the membership is gone.
    let user = app
        .db()
        .dashboard_users
        .find_by_id(member_id)
        .await
        .unwrap();
    assert!(user.is_some(), "identity should survive admin remove");

    let m = app
        .db()
        .dashboard_memberships
        .find_by_user_and_org(member_id, org_id)
        .await
        .unwrap();
    assert!(m.is_none(), "membership for current_org should be gone");

    // Marker is present.
    let marker = app
        .db()
        .removed_memberships
        .find(org_id, "member@example.com")
        .await
        .unwrap()
        .expect("marker should exist");
    assert_eq!(marker.email, "member@example.com");

    // Member's session is invalidated.
    let me_after = member.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me_after.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_removes_non_owner_admin_succeeds() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (_member, member_body) = app.register_member("second@example.com", &code).await;
    let other_id = member_body["user"]["id"].as_str().unwrap().to_string();

    // Promote the second user to admin.
    let promote = admin
        .patch(app.url(&format!("/dashboard-users/{other_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(promote.status(), StatusCode::OK);

    // Owner-admin removes the non-owner admin.
    let resp = admin
        .delete(app.url(&format!("/dashboard-users/{other_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn admin_cannot_remove_owner() {
    let app = TestApp::spawn().await;
    let (founder, founder_body) = app.register_admin("founder@example.com", "Acme").await;
    let owner_id = founder_body["user"]["id"].as_str().unwrap().to_string();
    let code = founder_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();

    // A second admin (non-owner) tries to remove the owner.
    let (_second, second_body) = app.register_member("second@example.com", &code).await;
    let second_id = second_body["user"]["id"].as_str().unwrap().to_string();
    let promote = founder
        .patch(app.url(&format!("/dashboard-users/{second_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(promote.status(), StatusCode::OK);

    // Re-login as the new admin.
    let (new_admin, _) = app.login("second@example.com", "hunter2hunter2").await;

    let resp = new_admin
        .delete(app.url(&format!("/dashboard-users/{owner_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "OWNER_PROTECTED");
}

#[tokio::test]
async fn admin_cannot_remove_self_via_id_endpoint() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let admin_id = body["user"]["id"].as_str().unwrap().to_string();

    let resp = admin
        .delete(app.url(&format!("/dashboard-users/{admin_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "FORBIDDEN");
}

#[tokio::test]
async fn member_cannot_remove_anyone() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let admin_id = admin_body["user"]["id"].as_str().unwrap().to_string();
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();

    let (member, _member_body) = app.register_member("member@example.com", &code).await;
    let _ = admin;

    let resp = member
        .delete(app.url(&format!("/dashboard-users/{admin_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_remove_cross_org_returns_not_found() {
    let app = TestApp::spawn().await;
    let (admin_a, _) = app.register_admin("alpha@example.com", "OrgA").await;
    let (_admin_b, body_b) = app.register_admin("beta@example.com", "OrgB").await;
    let outsider_id = body_b["user"]["id"].as_str().unwrap().to_string();

    let resp = admin_a
        .delete(app.url(&format!("/dashboard-users/{outsider_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "NOT_FOUND");
}
