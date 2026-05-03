mod common;

use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn logged_in_user_joins_via_org_code() {
    let app = TestApp::spawn().await;

    let (_owner_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();

    // Visitor builds their own first org so their identity exists.
    let (visitor, visitor_body) = app.register_admin("visitor@example.com", "VisitorOrg").await;
    let visitor_id = ObjectId::parse_str(visitor_body["user"]["id"].as_str().unwrap()).unwrap();

    let resp = visitor
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code_a }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["current_org"]["id"], org_a_id);
    assert_eq!(body["role"], "member");
    let memberships = body["memberships"].as_array().unwrap();
    assert_eq!(memberships.len(), 2);

    let count = app.membership_count(visitor_id).await;
    assert_eq!(count, 2);
}

#[tokio::test]
async fn join_via_active_slug_uses_same_org() {
    let app = TestApp::spawn().await;

    let (admin_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();
    admin_a
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "orga" }))
        .send()
        .await
        .unwrap();

    let (visitor, _) = app.register_admin("visitor@example.com", "Visitor").await;
    let resp = visitor
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": "orga" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["current_org"]["id"], org_a_id);
}

#[tokio::test]
async fn join_via_grace_period_slug() {
    let app = TestApp::spawn().await;

    use bson::DateTime;
    use bson::doc;
    const DAY_MS: i64 = 24 * 60 * 60 * 1000;

    let (admin_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();

    // Set "orga", backdate slug change, switch slug → "orga" enters grace.
    admin_a
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "orga" }))
        .send()
        .await
        .unwrap();
    let oid = ObjectId::parse_str(&org_a_id).unwrap();
    app.state
        .db
        .database
        .collection::<bson::Document>("orgs")
        .update_one(
            doc! { "_id": oid },
            doc! { "$set": {
                "slug_changed_at": DateTime::from_millis(DateTime::now().timestamp_millis() - 35 * DAY_MS)
            }},
        )
        .await
        .unwrap();
    admin_a
        .post(app.url("/orgs/me/slug"))
        .json(&json!({ "slug": "orga2" }))
        .send()
        .await
        .unwrap();

    // Visitor joins via the grace-period "orga".
    let (visitor, _) = app.register_admin("visitor@example.com", "Visitor").await;
    let resp = visitor
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": "orga" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["current_org"]["id"], org_a_id);
}

#[tokio::test]
async fn duplicate_membership_is_rejected() {
    let app = TestApp::spawn().await;

    let (_owner_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();

    let (visitor, _) = app.register_member("visitor@example.com", &code_a).await;
    // visitor is already a member; joining again must be ALREADY_MEMBER.
    let resp = visitor
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code_a }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "ALREADY_MEMBER");
}

#[tokio::test]
async fn invalid_identifier_rejected_without_lookup() {
    let app = TestApp::spawn().await;
    let (visitor, _) = app.register_admin("visitor@example.com", "VisitorOrg").await;

    for bad in ["!!!", "AcMe", "acme-corp", "x"] {
        let resp = visitor
            .post(app.url("/me/memberships"))
            .json(&json!({ "org_code": bad }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "input={bad:?}");
        let err: Value = resp.json().await.unwrap();
        assert_eq!(err["error"]["code"], "INVALID_ORG_CODE", "input={bad:?}");
    }
}

#[tokio::test]
async fn cooldown_blocks_join_via_me_memberships() {
    let app = TestApp::spawn().await;

    // OrgA admin kicks the visitor — cooldown for (OrgA, visitor email) lands.
    let (admin_a, body_a) = app.register_admin("a-owner@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let (_visitor, visitor_body) = app.register_member("visitor@example.com", &code_a).await;
    let visitor_id = visitor_body["user"]["id"].as_str().unwrap().to_string();

    let kick = admin_a
        .delete(app.url(&format!("/dashboard-users/{visitor_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(kick.status(), StatusCode::NO_CONTENT);

    // Identity survived the kick; logging back in works, but rejoining via
    // the new endpoint must be blocked by cooldown.
    let (visitor_again, _) = app.login("visitor@example.com", "hunter2hunter2").await;
    let resp = visitor_again
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code_a }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "EMAIL_IN_COOLDOWN");
}
