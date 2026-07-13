//! Confirms `external_auth` never leaks to a non-admin caller (dashboard
//! member or any AppUser session) — see
//! `openspec/specs/external-db-auth/spec.md`'s "External-auth configuration
//! is only visible to dashboard admins" requirement.
//!
//! `Org::external_auth()` reads `settings.external_auth` independent of
//! `auth_source`, so these tests set the config document directly via the
//! repository while leaving `auth_source = internal` — no MSSQL
//! testcontainer needed, internal-auth login still works normally.

mod common;

use bandao_api::domain::{EncryptMode, ExternalAuthConfig, OrgAuthSource};
use bson::oid::ObjectId;
use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};

fn fixture_config() -> ExternalAuthConfig {
    ExternalAuthConfig {
        driver: "mssql".to_string(),
        host: "internal-db.example.local".to_string(),
        port: 1433,
        database: "erp".to_string(),
        username: "svc_bandao".to_string(),
        password_encrypted: "ciphertext-not-a-real-secret".to_string(),
        query: "SELECT id, name FROM staff WHERE account=@account AND pass=@password".to_string(),
        key_col: "id".to_string(),
        display_col: "name".to_string(),
        encrypt: EncryptMode::Required,
        trust_server_certificate: false,
        list_query: None,
    }
}

async fn seed_external_auth_config(app: &TestApp, org_id: ObjectId) {
    app.db()
        .orgs
        .set_auth_config(org_id, OrgAuthSource::Internal, Some(&fixture_config()))
        .await
        .unwrap();
}

#[tokio::test]
async fn me_includes_external_auth_for_admin_and_omits_it_for_member() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let org_id = ObjectId::parse_str(body["current_org"]["id"].as_str().unwrap()).unwrap();
    seed_external_auth_config(&app, org_id).await;

    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    let (member, _) = app
        .register_member(&admin, "member@example.com", &code)
        .await;

    let admin_me: Value = admin
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        admin_me["current_org"]["external_auth"].is_object(),
        "admin's /me should include external_auth: {admin_me}"
    );

    let member_me: Value = member
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        member_me["current_org"]
            .as_object()
            .unwrap()
            .get("external_auth")
            .is_none(),
        "member's /me leaked external_auth: {member_me}"
    );

    // Also check the `memberships[].org` array, not just `current_org`.
    let member_membership_org = member_me["memberships"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["org"]["id"] == org_id.to_hex())
        .unwrap();
    assert!(
        member_membership_org["org"]
            .as_object()
            .unwrap()
            .get("external_auth")
            .is_none(),
        "member's memberships[].org leaked external_auth: {member_me}"
    );
}

#[tokio::test]
async fn login_includes_external_auth_for_admin_and_omits_it_for_member() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin2@example.com", "Acme2").await;
    let org_id = ObjectId::parse_str(body["current_org"]["id"].as_str().unwrap()).unwrap();
    seed_external_auth_config(&app, org_id).await;

    let code = body["current_org"]["code"].as_str().unwrap().to_string();
    app.register_member(&admin, "member2@example.com", &code)
        .await;

    let admin_login: Value = app
        .fresh_client()
        .post(app.url("/auth/login"))
        .json(&json!({ "email": "admin2@example.com", "password": "hunter2hunter2" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        admin_login["current_org"]["external_auth"].is_object(),
        "admin's login response should include external_auth: {admin_login}"
    );

    let member_login: Value = app
        .fresh_client()
        .post(app.url("/auth/login"))
        .json(&json!({ "email": "member2@example.com", "password": "hunter2hunter2" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        member_login["current_org"]
            .as_object()
            .unwrap()
            .get("external_auth")
            .is_none(),
        "member's login response leaked external_auth: {member_login}"
    );
}

#[tokio::test]
async fn register_response_never_leaks_external_auth_for_a_brand_new_org() {
    // A brand-new Org (created in the same call) can't have a pre-existing
    // external_auth document, so this doesn't exercise the role-gating logic
    // the way the login/`/me` tests do — it's a light regression check that
    // register's response shape is unaffected by the OrgDto split.
    let app = TestApp::spawn().await;
    let resp = app
        .fresh_client()
        .post(app.url("/auth/register"))
        .json(&json!({
            "mode": "create",
            "email": "register-check@example.com",
            "password": "hunter2hunter2",
            "org_name": "Fresh Org",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert!(
        body["current_org"]
            .as_object()
            .unwrap()
            .get("external_auth")
            .is_none()
    );
}

#[tokio::test]
async fn app_login_and_app_me_never_include_external_auth() {
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin3@example.com", "Acme3").await;
    let org_id = ObjectId::parse_str(body["current_org"]["id"].as_str().unwrap()).unwrap();
    seed_external_auth_config(&app, org_id).await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    let created = app.create_app_user(&admin, "alice", "Alice").await;
    let initial_password = created["initial_password"].as_str().unwrap().to_string();

    let login_resp: Value = app
        .fresh_client()
        .post(app.url("/app/auth/login"))
        .json(&json!({ "org_code": code, "username": "alice", "password": initial_password }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        login_resp["org"]
            .as_object()
            .unwrap()
            .get("external_auth")
            .is_none(),
        "app login leaked external_auth: {login_resp}"
    );
    let token = login_resp["token"].as_str().unwrap();

    let me_resp: Value = app
        .fresh_client()
        .get(app.url("/app/me"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        me_resp["org"]
            .as_object()
            .unwrap()
            .get("external_auth")
            .is_none(),
        "app /me leaked external_auth: {me_resp}"
    );
}

#[tokio::test]
async fn admin_only_endpoints_still_include_external_auth() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin4@example.com", "Acme4").await;

    // POST /orgs/me/external-auth (configure) — admin-only, response should
    // still include external_auth (it's literally what the admin just saved).
    // (Handler doc-comment says PUT; the router actually wires it as POST.)
    let configure_resp: Value = admin
        .post(app.url("/orgs/me/external-auth"))
        .json(&json!({
            "auth_source": "external_db",
            "external_auth": {
                "driver": "mssql",
                "host": "db.example.local",
                "port": 1433,
                "database": "erp",
                "username": "svc",
                "password": "s3cret!",
                "query": "SELECT id, name FROM staff WHERE account=@account AND pass=@password",
                "key_col": "id",
                "display_col": "name",
                "encrypt": "required",
                "trust_server_certificate": false
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(configure_resp["external_auth"].is_object());

    // POST /orgs/me/owner (transfer_owner) — admin-only, response should
    // still include external_auth for the (already admin) caller. Requires a
    // second admin in the same Org to transfer to.
    let code = configure_resp["code"].as_str().unwrap().to_string();
    let (member_client, member_body) = app
        .register_member(&admin, "target-admin@example.com", &code)
        .await;
    let target_user_id = member_body["user"]["id"].as_str().unwrap().to_string();
    let _ = member_client;

    let promote = admin
        .patch(app.url(&format!("/dashboard-users/{target_user_id}/role")))
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(promote.status(), StatusCode::OK);

    let transfer_resp: Value = admin
        .post(app.url("/orgs/me/owner"))
        .json(&json!({
            "new_owner_user_id": target_user_id,
            "current_password": "hunter2hunter2",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        transfer_resp["external_auth"].is_object(),
        "transfer_owner response should still include external_auth for the calling admin: {transfer_resp}"
    );
}
