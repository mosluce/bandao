mod common;

use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;

#[tokio::test]
async fn logout_invalidates_session_but_preserves_identity_and_membership() {
    let app = TestApp::spawn().await;

    let (client, body) = app.register_admin("founder@example.com", "Acme").await;
    let user_id = ObjectId::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();
    let org_id = ObjectId::parse_str(body["current_org"]["id"].as_str().unwrap()).unwrap();

    let me = client.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::OK);

    let logout = client
        .post(app.url("/auth/logout"))
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);

    // Same client now /me → 401.
    let me_after = client.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me_after.status(), StatusCode::UNAUTHORIZED);

    // But identity + membership survive logout.
    let user = app.db().dashboard_users.find_by_id(user_id).await.unwrap();
    assert!(user.is_some(), "identity should survive logout");
    let m = app
        .db()
        .dashboard_memberships
        .find_by_user_and_org(user_id, org_id)
        .await
        .unwrap();
    assert!(m.is_some(), "membership should survive logout");

    // Re-login still works.
    let (_relog, _) = app.login("founder@example.com", "hunter2hunter2").await;
}
