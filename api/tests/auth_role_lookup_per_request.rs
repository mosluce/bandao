mod common;

use bson::doc;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn role_demotion_takes_effect_on_next_request_without_relogin() {
    let app = TestApp::spawn().await;

    let (founder, body_a) = app.register_admin("founder@example.com", "Acme").await;
    let code = body_a["current_org"]["code"].as_str().unwrap().to_string();
    let (target, target_body) = app.register_member("second@example.com", &code).await;
    let target_id = target_body["user"]["id"].as_str().unwrap().to_string();

    // Promote second to admin.
    let promote = founder
        .patch(app.url(&format!("/dashboard-users/{target_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(promote.status(), StatusCode::OK);

    // Verify second is currently admin from their session — they can rotate.
    let rotate = target
        .post(app.url("/orgs/me/code/rotate"))
        .send()
        .await
        .unwrap();
    assert_eq!(rotate.status(), StatusCode::OK);

    // Founder demotes second.
    let demote = founder
        .patch(app.url(&format!("/dashboard-users/{target_id}/role")))
        .json(&json!({ "role": "member" }))
        .send()
        .await
        .unwrap();
    assert_eq!(demote.status(), StatusCode::OK);

    // Without re-login, second's NEXT request reflects the demotion.
    let rotate2 = target
        .post(app.url("/orgs/me/code/rotate"))
        .send()
        .await
        .unwrap();
    assert_eq!(rotate2.status(), StatusCode::FORBIDDEN);

    // /me also shows the new role.
    let me: Value = target
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["role"], "member");
}

#[tokio::test]
async fn stale_membership_session_returns_unauthorized_and_clears_cookie() {
    let app = TestApp::spawn().await;
    let (client, body) = app.register_admin("founder@example.com", "Acme").await;
    let user_id = bson::oid::ObjectId::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();
    let org_id = bson::oid::ObjectId::parse_str(body["current_org"]["id"].as_str().unwrap()).unwrap();

    // Force the membership row to vanish out from under the active session.
    // (Race / future-Org-delete simulation.)
    app.state
        .db
        .database
        .collection::<bson::Document>("dashboard_memberships")
        .delete_one(doc! { "user_id": user_id, "org_id": org_id })
        .await
        .unwrap();

    let me = client.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);

    // Middleware should emit a clearing Set-Cookie.
    let cleared = me
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .any(|v| {
            let s = v.to_str().unwrap_or("");
            s.starts_with("argus_session=") && (s.contains("Max-Age=0") || s.contains("max-age=0"))
        });
    assert!(
        cleared,
        "expected clearing Set-Cookie for argus_session, got headers: {:?}",
        me.headers().get_all(reqwest::header::SET_COOKIE)
    );
}
