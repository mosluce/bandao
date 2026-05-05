//! `GET /checkin/users/:id/locations` — admin pagination, cross-org guard,
//! and member role gating.

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
async fn admin_lists_pings_newest_first() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &app_client, &token, 5).await;

    let r = admin
        .get(app.url(&format!("/checkin/users/{app_user_id}/locations")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 5);

    // Verify descending by occurred_at_client.
    let mut prev: Option<String> = None;
    for p in arr {
        let t = p["occurred_at_client"].as_str().unwrap().to_string();
        if let Some(prev_t) = prev {
            assert!(t < prev_t, "expected descending");
        }
        prev = Some(t);
    }
}

#[tokio::test]
async fn before_cursor_filters_results() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;
    seed_pings(&app, &app_client, &token, 10).await;

    let r1 = admin
        .get(app.url(&format!("/checkin/users/{app_user_id}/locations?limit=5")))
        .send()
        .await
        .unwrap();
    let body1: Value = r1.json().await.unwrap();
    let first_page = body1.as_array().unwrap();
    assert_eq!(first_page.len(), 5);
    let oldest = first_page.last().unwrap()["occurred_at_client"]
        .as_str()
        .unwrap()
        .to_string();

    let r2 = admin
        .get(app.url(&format!(
            "/checkin/users/{app_user_id}/locations?limit=5&before={oldest}"
        )))
        .send()
        .await
        .unwrap();
    let body2: Value = r2.json().await.unwrap();
    let second_page = body2.as_array().unwrap();
    // The remaining 5 pings are all strictly older than `oldest`.
    assert_eq!(second_page.len(), 5);
    for p in second_page {
        let t = p["occurred_at_client"].as_str().unwrap();
        assert!(t < oldest.as_str(), "second page should be strictly older");
    }
}

#[tokio::test]
async fn cross_org_app_user_id_returns_404() {
    let app = TestApp::spawn().await;
    let (admin_a, _code_a, _id_a, _client_a, _token_a, _pw_a) = app
        .seed_app_user_ready_to_checkin("admin-a@example.com", "AcmeA", "alice", "Alice")
        .await;
    let (_admin_b, _code_b, app_user_b, _client_b, _token_b, _pw_b) = app
        .seed_app_user_ready_to_checkin("admin-b@example.com", "AcmeB", "bob", "Bob")
        .await;

    let r = admin_a
        .get(app.url(&format!("/checkin/users/{app_user_b}/locations")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn member_role_blocked() {
    let app = TestApp::spawn().await;
    let (admin, code, app_user_id, _app_client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    enable_tracking(&app, &admin).await;

    // Register a second dashboard identity that joins the same Org as a
    // regular member.
    let (member_client, _) = app.register_member("member@example.com", &code).await;

    let r = member_client
        .get(app.url(&format!("/checkin/users/{app_user_id}/locations")))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}
