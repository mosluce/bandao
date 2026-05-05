use std::sync::Arc;

use crate::config::Config;
use crate::db::Db;
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
}

impl AppState {
    pub fn new(db: Db, config: Config) -> Self {
        // Wrap Nominatim in an LRU cache decorator — see
        // `services/reverse_geocoder.rs`. Tests bypass this by injecting a
        // raw geocoder via `with_geocoder`.
        let geocoder: SharedReverseGeocoder =
            Arc::new(CachedReverseGeocoder::new(NominatimGeocoder::new()));
        Self {
            db: Arc::new(db),
            config: Arc::new(config),
            geocoder,
        }
    }

    /// Construct with a custom geocoder — primarily for tests that need to
    /// avoid hitting Nominatim.
    pub fn with_geocoder<G>(db: Db, config: Config, geocoder: G) -> Self
    where
        G: ReverseGeocoder + 'static,
    {
        Self {
            db: Arc::new(db),
            config: Arc::new(config),
            geocoder: Arc::new(geocoder),
        }
    }
}
