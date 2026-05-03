mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn register_create_mode_happy_path() {
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
        .expect("send register");

    assert_eq!(resp.status(), StatusCode::OK);
    let cookies: Vec<_> = resp.cookies().collect();
    assert!(
        cookies.iter().any(|c| c.name() == "argus_session"),
        "expected argus_session cookie, got {cookies:?}"
    );

    let body: Value = resp.json().await.expect("json body");
    assert_eq!(body["user"]["email"], "founder@example.com");
    assert_eq!(body["user"]["role"], "admin");
    assert_eq!(body["org"]["name"], "Acme");
    assert_eq!(body["role"], "admin");
    let code = body["org"]["code"].as_str().expect("org code");
    assert_eq!(code.chars().count(), 10);
    // Creator becomes the owner.
    assert_eq!(body["org"]["owner_id"], body["user"]["id"]);

    let me_resp = app
        .client
        .get(app.url("/me"))
        .send()
        .await
        .expect("send /me");
    assert_eq!(me_resp.status(), StatusCode::OK);
    let me: Value = me_resp.json().await.expect("me json");
    assert_eq!(me["user"]["email"], "founder@example.com");
    assert_eq!(me["org"]["code"], code);
}

#[tokio::test]
async fn register_join_mode_happy_path() {
    let app = TestApp::spawn().await;

    // Bootstrap an org with an admin to harvest its code.
    let creator = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let create_resp = creator
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
    assert_eq!(create_resp.status(), StatusCode::OK);
    let create_body: Value = create_resp.json().await.unwrap();
    let org_code = create_body["org"]["code"].as_str().unwrap().to_string();
    let org_id = create_body["org"]["id"].as_str().unwrap().to_string();

    // Different client = empty cookie jar.
    let joiner = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let join_resp = joiner
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "member@example.com",
            "password": "hunter2hunter2",
            "org_code": org_code,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(join_resp.status(), StatusCode::OK);
    let join_body: Value = join_resp.json().await.unwrap();
    assert_eq!(join_body["user"]["email"], "member@example.com");
    assert_eq!(join_body["user"]["role"], "member");
    assert_eq!(join_body["role"], "member");
    assert_eq!(join_body["org"]["id"], org_id);
}

#[tokio::test]
async fn register_rejects_email_taken() {
    let app = TestApp::spawn().await;

    let body = json!({
        "mode": "create",
        "email": "dup@example.com",
        "password": "hunter2hunter2",
        "org_name": "FirstOrg",
    });
    let first = app
        .client
        .post(app.url("/auth/register"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second_client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let second = second_client
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": "dup@example.com",
            "password": "hunter2hunter2",
            "org_name": "SecondOrg",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
    let err: Value = second.json().await.unwrap();
    assert_eq!(err["error"]["code"], "EMAIL_TAKEN");
}

#[tokio::test]
async fn register_join_rejects_bogus_code() {
    let app = TestApp::spawn().await;

    let resp = app
        .client
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "join",
            "email": "wanderer@example.com",
            "password": "hunter2hunter2",
            "org_code": "ZZZZZZZZZZ",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "INVALID_ORG_CODE");
}
