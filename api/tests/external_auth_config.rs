//! External-auth configuration validation and permission checks that don't
//! need a live database (the config endpoint validates before connecting).

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

fn config_body(query: &str, key_col: &str, display_col: &str) -> Value {
    json!({
        "auth_source": "external_db",
        "external_auth": {
            "driver": "mssql",
            "host": "10.0.0.9",
            "port": 1433,
            "database": "hr",
            "username": "sa",
            "password": "s3cret",
            "query": query,
            "key_col": key_col,
            "display_col": display_col,
        }
    })
}

#[tokio::test]
async fn query_missing_placeholder_is_rejected() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;

    // Missing @password.
    let resp = admin
        .put(app.url("/orgs/me/external-auth"))
        .json(&config_body(
            "SELECT id, name FROM staff WHERE acct=@account",
            "id",
            "name",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "VALIDATION");

    // Empty key_col.
    let resp = admin
        .put(app.url("/orgs/me/external-auth"))
        .json(&config_body(
            "SELECT id, name FROM staff WHERE acct=@account AND pwd=@password",
            "",
            "name",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn switching_to_external_without_config_is_rejected() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;

    let resp = admin
        .put(app.url("/orgs/me/external-auth"))
        .json(&json!({ "auth_source": "external_db" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn member_cannot_configure_external_auth() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let (member, _) = app
        .register_member(&admin, "member@example.com", &code)
        .await;

    let resp = member
        .put(app.url("/orgs/me/external-auth"))
        .json(&config_body(
            "SELECT id, name FROM staff WHERE acct=@account AND pwd=@password",
            "id",
            "name",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn default_org_reports_internal_auth_source() {
    let app = TestApp::spawn().await;
    let (_admin, body) = app.register_admin("admin@example.com", "Acme").await;
    assert_eq!(body["current_org"]["auth_source"], "internal");
    assert!(body["current_org"].get("external_auth").is_none());
}
