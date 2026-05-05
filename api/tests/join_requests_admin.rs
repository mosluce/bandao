//! Admin-side endpoints for `org-join-requests`:
//! - `GET /orgs/me/join-requests`
//! - `POST .../approve`
//! - `POST .../reject`

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn submit_pending(app: &TestApp, code: &str, email: &str) -> String {
    let (_, _) = app.register_member_pending(email, code).await;
    // Find the request id from the admin-side list — we don't have a way
    // to read /me/join-requests here without the admin session. Fetch via
    // an admin client elsewhere in the test.
    String::new()
}

async fn pending_id_for(app: &TestApp, admin: &reqwest::Client, email: &str) -> String {
    let listed: Value = admin
        .get(app.url("/orgs/me/join-requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    listed
        .as_array()
        .and_then(|arr| arr.iter().find(|r| r["email"] == email))
        .and_then(|r| r["id"].as_str())
        .unwrap_or_else(|| panic!("no pending request for {email}"))
        .to_string()
}

#[tokio::test]
async fn admin_lists_pending_requests() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    submit_pending(&app, &code, "alice@example.com").await;
    submit_pending(&app, &code, "bob@example.com").await;

    let listed: Value = admin
        .get(app.url("/orgs/me/join-requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr = listed.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let emails: Vec<&str> = arr.iter().map(|r| r["email"].as_str().unwrap()).collect();
    assert!(emails.contains(&"alice@example.com"));
    assert!(emails.contains(&"bob@example.com"));
}

#[tokio::test]
async fn admin_filter_by_status() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    submit_pending(&app, &code, "alice@example.com").await;
    let alice_id = pending_id_for(&app, &admin, "alice@example.com").await;
    let r = admin
        .post(app.url(&format!("/orgs/me/join-requests/{alice_id}/reject")))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    submit_pending(&app, &code, "bob@example.com").await;

    let pending: Value = admin
        .get(app.url("/orgs/me/join-requests?status=pending"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pending.as_array().unwrap().len(), 1);

    let rejected: Value = admin
        .get(app.url("/orgs/me/join-requests?status=rejected"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(rejected.as_array().unwrap().len(), 1);
    assert_eq!(rejected[0]["email"], "alice@example.com");
}

#[tokio::test]
async fn admin_approve_creates_membership() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    let (alice_client, _) = app
        .register_member_pending("alice@example.com", &code)
        .await;
    let id = pending_id_for(&app, &admin, "alice@example.com").await;

    let r = admin
        .post(app.url(&format!("/orgs/me/join-requests/{id}/approve")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // Alice should now have a membership when she calls /me.
    let me: Value = alice_client
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let memberships = me["memberships"].as_array().unwrap();
    assert_eq!(memberships.len(), 1);
    assert_eq!(memberships[0]["role"], "member");
}

#[tokio::test]
async fn admin_cant_approve_cross_org_request() {
    let app = TestApp::spawn().await;
    let (admin_a, body_a) = app.register_admin("admin-a@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let (admin_b, body_b) = app.register_admin("admin-b@example.com", "OrgB").await;
    let _code_b = body_b["current_org"]["code"].as_str().unwrap().to_string();

    submit_pending(&app, &code_a, "alice@example.com").await;
    let id = pending_id_for(&app, &admin_a, "alice@example.com").await;

    // admin_b tries to approve an OrgA request.
    let r = admin_b
        .post(app.url(&format!("/orgs/me/join-requests/{id}/approve")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_approve_non_pending_returns_400() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    submit_pending(&app, &code, "alice@example.com").await;
    let id = pending_id_for(&app, &admin, "alice@example.com").await;

    // Reject first.
    admin
        .post(app.url(&format!("/orgs/me/join-requests/{id}/reject")))
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    // Then attempt approve — should be INVALID_STATE.
    let r = admin
        .post(app.url(&format!("/orgs/me/join-requests/{id}/approve")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "INVALID_STATE");
}

#[tokio::test]
async fn admin_reject_with_reason() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    submit_pending(&app, &code, "alice@example.com").await;
    let id = pending_id_for(&app, &admin, "alice@example.com").await;

    let r = admin
        .post(app.url(&format!("/orgs/me/join-requests/{id}/reject")))
        .json(&json!({ "rejection_reason": "外部承包商不收" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    let listed: Value = admin
        .get(app.url("/orgs/me/join-requests?status=rejected"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed[0]["rejection_reason"], "外部承包商不收");
}

#[tokio::test]
async fn admin_reject_oversized_reason_rejected() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    submit_pending(&app, &code, "alice@example.com").await;
    let id = pending_id_for(&app, &admin, "alice@example.com").await;

    let huge = "x".repeat(501);
    let r = admin
        .post(app.url(&format!("/orgs/me/join-requests/{id}/reject")))
        .json(&json!({ "rejection_reason": huge }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn member_cant_list_or_decide() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    submit_pending(&app, &code, "alice@example.com").await;
    let id = pending_id_for(&app, &admin, "alice@example.com").await;

    // Approve alice — she becomes a member, NOT an admin.
    admin
        .post(app.url(&format!("/orgs/me/join-requests/{id}/approve")))
        .send()
        .await
        .unwrap();

    // Login as alice to get a member-role session.
    let (alice, _) = app.login("alice@example.com", "hunter2hunter2").await;

    let r = alice
        .get(app.url("/orgs/me/join-requests"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}
