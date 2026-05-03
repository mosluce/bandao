mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn user_can_switch_between_their_orgs() {
    let app = TestApp::spawn().await;

    let (visitor, body_a) = app.register_admin("visitor@example.com", "OrgA").await;
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();

    // Visitor creates a second Org via /me/orgs.
    let create_b = visitor
        .post(app.url("/me/orgs"))
        .json(&json!({ "org_name": "OrgB" }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_b.status(), StatusCode::OK);
    let body_b: Value = create_b.json().await.unwrap();
    let org_b_id = body_b["current_org"]["id"].as_str().unwrap().to_string();
    assert_eq!(body_b["current_org"]["id"], org_b_id);
    assert_ne!(org_a_id, org_b_id);

    // Switch back to OrgA.
    let switch = visitor
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_a_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(switch.status(), StatusCode::OK);
    let switched: Value = switch.json().await.unwrap();
    assert_eq!(switched["current_org"]["id"], org_a_id);
    assert_eq!(switched["role"], "admin");
}

#[tokio::test]
async fn switching_to_a_non_member_org_is_rejected() {
    let app = TestApp::spawn().await;

    let (_owner_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();

    // Identity B has no membership in OrgA — switch must be rejected.
    let (b_client, _) = app.register_admin("b-owner@example.com", "OrgB").await;
    let resp = b_client
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_a_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "NOT_A_MEMBER");
}

#[tokio::test]
async fn switching_to_current_org_is_a_noop() {
    let app = TestApp::spawn().await;
    let (visitor, body) = app.register_admin("visitor@example.com", "Acme").await;
    let org_id = body["current_org"]["id"].as_str().unwrap().to_string();

    let resp = visitor
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let switched: Value = resp.json().await.unwrap();
    assert_eq!(switched["current_org"]["id"], org_id);
}

#[tokio::test]
async fn switch_only_affects_caller_session() {
    let app = TestApp::spawn().await;

    // Same identity logs in twice (different cookie jars). Switching one
    // session must not move the other.
    let (admin, body) = app.register_admin("visitor@example.com", "Acme").await;
    let org_a_id = body["current_org"]["id"].as_str().unwrap().to_string();

    // Same user creates a second Org via the first session.
    let create_b = admin
        .post(app.url("/me/orgs"))
        .json(&json!({ "org_name": "OrgB" }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_b.status(), StatusCode::OK);
    let body_b: Value = create_b.json().await.unwrap();
    let org_b_id = body_b["current_org"]["id"].as_str().unwrap().to_string();

    // Open a second session (login from a fresh client).
    let (alt, login_body) = app.login("visitor@example.com", "hunter2hunter2").await;
    // login defaults to oldest-owned, which is OrgA.
    assert_eq!(login_body["current_org"]["id"], org_a_id);

    // First session is currently on OrgB; switch alt to OrgA explicitly.
    alt.post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_a_id }))
        .send()
        .await
        .unwrap();

    // First session should still see OrgB on /me.
    let me1: Value = admin
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me1["current_org"]["id"], org_b_id);

    let me2: Value = alt
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me2["current_org"]["id"], org_a_id);
}
