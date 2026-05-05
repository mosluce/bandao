//! Section 9.7 — admin force-checkout semantics. on_site / in_transit
//! AppUsers can be forced off; off_duty rejected NOT_ON_DUTY; cross-Org
//! NOT_FOUND; member FORBIDDEN; resulting event carries the synthetic
//! source / kind / location / manual_label / reason.

mod common;

use common::TestApp;
use common::ts;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn force_checkout_on_site_appuser_succeeds() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let _ = app
        .submit_checkin_event_with(
            &app_client,
            &token,
            json!({
                "event_type": "clock_in",
                "lat": 25.04,
                "lng": 121.56,
                "manual_label": "公司門口",
                "occurred_at_client": ts(0),
            }),
        )
        .await;

    let r = admin
        .post(app.url(&format!("/checkin/users/{app_user_id}/force-checkout")))
        .json(&json!({ "reason": "shift ended via line manager" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    let event = &body["event"];
    assert_eq!(event["event_type"], "clock_out");
    assert_eq!(event["source"], "admin_force");
    assert_eq!(event["initiated_by_kind"], "dashboard_user");
    assert_eq!(event["location"]["manual_label"], "管理員強制收班");
    // Coordinates copied from last event.
    assert_eq!(event["location"]["coordinates"]["lat"], 25.04);
    assert_eq!(event["location"]["coordinates"]["lng"], 121.56);
    assert_eq!(event["reason"], "shift ended via line manager");

    // Status flips to off_duty.
    assert_eq!(body["status"]["status"], "off_duty");
}

#[tokio::test]
async fn force_checkout_in_transit_appuser_succeeds() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, app_client, token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;
    let _ = app
        .submit_checkin_event(&app_client, &token, "clock_in", 25.04, 121.56, &ts(0))
        .await;
    let _ = app
        .submit_checkin_event(&app_client, &token, "transfer_out", 25.05, 121.57, &ts(1))
        .await;

    let r = admin
        .post(app.url(&format!("/checkin/users/{app_user_id}/force-checkout")))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["status"]["status"], "off_duty");
    assert_eq!(body["event"]["location"]["coordinates"]["lat"], 25.05);
}

#[tokio::test]
async fn force_checkout_off_duty_rejected_not_on_duty() {
    let app = TestApp::spawn().await;
    let (admin, _code, app_user_id, _app_client, _token, _pw) = app
        .seed_app_user_ready_to_checkin("admin@example.com", "Acme", "alice", "Alice")
        .await;

    let r = admin
        .post(app.url(&format!("/checkin/users/{app_user_id}/force-checkout")))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["code"], "NOT_ON_DUTY");
}

#[tokio::test]
async fn force_checkout_cross_org_returns_not_found() {
    let app = TestApp::spawn().await;
    // Org A admin, Org A AppUser on shift.
    let (_admin_a, _code_a, alice_id, alice_client, alice_token, _pw_a) = app
        .seed_app_user_ready_to_checkin("a-admin@example.com", "OrgA", "alice", "Alice")
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

    // Org B admin tries to force-checkout Alice.
    let (admin_b, _body) = app.register_admin("b-admin@example.com", "OrgB").await;
    let r = admin_b
        .post(app.url(&format!("/checkin/users/{alice_id}/force-checkout")))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn force_checkout_member_rejected() {
    let app = TestApp::spawn().await;
    let (_admin, _code, alice_id, alice_client, alice_token, _pw) = app
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

    // Add a member to the same Org and have them try.
    let (admin, body) = app.login("admin@example.com", "hunter2hunter2").await;
    let _ = admin;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let (member, _) = app.register_member("member@example.com", &code).await;

    let r = member
        .post(app.url(&format!("/checkin/users/{alice_id}/force-checkout")))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn force_checkout_reason_too_long_validation() {
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

    let r = admin
        .post(app.url(&format!("/checkin/users/{alice_id}/force-checkout")))
        .json(&json!({ "reason": "x".repeat(241) }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}
