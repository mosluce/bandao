mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::Value;

#[tokio::test]
async fn healthz_is_public_and_returns_ok() {
    let app = TestApp::spawn().await;

    // No auth header, no cookie. Healthz must be reachable to a fresh client.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let res = client.get(app.url("/healthz")).send().await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}
