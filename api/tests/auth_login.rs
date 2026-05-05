mod common;

use bson::oid::ObjectId;
use bson::{DateTime, doc};
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[tokio::test]
async fn login_happy_path() {
    let app = TestApp::spawn().await;
    let (_creator, body) = app.register_admin("founder@example.com", "Acme").await;
    let org_id = body["current_org"]["id"].as_str().unwrap().to_string();

    // Login from a fresh client to ensure we're exercising login (not the register cookie).
    let (client, login_body) = app.login("founder@example.com", "hunter2hunter2").await;
    // Login picks the user's owned org as default.
    assert_eq!(login_body["current_org"]["id"], org_id);
    assert_eq!(login_body["role"], "admin");

    let me = client.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::OK);
}

#[tokio::test]
async fn login_wrong_password_returns_invalid_credentials() {
    let app = TestApp::spawn().await;
    let (_creator, _) = app.register_admin("founder@example.com", "Acme").await;

    let resp = app
        .fresh_client()
        .post(app.url("/auth/login"))
        .json(&json!({
            "email": "founder@example.com",
            "password": "wrongwrongwrong",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn login_unknown_email_returns_invalid_credentials() {
    let app = TestApp::spawn().await;
    let (_creator, _) = app.register_admin("founder@example.com", "Acme").await;

    let resp = app
        .fresh_client()
        .post(app.url("/auth/login"))
        .json(&json!({
            "email": "ghost@example.com",
            "password": "anythinganything",
        }))
        .send()
        .await
        .unwrap();
    // Same generic error: must not leak whether the email exists.
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn login_default_current_org_prefers_oldest_owned() {
    let app = TestApp::spawn().await;

    // Identity A creates OrgA (owned). Identity B creates OrgB and gives A
    // a member-membership in OrgB. After A logs in, current_org should be
    // OrgA (owned-first wins over the older membership-via-join, even when
    // the joined membership is younger).
    let (_a, a_body) = app.register_admin("alpha@example.com", "OrgA").await;
    let owned_id = a_body["current_org"]["id"].as_str().unwrap().to_string();

    // B registers as a fresh identity with their own org.
    let (b_client, b_body) = app.register_admin("beta@example.com", "OrgB").await;
    let beta_code = b_body["current_org"]["code"].as_str().unwrap().to_string();
    let _ = b_client; // unused after admin setup

    // A joins OrgB as a member via the new /me/memberships endpoint.
    let (a_client, _) = app.login("alpha@example.com", "hunter2hunter2").await;
    let join = a_client
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": beta_code }))
        .send()
        .await
        .unwrap();
    assert_eq!(join.status(), StatusCode::OK);

    // Re-login: default should still be the owned OrgA, even though the
    // membership for OrgB is the most recently created.
    let (_relogged, login_body) = app.login("alpha@example.com", "hunter2hunter2").await;
    assert_eq!(login_body["current_org"]["id"], owned_id);
    assert_eq!(login_body["role"], "admin");
}

#[tokio::test]
async fn login_default_current_org_falls_back_to_oldest_membership() {
    let app = TestApp::spawn().await;

    // Two admins create two orgs A and B with distinct owners. A third
    // identity joins OrgA, then OrgB. They own neither, so default should
    // be OrgA (smallest joined_at).
    let (admin_a, body_a) = app.register_admin("a@example.com", "OrgA").await;
    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let org_a_id = body_a["current_org"]["id"].as_str().unwrap().to_string();
    let _ = admin_a;

    let (admin_b, body_b) = app.register_admin("b@example.com", "OrgB").await;
    let code_b = body_b["current_org"]["code"].as_str().unwrap().to_string();
    let _ = admin_b;

    // Visitor registers via mode=join into OrgA.
    let (visitor, visitor_body) = app.register_member("visitor@example.com", &code_a).await;
    let visitor_id = ObjectId::parse_str(visitor_body["user"]["id"].as_str().unwrap()).unwrap();

    // The secondary join goes through /me/memberships.
    let join_b = visitor
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code_b }))
        .send()
        .await
        .unwrap();
    assert_eq!(join_b.status(), StatusCode::OK);

    // Re-login → expect oldest membership (OrgA). Force timestamps explicit
    // by stamping joined_at directly, since the test runs sub-millisecond
    // and the original join already happened first.
    let now = DateTime::now().timestamp_millis();
    app.state
        .db
        .database
        .collection::<bson::Document>("dashboard_memberships")
        .update_one(
            doc! { "user_id": visitor_id, "org_id": ObjectId::parse_str(&org_a_id).unwrap() },
            doc! { "$set": { "joined_at": DateTime::from_millis(now - 5 * DAY_MS) } },
        )
        .await
        .unwrap();

    let (_, login_body) = app.login("visitor@example.com", "hunter2hunter2").await;
    assert_eq!(login_body["current_org"]["id"], org_a_id);
    assert_eq!(login_body["role"], "member");
}

#[tokio::test]
async fn login_default_current_org_is_null_when_no_memberships() {
    let app = TestApp::spawn().await;

    // Identity exists but has no memberships: register, then leave the only Org.
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();

    // Bring in a second admin who can take over before the founder bows out.
    let (second_client, second_body) = app.register_member("second@example.com", &code).await;
    let second_id = second_body["user"]["id"].as_str().unwrap().to_string();
    // Promote second to admin so we can transfer ownership.
    let promote = admin
        .patch(app.url(&format!("/dashboard-users/{second_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(promote.status(), StatusCode::OK);

    // Founder transfers ownership, then self-leaves; identity survives but
    // has zero memberships.
    let transfer = admin
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": second_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(transfer.status(), StatusCode::OK);

    let leave = admin.post(app.url("/me/leave")).send().await.unwrap();
    assert_eq!(leave.status(), StatusCode::NO_CONTENT);
    let _ = second_client;

    // Re-login: current_org is null, memberships is empty.
    let (_, login_body) = app.login("founder@example.com", "hunter2hunter2").await;
    assert!(
        login_body["current_org"].is_null(),
        "current_org should be null"
    );
    assert!(login_body["role"].is_null(), "role should be null");
    assert_eq!(login_body["memberships"].as_array().unwrap().len(), 0);
}
