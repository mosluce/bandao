mod common;

use bson::oid::ObjectId;
use bson::{DateTime, doc};
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Backdate the org's `slug_changed_at` so the rate-limit window has elapsed.
async fn backdate_slug_change(app: &TestApp, org_id_hex: &str, days_ago: i64) {
    let oid = ObjectId::parse_str(org_id_hex).unwrap();
    let backdated = DateTime::from_millis(DateTime::now().timestamp_millis() - days_ago * DAY_MS);
    app.state
        .db
        .database
        .collection::<bson::Document>("orgs")
        .update_one(
            doc! { "_id": oid },
            doc! { "$set": { "slug_changed_at": backdated } },
        )
        .await
        .unwrap();
}

/// Force-expire a grace reservation so the slug lookup treats it as freed.
async fn expire_reservation(app: &TestApp, slug: &str) {
    let past = DateTime::from_millis(DateTime::now().timestamp_millis() - DAY_MS);
    app.state
        .db
        .database
        .collection::<bson::Document>("slug_reservations")
        .update_one(
            doc! { "slug": slug },
            doc! { "$set": { "expires_at": past } },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn join_works_during_grace_then_fails_after_expiry() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let org_id = admin_body["current_org"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // First SET (no rate limit).
    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acme" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // Backdate so the second change is allowed.
    backdate_slug_change(&app, &org_id, 35).await;

    // Change slug → old "acme" enters grace.
    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acmecorp" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // Old slug "acme" still resolves via grace; new identity registers and a
    // pending join_request is filed against the grace org. The session is
    // zero-org until admin approves.
    let (_grace_user, grace_body) = app
        .register_member_pending("graceuser@example.com", "acme")
        .await;
    assert!(grace_body["current_org"].is_null());
    assert!(
        grace_body["memberships"]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(false)
    );
    // Verify a pending request exists against the org for this email.
    let pending: Value = admin
        .get(app.url("/orgs/me/join-requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr = pending.as_array().unwrap();
    assert!(
        arr.iter().any(|r| r["email"] == "graceuser@example.com"),
        "expected pending request for graceuser@example.com, got: {pending:?}"
    );
    let _ = org_id;

    // Force-expire the grace reservation.
    expire_reservation(&app, "acme").await;

    // Old slug should now be rejected.
    let stranger = app.fresh_client();
    let r = stranger
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "late@example.com",
            "password": "hunter2hunter2",
            "org_code": "acme",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let err: Value = r.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_ORG_CODE");
}
