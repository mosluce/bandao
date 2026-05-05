mod common;

use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn logged_in_user_creates_new_org_and_becomes_owner() {
    let app = TestApp::spawn().await;
    let (client, body) = app.register_admin("founder@example.com", "Acme").await;
    let user_id = ObjectId::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();
    let original_org_id = body["current_org"]["id"].as_str().unwrap().to_string();

    let resp = client
        .post(app.url("/me/orgs"))
        .json(&json!({ "org_name": "NewCo" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let new_body: Value = resp.json().await.unwrap();
    let new_org_id = new_body["current_org"]["id"].as_str().unwrap().to_string();
    assert_ne!(new_org_id, original_org_id);
    assert_eq!(new_body["current_org"]["name"], "NewCo");
    assert_eq!(new_body["current_org"]["owner_id"], body["user"]["id"]);
    assert_eq!(new_body["role"], "admin");

    // Two memberships now: the original Acme + the new NewCo.
    let memberships = new_body["memberships"].as_array().unwrap();
    assert_eq!(memberships.len(), 2);
    let count = app.membership_count(user_id).await;
    assert_eq!(count, 2);

    // /me agrees that current_org is the new one.
    let me: Value = client
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["current_org"]["id"], new_org_id);
}

#[tokio::test]
async fn zero_org_user_can_create_org_to_recover() {
    let app = TestApp::spawn().await;

    // Set the founder up, then offboard them entirely (transfer + leave).
    let (founder, founder_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = founder_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (_second, second_body) = app
        .register_member(&founder, "second@example.com", &code)
        .await;
    let second_id = second_body["user"]["id"].as_str().unwrap().to_string();
    founder
        .patch(app.url(&format!("/dashboard-users/{second_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    founder
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": second_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    founder.post(app.url("/me/leave")).send().await.unwrap();

    // Re-login as founder: zero memberships, current_org_id == null.
    let (zero, login_body) = app.login("founder@example.com", "hunter2hunter2").await;
    assert!(login_body["current_org"].is_null());
    assert_eq!(login_body["memberships"].as_array().unwrap().len(), 0);

    // Recover by creating a new Org.
    let resp = zero
        .post(app.url("/me/orgs"))
        .json(&json!({ "org_name": "Phoenix" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["current_org"]["name"], "Phoenix");
    assert_eq!(body["role"], "admin");
}

#[tokio::test]
async fn create_org_validates_name() {
    let app = TestApp::spawn().await;
    let (client, _) = app.register_admin("founder@example.com", "Acme").await;

    let resp = client
        .post(app.url("/me/orgs"))
        .json(&json!({ "org_name": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "VALIDATION");
}
