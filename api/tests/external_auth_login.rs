//! End-to-end external-database auth against a real MSSQL container.
//!
//! NOTE: the MS SQL Server image is amd64-only; on arm64 hosts it runs under
//! emulation and may be slow/flaky. This test is the live verification for the
//! MSSQL provider, the config/test-login endpoints, shadow provisioning, and
//! the external-mode gating — run it on an amd64 host / in CI.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mssql_server::MssqlServer;
use tiberius::{AuthMethod, Client, Config as TiberiusConfig};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

async fn mssql_client(
    host: &str,
    port: u16,
) -> Client<tokio_util::compat::Compat<TcpStream>> {
    let mut cfg = TiberiusConfig::new();
    cfg.host(host);
    cfg.port(port);
    cfg.authentication(AuthMethod::sql_server(
        "sa",
        MssqlServer::DEFAULT_SA_PASSWORD,
    ));
    cfg.trust_cert();
    let tcp = TcpStream::connect(cfg.get_addr()).await.unwrap();
    tcp.set_nodelay(true).unwrap();
    Client::connect(cfg, tcp.compat_write()).await.unwrap()
}

#[tokio::test]
async fn external_db_login_provisions_shadow_and_gates_internal_ops() {
    // 1. Boot MSSQL and seed a staff table with one plaintext-credential row.
    let mssql = MssqlServer::default()
        .with_accept_eula()
        .start()
        .await
        .expect("start mssql container");
    let host = mssql.get_host().await.unwrap().to_string();
    let port = mssql.get_host_port_ipv4(1433).await.unwrap();

    let mut db = mssql_client(&host, port).await;
    db.execute(
        "CREATE TABLE staff (emp_id INT, acct NVARCHAR(50), pwd NVARCHAR(50), name NVARCHAR(50))",
        &[],
    )
    .await
    .unwrap();
    db.execute(
        "INSERT INTO staff (emp_id, acct, pwd, name) VALUES (@P1, @P2, @P3, @P4)",
        &[&1001i32, &"wang", &"secret", &"王小明"],
    )
    .await
    .unwrap();

    // 2. Set up an Org + admin and point it at the external database.
    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin("admin@example.com", "Acme").await;
    let code = body["current_org"]["code"].as_str().unwrap().to_string();

    let external_auth = json!({
        "driver": "mssql",
        "host": host,
        "port": port,
        "database": "master",
        "username": "sa",
        "password": MssqlServer::DEFAULT_SA_PASSWORD,
        "query": "SELECT emp_id, name FROM staff WHERE acct=@account AND pwd=@password",
        "key_col": "emp_id",
        "display_col": "name",
    });

    let resp = admin
        .put(app.url("/orgs/me/external-auth"))
        .json(&json!({ "auth_source": "external_db", "external_auth": external_auth }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let org: Value = resp.json().await.unwrap();
    assert_eq!(org["auth_source"], "external_db");
    assert_eq!(org["external_auth"]["password_set"], true);
    assert!(
        org["external_auth"].get("password").is_none(),
        "connection password must never be echoed"
    );

    // 3. Dry-run test-login resolves the identity without any writes.
    let resp = admin
        .post(app.url("/orgs/me/external-auth/test-login"))
        .json(&json!({
            "external_auth": external_auth,
            "test_account": "wang",
            "test_password": "secret",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let tl: Value = resp.json().await.unwrap();
    assert_eq!(tl["connected"], true);
    assert_eq!(tl["matched"], true);
    assert_eq!(tl["external_key"], "1001");
    assert_eq!(tl["display_name"], "王小明");

    // 4. A real external login issues a token and provisions a shadow user.
    let login = app
        .client
        .post(app.url("/app/auth/login"))
        .json(&json!({ "org_code": code, "username": "wang", "password": "secret" }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let lb: Value = login.json().await.unwrap();
    assert!(lb["token"].as_str().is_some());
    assert_eq!(lb["user"]["auth_source"], "external");
    assert_eq!(lb["user"]["external_key"], "1001");
    assert_eq!(lb["user"]["display_name"], "王小明");

    // 5. Wrong password collapses to INVALID_CREDENTIALS.
    let bad = app
        .client
        .post(app.url("/app/auth/login"))
        .json(&json!({ "org_code": code, "username": "wang", "password": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::UNAUTHORIZED);

    // 5b. Logging in again reuses the same shadow identity (no duplicate row).
    let login_again = app
        .client
        .post(app.url("/app/auth/login"))
        .json(&json!({ "org_code": code, "username": "wang", "password": "secret" }))
        .send()
        .await
        .unwrap();
    assert_eq!(login_again.status(), StatusCode::OK);
    let lb_again: Value = login_again.json().await.unwrap();
    assert_eq!(
        lb_again["user"]["id"], lb["user"]["id"],
        "repeat external login must reuse the same shadow AppUser"
    );

    // 5c. A misconfigured identity column is a distinct, connectable diagnostic.
    let bad_col = admin
        .post(app.url("/orgs/me/external-auth/test-login"))
        .json(&json!({
            "external_auth": {
                "driver": "mssql", "host": host, "port": port, "database": "master",
                "username": "sa", "password": MssqlServer::DEFAULT_SA_PASSWORD,
                "query": "SELECT emp_id, name FROM staff WHERE acct=@account AND pwd=@password",
                "key_col": "does_not_exist", "display_col": "name",
            },
            "test_account": "wang",
            "test_password": "secret",
        }))
        .send()
        .await
        .unwrap();
    let bc: Value = bad_col.json().await.unwrap();
    assert_eq!(bc["connected"], false);
    assert!(
        bc["error"].as_str().unwrap().contains("column"),
        "expected a column diagnostic, got {:?}",
        bc["error"]
    );

    // 6. The shadow user now appears in the admin roster.
    let list: Value = admin
        .get(app.url("/app-users"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let found = list
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u["external_key"] == "1001" && u["auth_source"] == "external");
    assert!(found, "external shadow user should be listed after first login");

    // 7. Internal-only mutations are gated while external auth is active.
    let created = admin
        .post(app.url("/app-users"))
        .json(&json!({ "username": "manual", "display_name": "Manual" }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CONFLICT);
    let err: Value = created.json().await.unwrap();
    assert_eq!(err["error"]["code"], "EXTERNAL_AUTH_MODE");
}
