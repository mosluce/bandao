#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bandao_api::services::reverse_geocoder::ReverseGeocoder;
use bandao_api::{AppState, Config, Db, handlers};
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

    /// Spawn with a custom geocoder. Use the `StaticReverseGeocoder` from
    /// `bandao_api::services::reverse_geocoder` to control whether events
    /// record `region_name` or store `null`.
    pub async fn spawn_with_geocoder<G>(geocoder: G) -> Self
    where
        G: ReverseGeocoder + 'static,
    {
        Self::spawn_inner(
            |cfg| {
                cfg.session_ttl = Duration::from_secs(60 * 60);
            },
            Some(Box::new(|db, config| {
                AppState::with_geocoder(db, config, geocoder)
            })),
        )
        .await
    }

    pub async fn spawn_with<F>(mut tweak: F) -> Self
    where
        F: FnMut(&mut Config),
    {
        Self::spawn_inner(|cfg| tweak(cfg), None).await
    }

    async fn spawn_inner<F>(
        mut tweak: F,
        state_builder: Option<Box<dyn FnOnce(Db, Config) -> AppState + Send>>,
    ) -> Self
    where
        F: FnMut(&mut Config),
    {
        let mongo = Mongo::default()
            .start()
            .await
            .expect("failed to start mongo container");
        let host = mongo.get_host().await.expect("mongo host");
        let port = mongo.get_host_port_ipv4(27017).await.expect("mongo port");
        let mongo_uri = format!("mongodb://{host}:{port}");
        let mongo_db = format!("bandao_test_{}", ObjectId::new().to_hex());

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

        let state = match state_builder {
            Some(build) => build(db, config),
            None => AppState::new(db, config),
        };
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
    pub fn db(&self) -> Arc<bandao_api::Db> {
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
    pub async fn register_admin(&self, email: &str, org_name: &str) -> (reqwest::Client, Value) {
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
        assert_eq!(
            resp.status(),
            reqwest::StatusCode::OK,
            "register create failed"
        );
        let body: Value = resp.json().await.expect("register body json");
        (client, body)
    }

    /// Register a new identity that submits a pending join_request via
    /// `mode=join`. Returns the joiner's client + register response body
    /// (zero-org state). For the legacy "register-and-also-be-a-member"
    /// pattern that most tests want, use `register_member_approved`.
    pub async fn register_member_pending(
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
        assert_eq!(
            resp.status(),
            reqwest::StatusCode::OK,
            "register join failed"
        );
        let body: Value = resp.json().await.expect("register join body");
        (client, body)
    }

    /// Register a member and have the admin approve them in one shot. The
    /// returned body is the joiner's `/me` after the approve, mirroring the
    /// pre-approval-flow behavior of `register_member`.
    pub async fn register_member_approved(
        &self,
        admin: &reqwest::Client,
        email: &str,
        org_code: &str,
    ) -> (reqwest::Client, Value) {
        let (joiner, _) = self.register_member_pending(email, org_code).await;
        // Admin lists pending requests and approves the matching one.
        let pending: Value = admin
            .get(self.url("/orgs/me/join-requests"))
            .send()
            .await
            .expect("list pending")
            .json()
            .await
            .expect("pending json");
        let request_id = pending
            .as_array()
            .and_then(|arr| arr.iter().find(|r| r["email"] == email))
            .and_then(|r| r["id"].as_str())
            .unwrap_or_else(|| panic!("no pending request for {email}"))
            .to_string();
        let approve = admin
            .post(self.url(&format!("/orgs/me/join-requests/{request_id}/approve")))
            .send()
            .await
            .expect("approve");
        assert_eq!(approve.status(), reqwest::StatusCode::NO_CONTENT);

        // Joiner refreshes /me — but their session has current_org=null
        // until we explicitly switch them in. Switch the joiner's session
        // to the just-approved org so their `/me` shape mirrors the legacy
        // register_member return.
        let me_after_first = joiner
            .get(self.url("/me"))
            .send()
            .await
            .expect("me after approve")
            .json::<Value>()
            .await
            .expect("me json");
        let org_id = me_after_first["memberships"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|m| m["org"]["id"].as_str())
            .unwrap_or_else(|| panic!("no membership after approve"))
            .to_string();
        let switched = joiner
            .post(self.url("/me/current-org"))
            .json(&json!({ "org_id": org_id }))
            .send()
            .await
            .expect("switch current_org")
            .json::<Value>()
            .await
            .expect("switched json");
        (joiner, switched)
    }

    /// Backward-compat alias matching the pre-approval-flow signature most
    /// tests still use. Requires an admin client to do the approve step.
    pub async fn register_member(
        &self,
        admin: &reqwest::Client,
        email: &str,
        org_code: &str,
    ) -> (reqwest::Client, Value) {
        self.register_member_approved(admin, email, org_code).await
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

    /// Admin builder: create an AppUser via `POST /app-users` against
    /// `admin_client`'s current Org. Returns the parsed `CreateAppUserResponse`
    /// body (`{ user, initial_password }`).
    pub async fn create_app_user(
        &self,
        admin_client: &reqwest::Client,
        username: &str,
        display_name: &str,
    ) -> Value {
        let resp = admin_client
            .post(self.url("/app-users"))
            .json(&json!({ "username": username, "display_name": display_name }))
            .send()
            .await
            .expect("send create app user");
        assert_eq!(
            resp.status(),
            reqwest::StatusCode::CREATED,
            "create_app_user failed: status={}",
            resp.status()
        );
        resp.json().await.expect("create_app_user body")
    }

    /// Mobile-side builder: hit `POST /app/auth/login` with a fresh client
    /// (no shared cookie jar — bearer auth doesn't need one). Returns
    /// `(reqwest::Client, body)` for the caller to make subsequent
    /// `Authorization: Bearer <token>` requests via `app_get` / `app_post`.
    pub async fn app_login(
        &self,
        org_code: &str,
        username: &str,
        password: &str,
    ) -> (reqwest::Client, Value) {
        let client = self.fresh_client();
        let resp = client
            .post(self.url("/app/auth/login"))
            .json(&json!({
                "org_code": org_code,
                "username": username,
                "password": password,
            }))
            .send()
            .await
            .expect("send app login");
        assert_eq!(
            resp.status(),
            reqwest::StatusCode::OK,
            "app_login failed: status={}",
            resp.status()
        );
        let body: Value = resp.json().await.expect("app_login body");
        (client, body)
    }

    /// Convenience: send an authenticated `GET /app/...` request, attaching
    /// `Authorization: Bearer <token>` from a previous `app_login` body.
    pub fn app_get(
        &self,
        client: &reqwest::Client,
        token: &str,
        path: &str,
    ) -> reqwest::RequestBuilder {
        client
            .get(self.url(path))
            .header("Authorization", format!("Bearer {token}"))
    }

    /// Convenience: send an authenticated `POST /app/...` request.
    pub fn app_post(
        &self,
        client: &reqwest::Client,
        token: &str,
        path: &str,
    ) -> reqwest::RequestBuilder {
        client
            .post(self.url(path))
            .header("Authorization", format!("Bearer {token}"))
    }

    /// Submit a checkin event via `POST /app/checkin/events`. Returns the
    /// raw response so the caller can assert on status + body shape.
    pub async fn submit_checkin_event(
        &self,
        client: &reqwest::Client,
        token: &str,
        event_type: &str,
        lat: f64,
        lng: f64,
        occurred_at_client: &str,
    ) -> reqwest::Response {
        self.submit_checkin_event_with(
            client,
            token,
            json!({
                "event_type": event_type,
                "lat": lat,
                "lng": lng,
                "occurred_at_client": occurred_at_client,
            }),
        )
        .await
    }

    /// Lower-level variant: pass an arbitrary JSON body so tests can
    /// exercise `manual_label`, `accuracy`, etc.
    pub async fn submit_checkin_event_with(
        &self,
        client: &reqwest::Client,
        token: &str,
        body: Value,
    ) -> reqwest::Response {
        self.app_post(client, token, "/app/checkin/events")
            .json(&body)
            .send()
            .await
            .expect("submit_checkin_event")
    }

    /// Bootstrap an AppUser ready to submit checkin events: register
    /// dashboard admin, create the AppUser, log them in via `/app/auth/login`,
    /// clear the forced-password gate. Returns `(admin_client, org_code,
    /// app_user_id_hex, app_client, app_token, current_password)`.
    pub async fn seed_app_user_ready_to_checkin(
        &self,
        admin_email: &str,
        org_name: &str,
        username: &str,
        display_name: &str,
    ) -> (
        reqwest::Client,
        String,
        String,
        reqwest::Client,
        String,
        String,
    ) {
        let (admin, body) = self.register_admin(admin_email, org_name).await;
        let org_code = body["current_org"]["code"].as_str().unwrap().to_string();
        let create_body = self.create_app_user(&admin, username, display_name).await;
        let app_user_id = create_body["user"]["id"].as_str().unwrap().to_string();
        let initial_password = create_body["initial_password"]
            .as_str()
            .unwrap()
            .to_string();
        let (app_client, login_body) = self.app_login(&org_code, username, &initial_password).await;
        let token = login_body["token"].as_str().unwrap().to_string();
        // Clear `needs_password_change` so /app/checkin/* doesn't 423.
        let new_password = "newpass!secure".to_string();
        let resp = self
            .app_post(&app_client, &token, "/app/me/password")
            .json(&json!({
                "current_password": initial_password,
                "new_password": new_password,
            }))
            .send()
            .await
            .expect("change password");
        assert_eq!(
            resp.status(),
            reqwest::StatusCode::NO_CONTENT,
            "change_password failed"
        );
        (
            admin,
            org_code,
            app_user_id,
            app_client,
            token,
            new_password,
        )
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

/// Build an RFC3339 timestamp `now + minute` minutes (negative = past).
/// Anchored on `now()` so events stay within the 1h skew threshold regardless
/// of when the test runs — a fixed unix base would silently flip
/// `has_skew_warning` once enough wall time elapses.
pub fn ts(minute: i64) -> String {
    let now = ::time::OffsetDateTime::now_utc().unix_timestamp();
    let dt = ::time::OffsetDateTime::from_unix_timestamp(now + minute * 60).unwrap();
    dt.format(&::time::format_description::well_known::Rfc3339)
        .unwrap()
}
