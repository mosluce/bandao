//! Section 9.10 — `/app/checkin/status` and `/events` scope strictly to the
//! caller. AppUser A cannot peek at AppUser B's history through the AppUser
//! surface.

mod common;

use common::TestApp;
use common::ts;
use reqwest::StatusCode;
use serde_json::Value;


#[tokio::test]
async fn status_returns_only_callers_state() {
    let app = TestApp::spawn().await;
    let (_admin, _code, _alice_id, alice_client, alice_token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let _ = app
        .submit_checkin_event(&alice_client, &alice_token, "clock_in", 25.04, 121.56, &ts(0))
        .await;

    // Create bob in the same org.
    let (admin, body) = app.login("admin@example.com", "hunter2hunter2").await;
    let _ = admin;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let bob_create = app
        .create_app_user(
            &app.login("admin@example.com", "hunter2hunter2").await.0,
            "bob",
            "Bob Lee",
        )
        .await;
    let bob_initial = bob_create["initial_password"].as_str().unwrap().to_string();
    let (bob_client, login_b) = app.app_login(&code, "bob", &bob_initial).await;
    let bob_token = login_b["token"].as_str().unwrap().to_string();
    let r = app
        .app_post(&bob_client, &bob_token, "/app/me/password")
        .json(&serde_json::json!({
            "current_password": bob_initial,
            "new_password": "newpass!secure",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // Bob hits /app/checkin/status — must see his own (off_duty).
    let r = app
        .app_get(&bob_client, &bob_token, "/app/checkin/status")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["status"], "off_duty");

    // Bob hits /app/checkin/events — empty (Alice's events are not his).
    let r = app
        .app_get(&bob_client, &bob_token, "/app/checkin/events")
        .send()
        .await
        .unwrap();
    let events: Vec<Value> = r.json().await.unwrap();
    assert_eq!(events.len(), 0);
}
