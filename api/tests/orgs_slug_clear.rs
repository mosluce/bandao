mod common;

use bson::oid::ObjectId;
use bson::{DateTime, doc};
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

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
    let (admin_a, body_a) = app.register_admin("a@example.com", "OrgA").await;
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();
    let (admin_b, _) = app.register_admin("b@example.com", "OrgB").await;

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
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let org_id = body["current_org"]["id"].as_str().unwrap().to_string();

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

    let r = admin.delete(app.url("/orgs/me/slug")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
    let err: Value = r.json().await.unwrap();
    assert_eq!(err["error"]["code"], "SLUG_CHANGE_TOO_SOON");
}
