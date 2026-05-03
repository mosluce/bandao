mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn join_with_garbage_input_returns_invalid_org_code() {
    let app = TestApp::spawn().await;

    for bad in ["!!!", "", "AcMe", "acme-corp", "x"] {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .build()
            .unwrap();
        let r = client
            .post(app.url("/auth/register"))
            .json(&json!({
                "mode": "join",
                "email": "wanderer@example.com",
                "password": "hunter2hunter2",
                "org_code": bad,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST, "input={bad:?}");
        let err: Value = r.json().await.unwrap();
        assert_eq!(err["error"]["code"], "INVALID_ORG_CODE", "input={bad:?}");
    }
}
