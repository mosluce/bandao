//! Section 9.11 — creating an AppUser via `POST /app-users` immediately
//! yields a `checkin_user_status` row with status=off_duty, ready for first
//! `clock_in`.

mod common;

use bson::oid::ObjectId;
use common::TestApp;
use common::ts;
use reqwest::StatusCode;


#[tokio::test]
async fn newly_created_app_user_has_off_duty_status_row() {
    let app = TestApp::spawn().await;
    let (admin, _body) = app.register_admin("admin@example.com", "Acme").await;
    let create_body = app.create_app_user(&admin, "alice", "Alice Chen").await;
    let app_user_id =
        ObjectId::parse_str(create_body["user"]["id"].as_str().unwrap()).unwrap();

    let row = app
        .db()
        .checkin_user_status
        .find(app_user_id)
        .await
        .unwrap()
        .expect("status row should be initialised by AppUser create");
    assert_eq!(
        bson::to_bson(&row.status).unwrap().as_str(),
        Some("off_duty")
    );
    assert!(row.last_event_id.is_none());
    assert!(row.current_shift_started_at.is_none());
    // Sanity check: org_id on the status row matches the AppUser's org.
    let app_user = app
        .db()
        .app_users
        .find_by_id(app_user_id)
        .await
        .unwrap()
        .expect("app user row");
    assert_eq!(row.org_id, app_user.org_id);
}

#[tokio::test]
async fn newly_created_app_user_can_clock_in_immediately() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
    let create_body = app.create_app_user(&admin, "alice", "Alice").await;
    let initial = create_body["initial_password"].as_str().unwrap().to_string();
    let (app_client, login) = app.app_login(&org_code, "alice", &initial).await;
    let token = login["token"].as_str().unwrap().to_string();

    // Clear the password gate.
    let r = app
        .app_post(&app_client, &token, "/app/me/password")
        .json(&serde_json::json!({
            "current_password": initial,
            "new_password": "newpass!secure",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // First-ever event is a clock_in — succeeds because the status row
    // already exists in off_duty.
    let r = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;
    assert_eq!(r.status(), StatusCode::CREATED);
}
