//! Section 9.2 — full multi-site cycle: clock_in → transfer_out → transfer_in →
//! transfer_out → transfer_in → clock_out succeeds, three on_site segments
//! visible in the event list.

mod common;

use common::TestApp;
use common::ts;
use reqwest::StatusCode;
use serde_json::Value;

#[tokio::test]
async fn three_site_shift_cycles_through_legal_states() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let plan = [
        ("clock_in", 0, "on_site"),
        ("transfer_out", 30, "in_transit"),
        ("transfer_in", 60, "on_site"),
        ("transfer_out", 90, "in_transit"),
        ("transfer_in", 120, "on_site"),
        ("clock_out", 150, "off_duty"),
    ];

    let mut shift_started_at: Option<String> = None;
    for (event, min, expect_status) in plan {
        let resp = app
            .submit_checkin_event(&app_client, &token, event, 25.04, 121.56, &ts(min))
            .await;
        assert_eq!(
            resp.status(),
            StatusCode::CREATED,
            "transition `{event}` at minute {min} should succeed"
        );
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body["status"]["status"], expect_status);
        if event == "clock_in" {
            shift_started_at = body["status"]["current_shift_started_at"]
                .as_str()
                .map(|s| s.to_string());
        } else if expect_status == "off_duty" {
            assert!(
                body["status"]["current_shift_started_at"].is_null()
                    || !body["status"]
                        .as_object()
                        .unwrap()
                        .contains_key("current_shift_started_at"),
                "current_shift_started_at must be null after clock_out"
            );
        } else {
            assert_eq!(
                body["status"]["current_shift_started_at"],
                Value::String(shift_started_at.clone().unwrap()),
                "shift start should be preserved across {event}"
            );
        }
    }

    // Verify the event log lists all six events newest-first.
    let resp = app
        .app_get(&app_client, &token, "/app/checkin/events")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let events: Vec<Value> = resp.json().await.unwrap();
    assert_eq!(events.len(), 6);
    let types: Vec<&str> = events
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();
    assert_eq!(
        types,
        vec![
            "clock_out",
            "transfer_in",
            "transfer_out",
            "transfer_in",
            "transfer_out",
            "clock_in",
        ],
        "events should be newest-first"
    );

    // Three on_site segments are visible: 0→30, 60→90, 120→150 minutes.
    // Sanity-check by counting clock_in + transfer_in.
    let on_site_starts = events
        .iter()
        .filter(|e| {
            matches!(
                e["event_type"].as_str().unwrap(),
                "clock_in" | "transfer_in"
            )
        })
        .count();
    assert_eq!(on_site_starts, 3);
}
