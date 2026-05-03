//! Section 9.4 — dual timestamps. Past and future client times are accepted;
//! `has_skew_warning` flips at the 1-hour boundary.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::Value;

fn iso_offset_secs(seconds: i64) -> String {
    // `now + seconds` in RFC3339.
    let now = ::time::OffsetDateTime::now_utc();
    let dt = now + ::time::Duration::seconds(seconds);
    dt.format(&::time::format_description::well_known::Rfc3339)
        .unwrap()
}

#[tokio::test]
async fn future_client_time_accepted() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    // 30 minutes in the future.
    let r = app
        .submit_checkin_event(
            &app_client,
            &token,
            "clock_in",
            25.04,
            121.56,
            &iso_offset_secs(30 * 60),
        )
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn past_client_time_accepted_for_offline_sync() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    // Six hours ago — offline sync.
    let r = app
        .submit_checkin_event(
            &app_client,
            &token,
            "clock_in",
            25.04,
            121.56,
            &iso_offset_secs(-6 * 60 * 60),
        )
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn skew_warning_off_within_one_hour() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    // 59 minutes in the past (just inside the 1h window).
    let r = app
        .submit_checkin_event(
            &app_client,
            &token,
            "clock_in",
            25.04,
            121.56,
            &iso_offset_secs(-59 * 60),
        )
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    let body: Value = r.json().await.unwrap();
    assert_eq!(
        body["event"]["has_skew_warning"], false,
        "skew warning should be off at 59 min"
    );
}

#[tokio::test]
async fn skew_warning_on_beyond_one_hour() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    // 1h 1min in the past — outside the window.
    let r = app
        .submit_checkin_event(
            &app_client,
            &token,
            "clock_in",
            25.04,
            121.56,
            &iso_offset_secs(-(60 * 60 + 60)),
        )
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
    let body: Value = r.json().await.unwrap();
    assert_eq!(
        body["event"]["has_skew_warning"], true,
        "skew warning should be on at 1h 1min"
    );
}

#[tokio::test]
async fn ordering_uses_client_time() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    // First event is "now". Client time = now.
    let r = app
        .submit_checkin_event(
            &app_client,
            &token,
            "clock_in",
            25.04,
            121.56,
            &iso_offset_secs(0),
        )
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);

    // Second event arrives later in wall time, but its client time is
    // 30 minutes later than the first — fine.
    let r = app
        .submit_checkin_event(
            &app_client,
            &token,
            "clock_out",
            25.04,
            121.56,
            &iso_offset_secs(30 * 60),
        )
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);

    // Listing puts the later-client-time event first.
    let resp = app
        .app_get(&app_client, &token, "/app/checkin/events")
        .send()
        .await
        .unwrap();
    let events: Vec<Value> = resp.json().await.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["event_type"], "clock_out");
    assert_eq!(events[1]["event_type"], "clock_in");
}
