//! Section 9.1 — every legal transition succeeds, every illegal pair
//! returns `INVALID_TRANSITION` with the prior state and attempted event
//! and leaves the status row unchanged.

mod common;

use common::TestApp;
use common::ts;
use reqwest::StatusCode;
use serde_json::Value;

/// Bump-by-1-minute helper: tests submit events in chronological order so
/// `OUT_OF_ORDER` doesn't kick in. We start at a fixed past timestamp so
/// the test stays deterministic regardless of wall clock.
async fn submit(
    app: &TestApp,
    client: &reqwest::Client,
    token: &str,
    event_type: &str,
    minute: i64,
) -> (StatusCode, Value) {
    let resp = app
        .submit_checkin_event(client, token, event_type, 25.04, 121.56, &ts(minute))
        .await;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap();
    (status, body)
}

#[tokio::test]
async fn legal_clock_in_transitions_off_duty_to_on_site() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let (status, body) = submit(&app, &app_client, &token, "clock_in", 0).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["status"]["status"], "on_site");
    assert!(body["status"]["current_shift_started_at"].is_string());
    assert_eq!(body["event"]["event_type"], "clock_in");
}

#[tokio::test]
async fn legal_clock_out_returns_to_off_duty() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let _ = submit(&app, &app_client, &token, "clock_in", 0).await;
    let (status, body) = submit(&app, &app_client, &token, "clock_out", 1).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["status"]["status"], "off_duty");
    // current_shift_started_at SHOULD reset to null on clock_out.
    assert!(
        body["status"]["current_shift_started_at"].is_null()
            || !body["status"]
                .as_object()
                .unwrap()
                .contains_key("current_shift_started_at")
    );
}

#[tokio::test]
async fn legal_transfer_out_then_transfer_in_keeps_shift_started_at() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let (_s, body_in) = submit(&app, &app_client, &token, "clock_in", 0).await;
    let started_at = body_in["status"]["current_shift_started_at"]
        .as_str()
        .unwrap()
        .to_string();

    let (s_out, body_out) = submit(&app, &app_client, &token, "transfer_out", 1).await;
    assert_eq!(s_out, StatusCode::CREATED);
    assert_eq!(body_out["status"]["status"], "in_transit");
    assert_eq!(body_out["status"]["current_shift_started_at"], started_at);

    let (s_in, body_back) = submit(&app, &app_client, &token, "transfer_in", 2).await;
    assert_eq!(s_in, StatusCode::CREATED);
    assert_eq!(body_back["status"]["status"], "on_site");
    assert_eq!(body_back["status"]["current_shift_started_at"], started_at);
}

#[tokio::test]
async fn legal_clock_out_from_in_transit() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let _ = submit(&app, &app_client, &token, "clock_in", 0).await;
    let _ = submit(&app, &app_client, &token, "transfer_out", 1).await;
    let (status, body) = submit(&app, &app_client, &token, "clock_out", 2).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["status"]["status"], "off_duty");
}

#[tokio::test]
async fn illegal_clock_in_from_on_site_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let _ = submit(&app, &app_client, &token, "clock_in", 0).await;
    let (status, body) = submit(&app, &app_client, &token, "clock_in", 1).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "INVALID_TRANSITION");
    assert_eq!(body["error"]["from"], "on_site");
    assert_eq!(body["error"]["attempted"], "clock_in");

    // Status row must be unchanged.
    let resp = app
        .app_get(&app_client, &token, "/app/checkin/status")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "on_site");
}

#[tokio::test]
async fn illegal_clock_out_from_off_duty_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let (status, body) = submit(&app, &app_client, &token, "clock_out", 0).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "INVALID_TRANSITION");
    assert_eq!(body["error"]["from"], "off_duty");
    assert_eq!(body["error"]["attempted"], "clock_out");
}

#[tokio::test]
async fn illegal_transfer_in_from_on_site_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let _ = submit(&app, &app_client, &token, "clock_in", 0).await;
    let (status, body) = submit(&app, &app_client, &token, "transfer_in", 1).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "INVALID_TRANSITION");
    assert_eq!(body["error"]["from"], "on_site");
    assert_eq!(body["error"]["attempted"], "transfer_in");
}

#[tokio::test]
async fn illegal_transfer_out_from_in_transit_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let _ = submit(&app, &app_client, &token, "clock_in", 0).await;
    let _ = submit(&app, &app_client, &token, "transfer_out", 1).await;
    let (status, body) = submit(&app, &app_client, &token, "transfer_out", 2).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "INVALID_TRANSITION");
    assert_eq!(body["error"]["from"], "in_transit");
    assert_eq!(body["error"]["attempted"], "transfer_out");
}

#[tokio::test]
async fn illegal_transfer_out_from_off_duty_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let (status, body) = submit(&app, &app_client, &token, "transfer_out", 0).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "INVALID_TRANSITION");
}
