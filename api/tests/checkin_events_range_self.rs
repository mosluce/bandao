//! `GET /app/checkin/events?from=&to=` — AppUser self-history range filter.
//!
//! The trajectory surfaces ask both `location_pings` and `checkin_events`
//! for the same calendar day, so this endpoint's range semantics must match
//! `GET /app/checkin/me/locations` exactly: inclusive `from`, exclusive `to`,
//! single-sided allowed, `INVALID_RANGE` on parse failure / inversion /
//! span over 90 days, and no rejection based on an old `from` alone.

mod common;

use common::{TestApp, ts};
use reqwest::StatusCode;
use serde_json::Value;

/// Four events at ascending client times, alternating clock_in / clock_out so
/// the state machine and the per-AppUser ordering rule both pass.
async fn seed_four_events(app: &TestApp, client: &reqwest::Client, token: &str) {
    for (i, minute) in [-40i64, -30, -20, -10].iter().enumerate() {
        let event_type = if i % 2 == 0 { "clock_in" } else { "clock_out" };
        let r = app
            .submit_checkin_event(client, token, event_type, 25.0, 121.0, &ts(*minute))
            .await;
        assert_eq!(r.status(), StatusCode::CREATED, "seed event {i} failed");
    }
}

#[tokio::test]
async fn from_to_filters_to_range() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    seed_four_events(&app, &client, &token).await;

    // Window covering only the middle two (-30 and -20).
    let from = ts(-35);
    let to = ts(-15);
    let r = app
        .app_get(
            &client,
            &token,
            &format!("/app/checkin/events?from={from}&to={to}"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2, "expected the two in-window events");

    // Newest-first ordering is preserved alongside the range filter.
    let mut prev: Option<String> = None;
    for e in arr {
        let t = e["occurred_at_client"].as_str().unwrap().to_string();
        assert!(t >= from, "event before `from` leaked in");
        assert!(t < to, "event at or after `to` leaked in");
        if let Some(prev_t) = prev {
            assert!(t < prev_t, "expected newest-first");
        }
        prev = Some(t);
    }
}

#[tokio::test]
async fn single_sided_range_allowed() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    seed_four_events(&app, &client, &token).await;

    // Seeded at -40, -30, -20, -10; `to = -25` admits the first two.
    let to = ts(-25);
    let r = app
        .app_get(&client, &token, &format!("/app/checkin/events?to={to}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2, "the -40 and -30 events are before `to`");
    for e in arr {
        assert!(e["occurred_at_client"].as_str().unwrap() < to.as_str());
    }
}

#[tokio::test]
async fn omitting_the_range_preserves_prior_behaviour() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    seed_four_events(&app, &client, &token).await;

    let r = app
        .app_get(&client, &token, "/app/checkin/events")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 4, "unranged call returns the full page as before");
}

/// The span cap is the only range ceiling; a bound far outside any retention
/// window must still be accepted, or `legacy_backfill`-imported days become
/// permanently unreachable.
#[tokio::test]
async fn from_far_in_the_past_is_allowed_when_span_fits() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let from = ts(-200 * 24 * 60);
    let to = ts(-199 * 24 * 60);
    let r = app
        .app_get(
            &client,
            &token,
            &format!("/app/checkin/events?from={from}&to={to}"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::OK,
        "an old `from` alone must not be rejected"
    );
}

#[tokio::test]
async fn span_over_90_days_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let from = ts(-91 * 24 * 60);
    let to = ts(0);
    let r = app
        .app_get(
            &client,
            &token,
            &format!("/app/checkin/events?from={from}&to={to}"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"].as_str().unwrap(), "INVALID_RANGE");
}

#[tokio::test]
async fn to_before_from_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let from = ts(0);
    let to = ts(-24 * 60);
    let r = app
        .app_get(
            &client,
            &token,
            &format!("/app/checkin/events?from={from}&to={to}"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"].as_str().unwrap(), "INVALID_RANGE");
}

#[tokio::test]
async fn unparseable_bound_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let r = app
        .app_get(&client, &token, "/app/checkin/events?from=not-a-timestamp")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"].as_str().unwrap(), "INVALID_RANGE");
}
