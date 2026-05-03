#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use argus_api::{AppState, Config, Db, handlers};
use bson::oid::ObjectId;
use reqwest::redirect::Policy;
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
