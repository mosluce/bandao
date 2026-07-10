//! Legacy check-in data backfill: per-Org declarative config (`Org.settings.legacy_backfill`)
//! describing how to connect to and map a customer's legacy MongoDB into our
//! `checkin_events` shape, a provider that runs one AppUser's backfill, and a
//! persistent worker loop that processes a lightweight Mongo-backed job queue
//! (enqueued at first login — see `handlers::app_auth::login`).

pub mod provider;
pub mod worker;

pub use provider::{
    BackfillOutcome, LegacyBackfillError, MappedEvent, PreviewOutcome, preview_mapped,
    run_backfill, validate_config,
};
pub use worker::run_worker_loop;
