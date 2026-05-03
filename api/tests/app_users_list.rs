mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn admin_lists_only_current_org_app_users() {
    let app = TestApp::spawn().await;

    let (admin_a, _body_a) = app.register_admin("a@example.com", "OrgA").await;
    let _ = app.create_app_user(&admin_a, "alice", "Alice").await;
    let _ = app.create_app_user(&admin_a, "bob", "Bob").await;

    // Separate Org with its own AppUser; must not bleed into OrgA's list.
    let (admin_b, _body_b) = app.register_admin("b@example.com", "OrgB").await;
    let _ = app.create_app_user(&admin_b, "carol", "Carol").await;

    let resp = admin_a.get(app.url("/app-users")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let users = body.as_array().expect("array");
    assert_eq!(users.len(), 2);
    let usernames: Vec<&str> = users
        .iter()
        .map(|u| u["username"].as_str().unwrap())
        .collect();
    assert!(usernames.contains(&"alice"));
    assert!(usernames.contains(&"bob"));
    assert!(!usernames.contains(&"carol"), "carol belongs to OrgB");

    // No password_hash leakage.
    for u in users {
        assert!(u.get("password_hash").is_none());
    }
}

#[tokio::test]
async fn member_cannot_list_app_users() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let (member, _) = app.register_member("member@example.com", &code).await;

    let resp = member.get(app.url("/app-users")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn no_active_org_returns_no_active_org() {
    let app = TestApp::spawn().await;

    // Founder offboards to reach zero-Org state — same setup as the
    // dashboard zero-Org tests.
    let (founder, body) = app.register_admin("founder@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let (_second, second_body) = app.register_member("second@example.com", &code).await;
    let second_id = second_body["user"]["id"].as_str().unwrap().to_string();
    founder
        .patch(app.url(&format!("/dashboard-users/{second_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    founder
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": second_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    founder.post(app.url("/me/leave")).send().await.unwrap();

    // Re-login: zero memberships.
    let (zero, login) = app.login("founder@example.com", "hunter2hunter2").await;
    assert!(login["current_org"].is_null());

    let resp = zero.get(app.url("/app-users")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "NO_ACTIVE_ORG");
}
