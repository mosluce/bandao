//! Section 9.9 — admin live board and per-user history. current_org scope,
//! cross-Org NOT_FOUND, member FORBIDDEN, cursor pagination.

mod common;

use common::TestApp;
use common::ts;
use reqwest::StatusCode;
use serde_json::Value;

#[tokio::test]
async fn admin_lists_current_org_app_users_with_status() {
    let app = TestApp::spawn().await;
    let (admin, _code, alice_id, alice_client, alice_token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let _ = app
        .submit_checkin_event(
            &alice_client,
            &alice_token,
            "clock_in",
            25.04,
            121.56,
            &ts(0),
        )
        .await;

    let r = admin.get(app.url("/checkin/users")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Vec<Value> = r.json().await.unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["user"]["id"], alice_id);
    assert_eq!(body[0]["status"], "on_site");
    assert!(body[0]["current_shift_started_at"].is_string());
    assert_eq!(body[0]["has_skew_warning"], false);
}

#[tokio::test]
async fn cross_org_app_users_excluded_from_board() {
    let app = TestApp::spawn().await;
    let (_a_admin, _a_code, _alice_id, _ac, _at, _pw) = app
        .seed_app_user_ready_to_checkin("a@example.com", "OrgA", "alice", "Alice")
        .await;
    let (b_admin, _b_code, _bob_id, _bc, _bt, _pw_b) = app
        .seed_app_user_ready_to_checkin("b@example.com", "OrgB", "bob", "Bob")
        .await;

    let r = b_admin.get(app.url("/checkin/users")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Vec<Value> = r.json().await.unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["user"]["username"], "bob");
}

#[tokio::test]
async fn member_cannot_view_checkin_board() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let (member, _) = app
        .register_member(&admin, "member@example.com", &code)
        .await;

    let r = member.get(app.url("/checkin/users")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_views_app_user_event_history() {
    let app = TestApp::spawn().await;
    let (admin, _code, alice_id, alice_client, alice_token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    for (event_type, min) in [("clock_in", 0), ("clock_out", 30)] {
        let r = app
            .submit_checkin_event(
                &alice_client,
                &alice_token,
                event_type,
                25.04,
                121.56,
                &ts(min),
            )
            .await;
        assert_eq!(r.status(), StatusCode::CREATED);
    }

    let r = admin
        .get(app.url(&format!("/checkin/users/{alice_id}/events")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Vec<Value> = r.json().await.unwrap();
    assert_eq!(body.len(), 2);
    assert_eq!(body[0]["event_type"], "clock_out");
    assert_eq!(body[1]["event_type"], "clock_in");
}

#[tokio::test]
async fn cross_org_event_history_returns_not_found() {
    let app = TestApp::spawn().await;
    let (_a_admin, _a_code, alice_id, alice_client, alice_token, _pw) = app
        .seed_app_user_ready_to_checkin("a@example.com", "OrgA", "alice", "Alice")
        .await;
    let _ = app
        .submit_checkin_event(
            &alice_client,
            &alice_token,
            "clock_in",
            25.04,
            121.56,
            &ts(0),
        )
        .await;

    let (b_admin, _) = app.register_admin("b@example.com", "OrgB").await;

    let r = b_admin
        .get(app.url(&format!("/checkin/users/{alice_id}/events")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cursor_pagination_walks_history() {
    let app = TestApp::spawn().await;
    let (admin, _code, alice_id, alice_client, alice_token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    // Submit a synthetic alternating chain.
    let plan = [
        ("clock_in", 0),
        ("clock_out", 10),
        ("clock_in", 20),
        ("clock_out", 30),
        ("clock_in", 40),
    ];
    for (event_type, min) in plan {
        let r = app
            .submit_checkin_event(
                &alice_client,
                &alice_token,
                event_type,
                25.04,
                121.56,
                &ts(min),
            )
            .await;
        assert_eq!(r.status(), StatusCode::CREATED);
    }

    // First page with limit=2.
    let r = admin
        .get(app.url(&format!("/checkin/users/{alice_id}/events?limit=2")))
        .send()
        .await
        .unwrap();
    let page1: Vec<Value> = r.json().await.unwrap();
    assert_eq!(page1.len(), 2);
    let cursor = page1.last().unwrap()["occurred_at_client"]
        .as_str()
        .unwrap()
        .to_string();

    let r = admin
        .get(app.url(&format!(
            "/checkin/users/{alice_id}/events?limit=2&before={cursor}"
        )))
        .send()
        .await
        .unwrap();
    let page2: Vec<Value> = r.json().await.unwrap();
    assert_eq!(page2.len(), 2);
    // Pages must not overlap.
    assert_ne!(page1[1]["id"], page2[0]["id"]);
}
