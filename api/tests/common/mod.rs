#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use argus_api::{AppState, Config, Db, handlers};
use bson::oid::ObjectId;
use reqwest::redirect::Policy;
use serde_json::{Value, json};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mongo::Mongo;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub struct TestApp {
    pub base_url: String,
    pub state: AppState,
    pub client: reqwest::Client,
    _mongo: ContainerAsync<Mongo>,
    server: Option<JoinHandle<()>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl TestApp {
    pub async fn spawn() -> Self {
        Self::spawn_with(|cfg| {
            cfg.session_ttl = Duration::from_secs(60 * 60);
        })
        .await
    }

    pub async fn spawn_with<F>(mut tweak: F) -> Self
    where
        F: FnMut(&mut Config),
    {
        let mongo = Mongo::default()
            .start()
            .await
            .expect("failed to start mongo container");
        let host = mongo.get_host().await.expect("mongo host");
        let port = mongo
            .get_host_port_ipv4(27017)
            .await
            .expect("mongo port");
        let mongo_uri = format!("mongodb://{host}:{port}");
        let mongo_db = format!("argus_test_{}", ObjectId::new().to_hex());

        let mut config = Config {
            mongo_uri: mongo_uri.clone(),
            mongo_db: mongo_db.clone(),
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            session_ttl: Duration::from_secs(60 * 60),
            cookie_domain: None,
            cookie_secure: false,
            allowed_origin: None,
        };
        tweak(&mut config);

        let db = Db::connect(&config.mongo_uri, &config.mongo_db)
            .await
            .expect("connect mongo");
        db.ensure_indexes().await.expect("ensure indexes");

        let state = AppState::new(db, config);
        let app = handlers::router(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind random port");
        let local_addr: SocketAddr = listener.local_addr().expect("local addr");
        let base_url = format!("http://{local_addr}");

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        let client = reqwest::Client::builder()
            .cookie_store(true)
            .redirect(Policy::none())
            .build()
            .expect("reqwest client");

        Self {
            base_url,
            state,
            client,
            _mongo: mongo,
            server: Some(server),
            shutdown: Some(shutdown_tx),
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Borrow the inner DB handle for direct fixture manipulation.
    pub fn db(&self) -> Arc<argus_api::Db> {
        self.state.db.clone()
    }

    /// Spawn a fresh reqwest client with its own cookie jar — convenient when
    /// a test needs to act as a different identity / session than `self.client`.
    pub fn fresh_client(&self) -> reqwest::Client {
        reqwest::Client::builder()
            .cookie_store(true)
            .redirect(Policy::none())
            .build()
            .expect("fresh reqwest client")
    }

    /// Register the first identity in an Org via `mode=create`. Returns the
    /// authenticated client and the parsed `AuthResponse`-shaped body.
    pub async fn register_admin(
        &self,
        email: &str,
        org_name: &str,
    ) -> (reqwest::Client, Value) {
        let client = self.fresh_client();
        let resp = client
            .post(self.url("/auth/register"))
            .json(&json!({
                "mode": "create",
                "email": email,
                "password": "hunter2hunter2",
                "org_name": org_name,
            }))
            .send()
            .await
            .expect("send register create");
        assert_eq!(resp.status(), reqwest::StatusCode::OK, "register create failed");
        let body: Value = resp.json().await.expect("register body json");
        (client, body)
    }

    /// Register a new identity that joins an existing Org via `mode=join`.
    pub async fn register_member(
        &self,
        email: &str,
        org_code: &str,
    ) -> (reqwest::Client, Value) {
        let client = self.fresh_client();
        let resp = client
            .post(self.url("/auth/register"))
            .json(&json!({
                "mode": "join",
                "email": email,
                "password": "hunter2hunter2",
                "org_code": org_code,
            }))
            .send()
            .await
            .expect("send register join");
        assert_eq!(resp.status(), reqwest::StatusCode::OK, "register join failed");
        let body: Value = resp.json().await.expect("register join body");
        (client, body)
    }

    /// Log in an existing identity. Returns the cookie-bearing client + body.
    pub async fn login(&self, email: &str, password: &str) -> (reqwest::Client, Value) {
        let client = self.fresh_client();
        let resp = client
            .post(self.url("/auth/login"))
            .json(&json!({ "email": email, "password": password }))
            .send()
            .await
            .expect("send login");
        assert_eq!(resp.status(), reqwest::StatusCode::OK, "login failed");
        let body: Value = resp.json().await.expect("login body");
        (client, body)
    }

    /// Count how many membership rows the given user has.
    pub async fn membership_count(&self, user_id: ObjectId) -> u64 {
        self.db()
            .dashboard_memberships
            .count_by_user(user_id)
            .await
            .expect("membership count")
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.server.take() {
            handle.abort();
        }
    }
}

/// Pull `body["current_org"]["code"]` as an owned `String`. Tests use it
/// pervasively to feed new join requests.
pub fn current_org_code(body: &Value) -> String {
    body["current_org"]["code"]
        .as_str()
        .unwrap_or_else(|| panic!("expected current_org.code in {body}"))
        .to_string()
}

/// Pull `body["current_org"]["id"]` as an owned `String`.
pub fn current_org_id(body: &Value) -> String {
    body["current_org"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("expected current_org.id in {body}"))
        .to_string()
}

/// Pull `body["user"]["id"]` as an owned `String`.
pub fn user_id(body: &Value) -> String {
    body["user"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("expected user.id in {body}"))
        .to_string()
}
