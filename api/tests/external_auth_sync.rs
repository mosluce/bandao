//! `POST /orgs/me/external-auth/sync` — manual sync of the external AppUser
//! roster against a real MSSQL container. See
//! `openspec/specs/external-db-auth/spec.md`'s "Admin can manually sync the
//! external user roster" requirement.
//!
//! NOTE: the MS SQL Server image is amd64-only; on arm64 hosts it runs under
//! emulation and may be slow/flaky — same caveat as `external_auth_login.rs`.

mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::{Value, json};
use serial_test::serial;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mssql_server::MssqlServer;
use tiberius::{AuthMethod, Client, Config as TiberiusConfig};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

async fn mssql_client(host: &str, port: u16) -> Client<tokio_util::compat::Compat<TcpStream>> {
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

/// Boots MSSQL, seeds a `staff` table with two rows, and configures the Org
/// for `external_db` with both a login `query` and a `list_query`. Returns
/// `(app, admin, code, host, port, mssql)` for tests to further mutate/query
/// — callers MUST keep the returned `mssql` handle bound (even as `_mssql`)
/// for the lifetime of the test; dropping it stops the container.
async fn setup(
    admin_email: &str,
    org_name: &str,
) -> (
    TestApp,
    reqwest::Client,
    String,
    String,
    u16,
    ContainerAsync<MssqlServer>,
) {
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
    db.execute(
        "INSERT INTO staff (emp_id, acct, pwd, name) VALUES (@P1, @P2, @P3, @P4)",
        &[&1002i32, &"chen", &"secret2", &"陳小華"],
    )
    .await
    .unwrap();

    let app = TestApp::spawn().await;
    let (admin, body) = app.register_admin(admin_email, org_name).await;
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
        "list_query": "SELECT emp_id, name FROM staff",
    });
    let resp = admin
        .post(app.url("/orgs/me/external-auth"))
        .json(&json!({ "auth_source": "external_db", "external_auth": external_auth }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "configure failed");

    (app, admin, code, host, port, mssql)
}

fn find_user<'a>(list: &'a Value, external_key: &str) -> Option<&'a Value> {
    list.as_array()
        .unwrap()
        .iter()
        .find(|u| u["external_key"] == external_key)
}

/// 3.1 — a `list_query` containing `@account`/`@password` is rejected.
#[tokio::test]
async fn saving_list_query_with_placeholders_is_rejected() {
    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin@example.com", "Acme").await;

    let resp = admin
        .post(app.url("/orgs/me/external-auth"))
        .json(&json!({
            "auth_source": "external_db",
            "external_auth": {
                "driver": "mssql", "host": "db.local", "port": 1433, "database": "erp",
                "username": "svc", "password": "s3cret!",
                "query": "SELECT id, name FROM staff WHERE account=@account AND pass=@password",
                "key_col": "id", "display_col": "name",
                "list_query": "SELECT id, name FROM staff WHERE account=@account",
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "VALIDATION");

    // Not persisted: current_org should still be internal / no external_auth.
    let me: Value = admin
        .get(app.url("/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["current_org"]["auth_source"], "internal");
}

/// 3.2 — sync creates new AppUsers with `last_login_at = null`, `status =
/// active` for previously-unknown external_keys.
#[tokio::test]
#[serial(mssql)]
async fn sync_creates_new_shadow_users() {
    let (app, admin, _code, _host, _port, _mssql) = setup("admin2@example.com", "Acme2").await;

    let resp = admin
        .post(app.url("/orgs/me/external-auth/sync"))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let sync: Value = resp.json().await.unwrap();
    assert_eq!(status, StatusCode::OK, "sync failed: {sync}");
    assert_eq!(sync["total_rows"], 2);
    assert_eq!(sync["created"], 2);
    assert_eq!(sync["updated"], 0);
    assert_eq!(sync["skipped"].as_array().unwrap().len(), 0);

    let list: Value = admin
        .get(app.url("/app-users"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let wang = find_user(&list, "1001").expect("wang synced");
    assert_eq!(wang["display_name"], "王小明");
    assert_eq!(wang["status"], "active");
    assert_eq!(wang["auth_source"], "external");
    assert!(wang["last_login_at"].is_null());

    let chen = find_user(&list, "1002").expect("chen synced");
    assert_eq!(chen["display_name"], "陳小華");
    assert!(chen["last_login_at"].is_null());
}

/// 3.3 — sync updates `display_name` for an already-existing external
/// AppUser without touching its `last_login_at`.
#[tokio::test]
#[serial(mssql)]
async fn sync_updates_display_name_without_touching_last_login_at() {
    let (app, admin, code, host, port, _mssql) = setup("admin3@example.com", "Acme3").await;

    // Real login first, so wang has a non-null last_login_at.
    let login = app
        .client
        .post(app.url("/app/auth/login"))
        .json(&json!({ "org_code": code, "username": "wang", "password": "secret" }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);

    let list_before: Value = admin
        .get(app.url("/app-users"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let before = find_user(&list_before, "1001").expect("wang exists after login");
    let last_login_before = before["last_login_at"].as_str().unwrap().to_string();

    // Change wang's display name in the external DB, then sync.
    let mut db = mssql_client(&host, port).await;
    db.execute(
        "UPDATE staff SET name=@P1 WHERE emp_id=@P2",
        &[&"王大明", &1001i32],
    )
    .await
    .unwrap();

    let resp = admin
        .post(app.url("/orgs/me/external-auth/sync"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let sync: Value = resp.json().await.unwrap();
    // wang updates, chen (never logged in, but already synced would've been
    // created previously) — here chen doesn't exist locally yet either, so
    // this run creates chen and updates wang.
    assert_eq!(sync["created"], 1);
    assert_eq!(sync["updated"], 1);

    let list_after: Value = admin
        .get(app.url("/app-users"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let after = find_user(&list_after, "1001").expect("wang still exists");
    assert_eq!(after["display_name"], "王大明");
    assert_eq!(
        after["last_login_at"].as_str().unwrap(),
        last_login_before,
        "sync must not touch last_login_at on update"
    );
}

/// 3.4 — a local AppUser whose external_key is absent from the sync result
/// is completely unchanged after sync (no auto-disable/delete).
#[tokio::test]
#[serial(mssql)]
async fn sync_never_touches_users_absent_from_the_result() {
    let (app, admin, code, host, port, _mssql) = setup("admin4@example.com", "Acme4").await;

    // Log wang in so a local shadow user exists for emp_id 1001.
    let login = app
        .client
        .post(app.url("/app/auth/login"))
        .json(&json!({ "org_code": code, "username": "wang", "password": "secret" }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);

    let list_before: Value = admin
        .get(app.url("/app-users"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let before = find_user(&list_before, "1001")
        .expect("wang exists")
        .clone();

    // Narrow the list_query so wang (1001) is no longer returned.
    let mut db = mssql_client(&host, port).await;
    db.execute("DELETE FROM staff WHERE emp_id=1001", &[])
        .await
        .unwrap();

    let resp = admin
        .post(app.url("/orgs/me/external-auth/sync"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let sync: Value = resp.json().await.unwrap();
    assert_eq!(sync["total_rows"], 1, "only chen remains in the source");
    assert_eq!(sync["created"], 1, "chen gets created");

    let list_after: Value = admin
        .get(app.url("/app-users"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let after = find_user(&list_after, "1001").expect("wang must still be listed");
    assert_eq!(
        after, &before,
        "user absent from sync result must be byte-identical after sync"
    );
}

/// 3.5 — a row with empty/NULL key_col is skipped and reported, other rows
/// still process; response is still 200.
#[tokio::test]
#[serial(mssql)]
async fn sync_skips_rows_with_null_key_col() {
    let (app, admin, _code, host, port, _mssql) = setup("admin5@example.com", "Acme5").await;

    let mut db = mssql_client(&host, port).await;
    db.execute(
        "INSERT INTO staff (emp_id, acct, pwd, name) VALUES (NULL, @P1, @P2, @P3)",
        &[&"noone", &"x", &"Ghost"],
    )
    .await
    .unwrap();

    let resp = admin
        .post(app.url("/orgs/me/external-auth/sync"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let sync: Value = resp.json().await.unwrap();
    assert_eq!(sync["total_rows"], 3);
    assert_eq!(sync["created"], 2, "the two valid rows still process");
    let skipped = sync["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    assert!(
        skipped[0]["reason"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("key")
    );
}

/// 3.6 — key_col/display_col column-not-found fails the whole sync with no
/// writes.
#[tokio::test]
#[serial(mssql)]
async fn sync_fails_whole_batch_on_missing_column() {
    let (app, admin, code, _host, _port, _mssql) = setup("admin6@example.com", "Acme6").await;

    let resp = admin
        .post(app.url("/orgs/me/external-auth"))
        .json(&json!({
            "auth_source": "external_db",
            "external_auth": {
                "driver": "mssql",
                "host": _host, "port": _port, "database": "master",
                "username": "sa", "password": MssqlServer::DEFAULT_SA_PASSWORD,
                "query": "SELECT emp_id, name FROM staff WHERE acct=@account AND pwd=@password",
                "key_col": "emp_id", "display_col": "name",
                // list_query deliberately omits emp_id/name so column
                // resolution fails.
                "list_query": "SELECT acct FROM staff",
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "reconfigure failed");

    let resp = admin
        .post(app.url("/orgs/me/external-auth/sync"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "EXTERNAL_AUTH_SYNC_FAILED");

    // No writes happened.
    let list: Value = admin
        .get(app.url("/app-users"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);
    let _ = code;
}

/// 3.7 — sync rejected with EXTERNAL_AUTH_NOT_ENABLED when auth_source ==
/// internal, even if external_auth (incl. list_query) is configured.
#[tokio::test]
#[serial(mssql)]
async fn sync_rejected_when_auth_source_is_internal() {
    let (app, admin, _code, host, port, _mssql) = setup("admin7@example.com", "Acme7").await;

    // Switch back to internal — config (incl. list_query) stays stored.
    let resp = admin
        .post(app.url("/orgs/me/external-auth"))
        .json(&json!({ "auth_source": "internal" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.json::<Value>().await.unwrap()["auth_source"],
        "internal"
    );

    let resp = admin
        .post(app.url("/orgs/me/external-auth/sync"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "EXTERNAL_AUTH_NOT_ENABLED");
    let _ = (host, port);
}

/// 3.8 — sync rejected with a validation error when auth_source ==
/// external_db but no list_query is set.
#[tokio::test]
#[serial(mssql)]
async fn sync_rejected_when_list_query_not_configured() {
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

    let app = TestApp::spawn().await;
    let (admin, _) = app.register_admin("admin8@example.com", "Acme8").await;

    let resp = admin
        .post(app.url("/orgs/me/external-auth"))
        .json(&json!({
            "auth_source": "external_db",
            "external_auth": {
                "driver": "mssql", "host": host, "port": port, "database": "master",
                "username": "sa", "password": MssqlServer::DEFAULT_SA_PASSWORD,
                "query": "SELECT emp_id, name FROM staff WHERE acct=@account AND pwd=@password",
                "key_col": "emp_id", "display_col": "name",
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = admin
        .post(app.url("/orgs/me/external-auth/sync"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "VALIDATION");
}

/// 3.9 — member (non-admin) gets FORBIDDEN calling sync.
#[tokio::test]
#[serial(mssql)]
async fn sync_forbidden_for_non_admin() {
    let (app, admin, code, _host, _port, _mssql) = setup("admin9@example.com", "Acme9").await;
    let (member, _) = app
        .register_member(&admin, "member9@example.com", &code)
        .await;

    let resp = member
        .post(app.url("/orgs/me/external-auth/sync"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

/// 3.10 — regression: login/test-login still function on an Org that also
/// has a list_query configured (sync doesn't touch the login path).
#[tokio::test]
#[serial(mssql)]
async fn login_and_test_login_regression_with_list_query_configured() {
    let (app, admin, code, host, port, _mssql) = setup("admin10@example.com", "Acme10").await;

    let resp = admin
        .post(app.url("/orgs/me/external-auth/test-login"))
        .json(&json!({
            "external_auth": {
                "driver": "mssql", "host": host, "port": port, "database": "master",
                "username": "sa", "password": MssqlServer::DEFAULT_SA_PASSWORD,
                "query": "SELECT emp_id, name FROM staff WHERE acct=@account AND pwd=@password",
                "key_col": "emp_id", "display_col": "name",
                "list_query": "SELECT emp_id, name FROM staff",
            },
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

    let login = app
        .client
        .post(app.url("/app/auth/login"))
        .json(&json!({ "org_code": code, "username": "wang", "password": "secret" }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let lb: Value = login.json().await.unwrap();
    assert_eq!(lb["user"]["external_key"], "1001");
}
