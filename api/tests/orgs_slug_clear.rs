mod common;

use bson::oid::ObjectId;
use bson::{DateTime, doc};
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

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

#[tokio::test]
async fn clear_puts_slug_in_grace_and_locks_against_other_orgs() {
    let app = TestApp::spawn().await;
    let (admin_a, org_a_id) = register_admin(&app, "a@example.com", "OrgA").await;
    let (admin_b, _) = register_admin(&app, "b@example.com", "OrgB").await;

    let r = admin_a
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "shared" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    backdate_slug_change(&app, &org_a_id, 35).await;

    let r = admin_a.delete(app.url("/orgs/me/slug")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // OrgB cannot claim "shared" during grace period.
    let r = admin_b
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "shared" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let err: Value = r.json().await.unwrap();
    assert_eq!(err["error"]["code"], "SLUG_TAKEN");
}

#[tokio::test]
async fn second_clear_within_30_days_rate_limited() {
    let app = TestApp::spawn().await;
    let (admin, org_id) = register_admin(&app, "founder@example.com", "Acme").await;

    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acme" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    backdate_slug_change(&app, &org_id, 35).await;

    let r = admin.delete(app.url("/orgs/me/slug")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // Immediate second clear → rate limited.
    let r = admin.delete(app.url("/orgs/me/slug")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
    let err: Value = r.json().await.unwrap();
    assert_eq!(err["error"]["code"], "SLUG_CHANGE_TOO_SOON");
}
