//! Section 9.3 — strict per-AppUser ordering on `occurred_at_client`.

mod common;

use common::TestApp;
use common::ts;
use reqwest::StatusCode;
use serde_json::Value;


#[tokio::test]
async fn earlier_than_last_event_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let r = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(60))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);

    // OUT_OF_ORDER: client time is before the first event.
    let r = app
        .submit_checkin_event(&app_client, &token, "clock_out", 25.04, 121.56, &ts(30))
        .await;
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "OUT_OF_ORDER");
}

#[tokio::test]
async fn equal_to_last_event_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let when = ts(60);
    let r = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &when)
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);

    let r = app
        .submit_checkin_event(&app_client, &token, "clock_out", 25.04, 121.56, &when)
        .await;
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "OUT_OF_ORDER");
}

#[tokio::test]
async fn first_event_accepts_any_time() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    // Six hours in the past — first event, must succeed.
    let r = app
        .submit_checkin_event(
            &app_client,
            &token,
            "clock_in",
            25.04,
            121.56,
            "2020-01-01T00:00:00Z",
        )
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn out_of_order_is_per_app_user_only() {
    let app = TestApp::spawn().await;
    // Two AppUsers in the same Org.
    let (_admin, _code, _alice_id, alice_client, alice_token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    // Reuse the existing admin to create + log in bob.
    let (admin, body) = app.login("admin@example.com", "hunter2hunter2").await;
    let _ = admin;
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
    let bob_create = app
        .create_app_user(
            // The admin client is shared via `body`, so re-fetch by login.
            &app.login("admin@example.com", "hunter2hunter2").await.0,
            "bob",
            "Bob Lee",
        )
        .await;
    let bob_initial = bob_create["initial_password"].as_str().unwrap().to_string();
    let (bob_client, bob_login) = app.app_login(&org_code, "bob", &bob_initial).await;
    let bob_token = bob_login["token"].as_str().unwrap().to_string();
    // Clear bob's password gate.
    let resp = app
        .app_post(&bob_client, &bob_token, "/app/me/password")
        .json(&serde_json::json!({
            "current_password": bob_initial,
            "new_password": "newpass!secure",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Alice clocks in at minute 60.
    let r = app
        .submit_checkin_event(
            &alice_client,
            &alice_token,
            "clock_in",
            25.04,
            121.56,
            &ts(60),
        )
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);

    // Bob clocks in at minute 30 — earlier than Alice's, but Bob has no
    // events of his own. Per-AppUser scoping → accepted.
    let r = app
        .submit_checkin_event(&bob_client, &bob_token, "clock_in", 25.04, 121.56, &ts(30))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}
