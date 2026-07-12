use std::sync::Arc;

use crate::config::Config;
use crate::db::Db;
use crate::services::email::{NoopEmailSender, ResendEmailSender, SharedEmailSender};
use crate::services::reverse_geocoder::{
    CachedReverseGeocoder, NominatimGeocoder, ReverseGeocoder, SharedReverseGeocoder,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub config: Arc<Config>,
    /// Reverse-geocoder used by `POST /app/checkin/events`. Failures are
    /// fail-soft: the event still records with `region_name = null`. Tests
    /// substitute `StaticReverseGeocoder` via [`AppState::with_geocoder`].
    pub geocoder: SharedReverseGeocoder,
    /// Transactional email sender (password-reset links, and future
    /// email-based features). Falls back to `NoopEmailSender` when
    /// `Config::resend_api_key` is unset. Tests substitute
    /// `RecordingEmailSender` via [`AppState::with_email_sender`].
    pub email: SharedEmailSender,
}

impl AppState {
    pub fn new(db: Db, config: Config) -> Self {
        // Wrap Nominatim in an LRU cache decorator — see
        // `services/reverse_geocoder.rs`. Tests bypass this by injecting a
        // raw geocoder via `with_geocoder`.
        let geocoder: SharedReverseGeocoder =
            Arc::new(CachedReverseGeocoder::new(NominatimGeocoder::new()));
        let email: SharedEmailSender = match &config.resend_api_key {
            Some(key) => Arc::new(ResendEmailSender::new(
                key.clone(),
                config.email_from_address.clone(),
            )),
            None => Arc::new(NoopEmailSender),
        };
        Self {
            db: Arc::new(db),
            config: Arc::new(config),
            geocoder,
            email,
        }
    }

    /// Construct with a custom geocoder — primarily for tests that need to
    /// avoid hitting Nominatim.
    pub fn with_geocoder<G>(db: Db, config: Config, geocoder: G) -> Self
    where
        G: ReverseGeocoder + 'static,
    {
        let email: SharedEmailSender = match &config.resend_api_key {
            Some(key) => Arc::new(ResendEmailSender::new(
                key.clone(),
                config.email_from_address.clone(),
            )),
            None => Arc::new(NoopEmailSender),
        };
        Self {
            db: Arc::new(db),
            config: Arc::new(config),
            geocoder: Arc::new(geocoder),
            email,
        }
    }

    /// Construct with a custom email sender — primarily for tests that need
    /// to assert on sent email (e.g. `RecordingEmailSender`) without hitting
    /// Resend. Takes an already-`Arc`-wrapped sender (rather than a bare
    /// generic, unlike `with_geocoder`) so the caller can keep its own
    /// handle to inspect state recorded on the sender after the request
    /// completes.
    pub fn with_email_sender(db: Db, config: Config, email: SharedEmailSender) -> Self {
        let geocoder: SharedReverseGeocoder =
            Arc::new(CachedReverseGeocoder::new(NominatimGeocoder::new()));
        Self {
            db: Arc::new(db),
            config: Arc::new(config),
            geocoder,
            email,
        }
    }
}
