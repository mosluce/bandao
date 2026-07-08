use std::process::ExitCode;

use bandao_api::{AppState, Config, Db, handlers, startup};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    // Load a local `.env` if present (dev convenience). Absent/real-env vars
    // win — this only fills gaps, never overrides the process environment.
    dotenvy::dotenv().ok();

    init_tracing();

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(err) => {
            tracing::error!(?err, "failed to load configuration");
            return ExitCode::from(1);
        }
    };

    let db = match Db::connect(&config.mongo_uri, &config.mongo_db).await {
        Ok(db) => db,
        Err(err) => {
            tracing::error!(?err, "failed to connect to MongoDB");
            return ExitCode::from(1);
        }
    };

    if let Err(err) = db.ensure_indexes().await {
        tracing::error!(?err, "failed to ensure indexes");
        return ExitCode::from(1);
    }

    // One-shot drift repair on the checkin status projection. See
    // `startup::repair_checkin_status_drift` for the why.
    startup::repair_checkin_status_drift(&db).await;

    let listen_addr = config.listen_addr;
    let state = AppState::new(db, config);
    let app = handlers::router(state);

    let listener = match TcpListener::bind(listen_addr).await {
        Ok(l) => l,
        Err(err) => {
            tracing::error!(?err, %listen_addr, "failed to bind listener");
            return ExitCode::from(1);
        }
    };

    tracing::info!(%listen_addr, "bandao-api listening");

    let serve = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal());
    if let Err(err) = serve.await {
        tracing::error!(?err, "server error");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn init_tracing() {
    let filter = EnvFilter::try_from_env("BANDAO_LOG")
        .or_else(|_| EnvFilter::try_new("info,bandao_api=debug"))
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(?err, "failed to install ctrl_c handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(err) => {
                tracing::error!(?err, "failed to install SIGTERM handler");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
