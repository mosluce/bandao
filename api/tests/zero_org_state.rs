mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

/// Walk a new identity into the zero-Org state by transferring ownership
/// then self-leaving, and finally re-logging in. Returns a client whose
/// session has `current_org_id == null` and the identity's user_id (hex).
async fn build_zero_org_user(app: &TestApp) -> (reqwest::Client, String) {
    let (founder, founder_body) = app.register_admin("founder@example.com", "Acme").await;
    let founder_id = founder_body["user"]["id"].as_str().unwrap().to_string();
    let code = founder_body["current_org"]["code"].as_str().unwrap().to_string();
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

    let (zero, login_body) = app.login("founder@example.com", "hunter2hunter2").await;
    assert!(login_body["current_org"].is_null());
    assert_eq!(login_body["memberships"].as_array().unwrap().len(), 0);
    (zero, founder_id)
}

#[tokio::test]
async fn org_scoped_endpoints_reject_with_no_active_org() {
    let app = TestApp::spawn().await;
    let (zero, _) = build_zero_org_user(&app).await;

    // Sample a representative set of org-scoped endpoints. NO_ACTIVE_ORG = 403.
    let r = zero
        .post(app.url("/orgs/me/code/rotate"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN, "rotate code");
    let err: Value = r.json().await.unwrap();
    assert_eq!(err["error"]["code"], "NO_ACTIVE_ORG");

    let r = zero
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acme" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN, "set slug");

    let r = zero
        .get(app.url("/dashboard-users/cooldowns"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN, "list cooldowns");

    let r = zero.get(app.url("/dashboard-users")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN, "list members");

    let r = zero.post(app.url("/me/leave")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN, "leave");
}

#[tokio::test]
async fn org_agnostic_endpoints_succeed_with_no_active_org() {
    let app = TestApp::spawn().await;
    let (zero, _) = build_zero_org_user(&app).await;

    // /me succeeds with current_org=null
    let me = zero.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    let body: Value = me.json().await.unwrap();
    assert!(body["current_org"].is_null());

    // /me/orgs succeeds — this is the recovery path.
    let create = zero
        .post(app.url("/me/orgs"))
        .json(&json!({ "org_name": "Phoenix" }))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
}

#[tokio::test]
async fn user_recovers_via_me_memberships() {
    let app = TestApp::spawn().await;

    // OrgA exists with another owner.
    let (_owner_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();

    let (zero, _) = build_zero_org_user(&app).await;

    let resp = zero
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code_a }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["current_org"]["code"], code_a);
    assert_eq!(body["role"], "member");
}
