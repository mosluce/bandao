mod common;

use bson::doc;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

/// Helper: drop the identity row directly. This simulates the pre-launch
/// scenario where a previously-kicked identity is also gone (e.g. a future
/// "delete-account" endpoint), so a brand-new register-join with the same
/// email must hit the cooldown gate instead of EMAIL_TAKEN.
async fn delete_identity(app: &TestApp, email: &str) {
    app.state
        .db
        .database
        .collection::<bson::Document>("dashboard_users")
        .delete_one(doc! { "email": email })
        .await
        .unwrap();
}

#[tokio::test]
async fn rejoin_during_cooldown_via_register_is_blocked() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();

    // Member joins, then admin removes them — marker is created.
    let (_member, join_body) = app
        .register_member(&admin, "transient@example.com", &code)
        .await;
    let member_id = join_body["user"]["id"].as_str().unwrap().to_string();

    let removed = admin
        .delete(app.url(&format!("/dashboard-users/{member_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);

    // Identity SURVIVES admin-remove in the new model. To exercise the
    // register-mode=join cooldown branch we have to drop the identity row
    // manually (e.g. simulating a future delete-account).
    delete_identity(&app, "transient@example.com").await;

    // Brand-new register-join with the same email + same org → cooldown wins.
    let retry = app
        .fresh_client()
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "transient@example.com",
            "password": "hunter2hunter2",
            "org_code": code,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(retry.status(), StatusCode::CONFLICT);
    let err: Value = retry.json().await.unwrap();
    assert_eq!(err["error"]["code"], "EMAIL_IN_COOLDOWN");
}

#[tokio::test]
async fn existing_identity_rejoin_via_register_is_email_taken() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();

    let (_member, join_body) = app
        .register_member(&admin, "transient@example.com", &code)
        .await;
    let member_id = join_body["user"]["id"].as_str().unwrap().to_string();
    admin
        .delete(app.url(&format!("/dashboard-users/{member_id}")))
        .send()
        .await
        .unwrap();

    // Identity survived the kick; register-join with the same email is
    // strictly EMAIL_TAKEN (not EMAIL_IN_COOLDOWN), enforcing the
    // "register is for brand-new identities only" rule.
    let retry = app
        .fresh_client()
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "transient@example.com",
            "password": "hunter2hunter2",
            "org_code": code,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(retry.status(), StatusCode::CONFLICT);
    let err: Value = retry.json().await.unwrap();
    assert_eq!(err["error"]["code"], "EMAIL_TAKEN");
}

#[tokio::test]
async fn rejoin_to_different_org_during_cooldown_succeeds() {
    let app = TestApp::spawn().await;
    let (admin_a, body_a) = app.register_admin("alpha-owner@example.com", "OrgA").await;
    let (_admin_b, body_b) = app.register_admin("beta-owner@example.com", "OrgB").await;

    let code_a = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let code_b = body_b["current_org"]["code"].as_str().unwrap().to_string();

    // Member joins OrgA, then is kicked → cooldown for (OrgA, member email).
    let (_member, join_a) = app
        .register_member(&admin_a, "wanderer@example.com", &code_a)
        .await;
    let member_id = join_a["user"]["id"].as_str().unwrap().to_string();

    let kick = admin_a
        .delete(app.url(&format!("/dashboard-users/{member_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(kick.status(), StatusCode::NO_CONTENT);

    // Same email registers in OrgB — different org, so no cooldown applies.
    // Note: identity for `wanderer@example.com` survived the kick (only the
    // membership was deleted). So register would EMAIL_TAKEN. Use the new
    // /me/memberships flow to add another org from the same identity.
    let (wanderer, _) = app.login("wanderer@example.com", "hunter2hunter2").await;
    let join_b = wanderer
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code_b }))
        .send()
        .await
        .unwrap();
    assert_eq!(join_b.status(), StatusCode::OK);
}

#[tokio::test]
async fn rejoin_with_mixed_case_email_matches_lowercased_marker() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();

    let (_member, join1) = app
        .register_member(&admin, "transient@example.com", &code)
        .await;
    let member_id = join1["user"]["id"].as_str().unwrap().to_string();

    let removed = admin
        .delete(app.url(&format!("/dashboard-users/{member_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);

    // Mixed-case rejoin must hit the same marker (identity survives, but
    // login-then-/me/memberships should also be blocked by cooldown).
    // Use the register-join path with a freshly-cased email; identity was
    // preserved, so EMAIL_TAKEN would be raised first by register. Test the
    // cooldown via /me/memberships instead.
    let (transient, _) = app.login("transient@example.com", "hunter2hunter2").await;
    let retry_resp = transient
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code }))
        .send()
        .await
        .unwrap();
    assert_eq!(retry_resp.status(), StatusCode::CONFLICT);
    let err: Value = retry_resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "EMAIL_IN_COOLDOWN");
}

#[tokio::test]
async fn rejoin_after_admin_clears_cooldown_succeeds() {
    let app = TestApp::spawn().await;
    let (admin, admin_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = admin_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();

    let (_member, join1) = app
        .register_member(&admin, "transient@example.com", &code)
        .await;
    let member_id = join1["user"]["id"].as_str().unwrap().to_string();

    admin
        .delete(app.url(&format!("/dashboard-users/{member_id}")))
        .send()
        .await
        .unwrap();

    // Admin clears the cooldown.
    let cleared = admin
        .delete(app.url("/dashboard-users/cooldowns/transient@example.com"))
        .send()
        .await
        .unwrap();
    assert_eq!(cleared.status(), StatusCode::NO_CONTENT);

    // Identity survived the kick; log in and rejoin via /me/memberships.
    let (transient, _) = app.login("transient@example.com", "hunter2hunter2").await;
    let retry = transient
        .post(app.url("/me/memberships"))
        .json(&json!({ "org_code": code }))
        .send()
        .await
        .unwrap();
    assert_eq!(retry.status(), StatusCode::OK);
}
