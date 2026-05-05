mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

/// Build OrgA with two admins: founder (current owner) and second (admin).
async fn setup_owner_and_admin(
    app: &TestApp,
) -> (reqwest::Client, String, reqwest::Client, String) {
    let (founder, founder_body) = app.register_admin("founder@example.com", "Acme").await;
    let founder_id = founder_body["user"]["id"].as_str().unwrap().to_string();
    let code = founder_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();

    let (second, second_body) = app
        .register_member(&founder, "second@example.com", &code)
        .await;
    let second_id = second_body["user"]["id"].as_str().unwrap().to_string();
    founder
        .patch(app.url(&format!("/dashboard-users/{second_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();

    (founder, founder_id, second, second_id)
}

#[tokio::test]
async fn happy_path_transfers_ownership() {
    let app = TestApp::spawn().await;
    let (founder, _founder_id, _second, second_id) = setup_owner_and_admin(&app).await;

    let resp = founder
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": second_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["owner_id"], second_id);

    // /me from founder side: still admin in this Org (membership is unchanged).
    let me: Value = founder
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["role"], "admin");
    assert_eq!(me["current_org"]["owner_id"], second_id);
}

#[tokio::test]
async fn non_owner_cannot_transfer() {
    let app = TestApp::spawn().await;
    let (_founder, founder_id, second, _second_id) = setup_owner_and_admin(&app).await;

    let resp = second
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": founder_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "FORBIDDEN");
}

#[tokio::test]
async fn member_cannot_transfer() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("founder@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let owner_id = body["user"]["id"].as_str().unwrap().to_string();
    let (member, _) = app
        .register_member(&admin, "member@example.com", &code)
        .await;

    let resp = member
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": owner_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn wrong_password_rejected() {
    let app = TestApp::spawn().await;
    let (founder, _, _, second_id) = setup_owner_and_admin(&app).await;

    let resp = founder
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": second_id,
            "current_password": "WRONGWRONGWRONG",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_PASSWORD");
}

#[tokio::test]
async fn target_must_be_admin() {
    let app = TestApp::spawn().await;
    let (founder, founder_body) = app.register_admin("founder@example.com", "Acme").await;
    let code = founder_body["current_org"]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (_member, member_body) = app
        .register_member(&founder, "member@example.com", &code)
        .await;
    let member_id = member_body["user"]["id"].as_str().unwrap().to_string();

    // Target is a member, not admin.
    let resp = founder
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": member_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_TARGET");

    // Target with no membership at all also INVALID_TARGET.
    let (_outside_admin, outside_body) = app.register_admin("outside@example.com", "Other").await;
    let outside_id = outside_body["user"]["id"].as_str().unwrap().to_string();
    let resp = founder
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": outside_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_TARGET");
}

#[tokio::test]
async fn self_transfer_rejected() {
    let app = TestApp::spawn().await;
    let (founder, founder_id, _, _) = setup_owner_and_admin(&app).await;

    let resp = founder
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": founder_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "SAME_OWNER");
}

#[tokio::test]
async fn previous_owner_can_self_leave_after_transfer() {
    let app = TestApp::spawn().await;
    let (founder, _, _, second_id) = setup_owner_and_admin(&app).await;

    let transfer = founder
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": second_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(transfer.status(), StatusCode::OK);

    let leave = founder.post(app.url("/me/leave")).send().await.unwrap();
    assert_eq!(leave.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn new_owner_is_protected_from_demotion_and_removal() {
    let app = TestApp::spawn().await;
    let (founder, founder_id, second, second_id) = setup_owner_and_admin(&app).await;

    let transfer = founder
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": second_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(transfer.status(), StatusCode::OK);
    let _ = second; // session usage hint

    // Founder, now a regular admin, tries to demote the new owner — rejected.
    let demote = founder
        .patch(app.url(&format!("/dashboard-users/{second_id}/role")))
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap();
    assert_eq!(demote.status(), StatusCode::FORBIDDEN);
    let err: Value = demote.json().await.unwrap();
    assert_eq!(err["error"]["code"], "OWNER_PROTECTED");

    // And cannot remove the new owner.
    let kick = founder
        .delete(app.url(&format!("/dashboard-users/{second_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(kick.status(), StatusCode::FORBIDDEN);
    let err: Value = kick.json().await.unwrap();
    assert_eq!(err["error"]["code"], "OWNER_PROTECTED");

    // The founder, on the other hand, is just a regular admin now.
    // The new owner can demote / remove them.
    let (new_owner_client, _) = app.login("second@example.com", "hunter2hunter2").await;
    let demote = new_owner_client
        .patch(app.url(&format!("/dashboard-users/{founder_id}/role")))
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap();
    assert_eq!(demote.status(), StatusCode::OK);
}
