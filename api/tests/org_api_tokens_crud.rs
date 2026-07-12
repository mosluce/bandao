mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

#[tokio::test]
async fn create_lists_rotates_disables_reenables_and_deletes() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;

    // Create.
    let create_resp = admin
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "震旦雲匯出", "scopes": ["checkin:read"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created: Value = create_resp.json().await.unwrap();
    let secret_v1 = created["secret"].as_str().unwrap().to_string();
    assert!(secret_v1.starts_with("bandao_at_"));
    let token_id = created["token"]["id"].as_str().unwrap().to_string();
    assert_eq!(created["token"]["name"], "震旦雲匯出");
    assert_eq!(created["token"]["status"], "active");
    assert!(
        created["token"]["token_prefix"]
            .as_str()
            .unwrap()
            .starts_with("bandao_at_")
    );

    // List: the plaintext secret must never come back.
    let list_resp = admin
        .get(app.url("/orgs/me/api-tokens"))
        .send()
        .await
        .unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list: Value = list_resp.json().await.unwrap();
    let entry = list
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == token_id)
        .expect("created token in list");
    assert_eq!(entry["token_prefix"], created["token"]["token_prefix"]);
    assert!(entry.get("secret").is_none());
    assert!(entry.get("token_hash").is_none());
    let serialized = serde_json::to_string(&list).unwrap();
    assert!(
        !serialized.contains(&secret_v1),
        "list response leaked the plaintext secret"
    );

    // Rotate: new secret returned, old value stops resolving (checked via
    // `resolve_from_bearer` directly in org_api_tokens_auth.rs — here we
    // just assert the row's prefix/secret actually changed).
    let rotate_resp = admin
        .post(app.url(&format!("/orgs/me/api-tokens/{token_id}/rotate")))
        .send()
        .await
        .unwrap();
    assert_eq!(rotate_resp.status(), StatusCode::OK);
    let rotated: Value = rotate_resp.json().await.unwrap();
    let secret_v2 = rotated["secret"].as_str().unwrap().to_string();
    assert_ne!(secret_v1, secret_v2);
    assert_ne!(
        created["token"]["token_prefix"],
        rotated["token"]["token_prefix"]
    );
    assert_eq!(rotated["token"]["name"], "震旦雲匯出");
    assert_eq!(rotated["token"]["scopes"], json!(["checkin:read"]));

    // Disable.
    let disable_resp = admin
        .patch(app.url(&format!("/orgs/me/api-tokens/{token_id}")))
        .json(&json!({ "status": "disabled" }))
        .send()
        .await
        .unwrap();
    assert_eq!(disable_resp.status(), StatusCode::OK);
    let disabled: Value = disable_resp.json().await.unwrap();
    assert_eq!(disabled["status"], "disabled");

    let disabled_token = app
        .db()
        .org_api_tokens
        .find_active_by_hash(&bandao_api::auth::api_token::hash_token(&secret_v2))
        .await
        .unwrap();
    assert!(
        disabled_token.is_none(),
        "disabled token must not resolve as active"
    );

    // Re-enable: same secret works again (no new secret generated).
    let enable_resp = admin
        .patch(app.url(&format!("/orgs/me/api-tokens/{token_id}")))
        .json(&json!({ "status": "active" }))
        .send()
        .await
        .unwrap();
    assert_eq!(enable_resp.status(), StatusCode::OK);
    let reenabled_token = app
        .db()
        .org_api_tokens
        .find_active_by_hash(&bandao_api::auth::api_token::hash_token(&secret_v2))
        .await
        .unwrap();
    assert!(
        reenabled_token.is_some(),
        "re-enabled token should resolve with its existing secret"
    );

    // Delete: irreversible, subsequent mutations 404.
    let delete_resp = admin
        .delete(app.url(&format!("/orgs/me/api-tokens/{token_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

    let after_delete = admin
        .patch(app.url(&format!("/orgs/me/api-tokens/{token_id}")))
        .json(&json!({ "status": "disabled" }))
        .send()
        .await
        .unwrap();
    assert_eq!(after_delete.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_with_empty_scopes_is_rejected() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;

    let resp = admin
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "no scopes", "scopes": [] }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: Value = resp.json().await.unwrap();
    assert_eq!(err["error"]["code"], "VALIDATION");

    let list: Value = admin
        .get(app.url("/orgs/me/api-tokens"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        list.as_array().unwrap().is_empty(),
        "rejected create must not persist a row"
    );
}

#[tokio::test]
async fn create_with_unknown_scope_value_is_rejected() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;

    // An unknown scope string fails to deserialize into `ApiTokenScope`
    // before the handler body ever runs — axum's default `Json` extractor
    // rejects this as 422 (not a hand-written `ApiError::Validation`, which
    // this codebase maps to 400). Either way the row is never persisted.
    let resp = admin
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "bad scope", "scopes": ["checkin:reed"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let list: Value = admin
        .get(app.url("/orgs/me/api-tokens"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        list.as_array().unwrap().is_empty(),
        "rejected create must not persist a row"
    );
}

#[tokio::test]
async fn member_is_forbidden_on_all_endpoints() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let (member, _) = app
        .register_member(&admin, "member@example.com", &code)
        .await;

    let created: Value = admin
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "seed", "scopes": ["checkin:read"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token_id = created["token"]["id"].as_str().unwrap();

    let list = member
        .get(app.url("/orgs/me/api-tokens"))
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::FORBIDDEN);

    let create = member
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "x", "scopes": ["checkin:read"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::FORBIDDEN);

    let rotate = member
        .post(app.url(&format!("/orgs/me/api-tokens/{token_id}/rotate")))
        .send()
        .await
        .unwrap();
    assert_eq!(rotate.status(), StatusCode::FORBIDDEN);

    let patch = member
        .patch(app.url(&format!("/orgs/me/api-tokens/{token_id}")))
        .json(&json!({ "status": "disabled" }))
        .send()
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::FORBIDDEN);

    let delete = member
        .delete(app.url(&format!("/orgs/me/api-tokens/{token_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cross_org_access_is_not_found() {
    let app = TestApp::spawn().await;
    let (admin_a, _) = app.register_admin("a@example.com", "OrgA").await;
    let (admin_b, _) = app.register_admin("b@example.com", "OrgB").await;

    let created: Value = admin_a
        .post(app.url("/orgs/me/api-tokens"))
        .json(&json!({ "name": "org a token", "scopes": ["checkin:read"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token_id = created["token"]["id"].as_str().unwrap();

    let rotate = admin_b
        .post(app.url(&format!("/orgs/me/api-tokens/{token_id}/rotate")))
        .send()
        .await
        .unwrap();
    assert_eq!(rotate.status(), StatusCode::NOT_FOUND);

    let patch = admin_b
        .patch(app.url(&format!("/orgs/me/api-tokens/{token_id}")))
        .json(&json!({ "status": "disabled" }))
        .send()
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::NOT_FOUND);

    let delete = admin_b
        .delete(app.url(&format!("/orgs/me/api-tokens/{token_id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::NOT_FOUND);

    // Org B's own list must stay empty — cross-Org token never leaks in.
    let list_b: Value = admin_b
        .get(app.url("/orgs/me/api-tokens"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list_b.as_array().unwrap().is_empty());
}
