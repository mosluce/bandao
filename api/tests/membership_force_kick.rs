mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn leave_only_kicks_sessions_pointing_at_left_org() {
    let app = TestApp::spawn().await;

    let (_owner_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();

    let (_owner_b, body_b) = app.register_admin("b-owner@example.com", "OrgB").await;
    let code_b = body_b["current_org"]["code"].as_str().unwrap().to_string();
    let org_b_id = body_b["current_org"]["id"].as_str().unwrap().to_string();

    // Visitor joins OrgA via register, then OrgB via /me/memberships.
    let (visitor_a_session, _) = app.register_member("visitor@example.com", &code_a).await;
    visitor_a_session
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code_b }))
        .send()
        .await
        .unwrap();
    // visitor_a_session.current_org is now OrgB after the join.
    visitor_a_session
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_a_id }))
        .send()
        .await
        .unwrap();

    // Open a second session on OrgB.
    let (visitor_b_session, _) = app.login("visitor@example.com", "hunter2hunter2").await;
    visitor_b_session
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_b_id }))
        .send()
        .await
        .unwrap();

    // Visitor leaves OrgA via the first session.
    let leave = visitor_a_session
        .post(app.url("/me/leave"))
        .send()
        .await
        .unwrap();
    assert_eq!(leave.status(), StatusCode::NO_CONTENT);

    // visitor_a_session is now invalid.
    let me_a = visitor_a_session.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me_a.status(), StatusCode::UNAUTHORIZED);

    // visitor_b_session — pointing at OrgB — is still alive.
    let me_b = visitor_b_session.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me_b.status(), StatusCode::OK);
    let body: Value = me_b.json().await.unwrap();
    assert_eq!(body["current_org"]["id"], org_b_id);
}

#[tokio::test]
async fn admin_remove_only_kicks_target_sessions_for_that_org() {
    let app = TestApp::spawn().await;

    let (admin_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();
    let (_owner_b, body_b) = app.register_admin("b-owner@example.com", "OrgB").await;
    let code_b = body_b["current_org"]["code"].as_str().unwrap().to_string();
    let org_b_id = body_b["current_org"]["id"].as_str().unwrap().to_string();

    let (visitor, visitor_body) = app.register_member("visitor@example.com", &code_a).await;
    let visitor_id = visitor_body["user"]["id"].as_str().unwrap().to_string();
    visitor
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code_b }))
        .send()
        .await
        .unwrap();
    // /me/memberships swaps current_org to the joined Org per spec, so put the
    // first session back on OrgA — that's the session the kick should kill.
    visitor
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_a_id }))
        .send()
        .await
        .unwrap();

    // Open a second session and pin it to OrgB.
    let (visitor_b, _) = app.login("visitor@example.com", "hunter2hunter2").await;
    visitor_b
        .post(app.url("/me/current-org"))
        .json(&json!({ "org_id": org_b_id }))
        .send()
        .await
        .unwrap();

    // OrgA admin kicks visitor.
    let kick = admin_a
        .delete(app.url(&format!("/dashboard-users/{visitor_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(kick.status(), StatusCode::NO_CONTENT);

    // First session (OrgA-scoped) is now dead.
    let me = visitor.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);

    // Second session (OrgB-scoped) survives.
    let me_b = visitor_b.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me_b.status(), StatusCode::OK);
    let body: Value = me_b.json().await.unwrap();
    assert_eq!(body["current_org"]["id"], org_b_id);
}
