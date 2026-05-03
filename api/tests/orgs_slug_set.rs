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
async fn set_slug_happy_path() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("founder@example.com", "Acme").await;

    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acme" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["slug"], "acme");
}

#[tokio::test]
async fn set_slug_normalizes_to_lowercase() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("founder@example.com", "Acme").await;

    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "AcMe" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["slug"], "acme");
}

#[tokio::test]
async fn set_slug_rejects_invalid_format() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("founder@example.com", "Acme").await;

    for bad in ["a", "acme-corp", &"a".repeat(25)] {
        let r = admin
            .post(app.url("/orgs/me/slug"))
            .json(&json!({ "slug": bad }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST, "input={bad}");
        let err: Value = r.json().await.unwrap();
        assert_eq!(err["error"]["code"], "INVALID_SLUG_FORMAT", "input={bad}");
    }
}

#[tokio::test]
async fn set_slug_rejects_reserved() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("founder@example.com", "Acme").await;

    for reserved in ["admin", "argus", "auth"] {
        let r = admin
            .post(app.url("/orgs/me/slug"))
            .json(&json!({ "slug": reserved }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST, "input={reserved}");
        let err: Value = r.json().await.unwrap();
        assert_eq!(err["error"]["code"], "SLUG_RESERVED", "input={reserved}");
    }
}

#[tokio::test]
async fn set_slug_rejects_taken_active() {
    let app = TestApp::spawn().await;
    let (admin_a, _) = app.register_admin("a@example.com", "OrgA").await;
    let (admin_b, _) = app.register_admin("b@example.com", "OrgB").await;

    let r1 = admin_a
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "shared" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::OK);

    let r2 = admin_b
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "shared" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::CONFLICT);
    let err: Value = r2.json().await.unwrap();
    assert_eq!(err["error"]["code"], "SLUG_TAKEN");
}

#[tokio::test]
async fn set_slug_rejects_taken_in_grace() {
    let app = TestApp::spawn().await;
    let (admin_a, body_a) = app.register_admin("a@example.com", "OrgA").await;
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();
    let (admin_b, _) = app.register_admin("b@example.com", "OrgB").await;

    // OrgA sets "shared" then changes away → "shared" enters grace.
    let r = admin_a
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "shared" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    backdate_slug_change(&app, &org_a_id, 35).await;

    let r = admin_a
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "shared2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // OrgB tries to claim "shared" while it's in grace → SLUG_TAKEN.
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
async fn set_slug_rate_limit_within_30_days_rejected() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("founder@example.com", "Acme").await;

    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acme" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acmecorp" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
    let err: Value = r.json().await.unwrap();
    assert_eq!(err["error"]["code"], "SLUG_CHANGE_TOO_SOON");
    assert!(
        err["error"]["retry_after"].is_string(),
        "expected retry_after timestamp"
    );
}

#[tokio::test]
async fn set_slug_after_30_days_succeeds() {
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

    backdate_slug_change(&app, &org_id, 31).await;

    let r = admin
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acmecorp" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
}

#[tokio::test]
async fn member_cannot_set_slug() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"].as_str().unwrap().to_string();
    let _ = admin;
    let (member, _) = app.register_member("member@example.com", &code).await;

    let r = member
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "acme" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    let err: Value = r.json().await.unwrap();
    assert_eq!(err["error"]["code"], "FORBIDDEN");
}
