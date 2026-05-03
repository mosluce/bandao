mod common;

use bson::oid::ObjectId;
use bson::{DateTime, doc};
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn register_admin(app: &TestApp, email: &str, org_name: &str) -> (reqwest::Client, String) {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let resp = client
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": email,
            "password": "hunter2hunter2",
            "org_name": org_name,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let org_id = body["org"]["id"].as_str().unwrap().to_string();
    (client, org_id)
}

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
    let (admin, org_id) = register_admin(&app, "founder@example.com", "Acme").await;

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

    // Old slug "acme" still resolves via grace.
    let joiner = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let r = joiner
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "graceuser@example.com",
            "password": "hunter2hunter2",
            "org_code": "acme",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["org"]["id"], org_id);

    // Force-expire the grace reservation.
    expire_reservation(&app, "acme").await;

    // Old slug should now be rejected.
    let stranger = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
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
