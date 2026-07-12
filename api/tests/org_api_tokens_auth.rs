//! Direct coverage of `auth::api_token::resolve_from_bearer` against a real
//! (testcontainers) `Db`. Deliberately does NOT go through HTTP or the real
//! router — `ApiTokenAuthContext` has no route consumer yet (that lands in
//! `add-zhengdan-checkin-export`), per `add-org-api-tokens` tasks.md 3.4.

mod common;

use bandao_api::auth::api_token::resolve_from_bearer;
use bandao_api::domain::ApiTokenScope;
use common::TestApp;
use serde_json::{Value, json};

/// Creates an active `checkin:read` token via the real (HTTP) CRUD endpoint
/// and returns its plaintext secret — reusing the real creation path keeps
/// this test honest about what a token actually looks like on the wire.
async fn create_token(app: &TestApp, admin: &reqwest::Client) -> String {
    let created: Value = admin
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "test token", "scopes": ["checkin:read"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    created["secret"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn valid_active_token_resolves_org_id_and_scopes() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = body["current_org"]["id"].as_str().unwrap().to_string();
    let secret = create_token(&app, &admin).await;

    let ctx = resolve_from_bearer(&app.state.db, &secret)
        .await
        .expect("active token should resolve");
    assert_eq!(ctx.org_id.to_hex(), org_id);
    assert_eq!(ctx.scopes, vec![ApiTokenScope::CheckinRead]);
    assert!(ctx.require_scope(ApiTokenScope::CheckinRead).is_ok());
}

#[tokio::test]
async fn disabled_token_does_not_resolve() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;
    let created: Value = admin
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "will disable", "scopes": ["checkin:read"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let secret = created["secret"].as_str().unwrap().to_string();
    let token_id = created["token"]["id"].as_str().unwrap();

    let patch = admin
        .patch(app.url(&format!("/orgs/me/api-tokens/{token_id}")))
        .json(&json!({ "status": "disabled" }))
        .send()
        .await
        .unwrap();
    assert_eq!(patch.status(), reqwest::StatusCode::OK);

    assert!(resolve_from_bearer(&app.state.db, &secret).await.is_err());
}

#[tokio::test]
async fn unknown_token_does_not_resolve() {
    let app = TestApp::spawn().await;
    let bogus = format!("bandao_at_{}", "x".repeat(43));
    assert!(resolve_from_bearer(&app.state.db, &bogus).await.is_err());
}

#[tokio::test]
async fn non_prefixed_bearer_value_is_untouched_by_this_path() {
    let app = TestApp::spawn().await;
    // Simulates an AppUser session token, which never carries the
    // `bandao_at_` prefix — must be rejected by this resolver without
    // (incorrectly) matching anything in `org_api_tokens`.
    let app_user_shaped_token = "not-an-api-token-at-all";
    assert!(
        resolve_from_bearer(&app.state.db, app_user_shaped_token)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rotated_token_invalidates_the_previous_secret() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;
    let created: Value = admin
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "will rotate", "scopes": ["checkin:read"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let secret_v1 = created["secret"].as_str().unwrap().to_string();
    let token_id = created["token"]["id"].as_str().unwrap();

    assert!(resolve_from_bearer(&app.state.db, &secret_v1).await.is_ok());

    let rotated: Value = admin
        .post(app.url(&format!("/orgs/me/api-tokens/{token_id}/rotate")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let secret_v2 = rotated["secret"].as_str().unwrap().to_string();

    assert!(
        resolve_from_bearer(&app.state.db, &secret_v1)
            .await
            .is_err(),
        "old secret must stop resolving immediately after rotate"
    );
    assert!(resolve_from_bearer(&app.state.db, &secret_v2).await.is_ok());
}
