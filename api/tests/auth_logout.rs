mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn logout_invalidates_session() {
    let app = TestApp::spawn().await;

    let resp = app
        .client
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": "founder@example.com",
            "password": "hunter2hunter2",
            "org_name": "Acme",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let me = app.client.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me.status(), StatusCode::OK);

    let logout = app
        .client
        .post(app.url("/auth/logout"))
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);

    let me_after = app.client.get(app.url("/me")).send().await.unwrap();
    assert_eq!(me_after.status(), StatusCode::UNAUTHORIZED);
}
