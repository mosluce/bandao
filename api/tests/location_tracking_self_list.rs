//! `GET /app/checkin/me/locations` — AppUser self-list.
//!
//! Mirrors the admin list endpoint's range / pagination semantics but
//! resolves identity from the bearer token (no path / body input) and
//! intentionally does NOT consult the Org `location_tracking_enabled`
//! toggle so a user can still review pings from before the toggle was
//! turned off.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn enable_tracking(app: &TestApp, admin: &reqwest::Client) {
    let r = admin
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({ "location_tracking_enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
}

async fn disable_tracking(app: &TestApp, admin: &reqwest::Client) {
    let r = admin
        .patch(app.url("/orgs/me/settings"))
        .json(&json!({ "location_tracking_enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
}

fn iso_offset(seconds: i64) -> String {
    let t = ::time::OffsetDateTime::now_utc() + ::time::Duration::seconds(seconds);
    t.format(&::time::format_description::well_known::Rfc3339)
        .unwrap()
}

async fn seed_pings(app: &TestApp, app_client: &reqwest::Client, token: &str, count: usize) {
    let pings: Vec<Value> = (0..count)
        .map(|i| {
            json!({
                "lat": 25.0,
                "lng": 121.0 + (i as f64) * 0.001,
                "occurred_at_client": iso_offset(-(count as i64 - i as i64) * 10),
            })
        })
        .collect();
    let r = app
        .app_post(app_client, token, "/app/checkin/locations")
        .json(&json!({ "pings": pings }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED, "seed ping batch failed");
}

#[tokio::test]
async fn self_lists_own_pings_newest_first() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &app_client, &token, 5).await;

    let r = app
        .app_get(&app_client, &token, "/app/checkin/me/locations")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 5);

    let mut prev: Option<String> = None;
    for p in arr {
        assert_eq!(p["app_user_id"].as_str().unwrap(), app_user_id);
        let t = p["occurred_at_client"].as_str().unwrap().to_string();
        if let Some(prev_t) = prev {
            assert!(t < prev_t, "expected descending");
        }
        prev = Some(t);
    }
}

#[tokio::test]
async fn from_to_filters_to_range() {
    let app = TestApp::spawn().await;
    let (admin, _code, _app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &app_client, &token, 5).await;

    // Range covering only the middle 3 (offsets -40 .. -20 inclusive).
    let from = iso_offset(-45);
    let to = iso_offset(-15);

    let r = app
        .app_get(
            &app_client,
            &token,
            &format!("/app/checkin/me/locations?from={from}&to={to}"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    let mut prev: Option<String> = None;
    for p in arr {
        let t = p["occurred_at_client"].as_str().unwrap().to_string();
        assert!(t >= from);
        assert!(t < to);
        if let Some(prev_t) = prev {
            assert!(t < prev_t);
        }
        prev = Some(t);
    }
}

#[tokio::test]
async fn invalid_range_span_too_large() {
    let app = TestApp::spawn().await;
    let (admin, _code, _app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;

    let from = iso_offset(-91 * 24 * 3600);
    let to = iso_offset(0);
    let r = app
        .app_get(
            &app_client,
            &token,
            &format!("/app/checkin/me/locations?from={from}&to={to}"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"].as_str().unwrap(), "INVALID_RANGE");
}

#[tokio::test]
async fn unauthenticated_returns_401() {
    let app = TestApp::spawn().await;

    let resp = app
        .fresh_client()
        .get(app.url("/app/checkin/me/locations"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn another_users_pings_are_not_visible() {
    // Two AppUsers in the same Org. Alice seeds pings. Bob calls
    // /app/checkin/me/locations — must see only Bob's own pings (in this
    // case, zero) and NOT any ping carrying Alice's `app_user_id`.
    let app = TestApp::spawn().await;
    let (admin, code, alice_id, alice_client, alice_token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &alice_client, &alice_token, 3).await;

    // Create Bob in the same Org, log him in, and clear the force-change
    // gate so /app/checkin/* returns OK instead of 423.
    let create_bob = app.create_app_user(&admin, "bob", "Bob").await;
    let bob_initial_pw = create_bob["initial_password"].as_str().unwrap().to_string();
    let (bob_client, bob_login) = app.app_login(&code, "bob", &bob_initial_pw).await;
    let bob_token = bob_login["token"].as_str().unwrap().to_string();
    let resp = app
        .app_post(&bob_client, &bob_token, "/app/me/password")
        .json(&json!({
            "current_password": bob_initial_pw,
            "new_password": "newpass!secure",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let r = app
        .app_get(&bob_client, &bob_token, "/app/checkin/me/locations")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    for p in arr {
        assert_ne!(
            p["app_user_id"].as_str().unwrap(),
            alice_id,
            "bob's self-list leaked one of alice's pings"
        );
    }
}

#[tokio::test]
async fn toggle_off_does_not_block_self_read() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &app_client, &token, 3).await;

    // Flip the toggle back off — admin no longer wants new pings, but the
    // existing ones must still be readable by the AppUser themselves.
    disable_tracking(&app, &admin).await;

    let r = app
        .app_get(&app_client, &token, "/app/checkin/me/locations")
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::OK,
        "toggle-off must not block /app/checkin/me/locations"
    );
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    for p in arr {
        assert_eq!(p["app_user_id"].as_str().unwrap(), app_user_id);
    }
}
