//! Reverse-geocoding abstraction. The trait has one production impl
//! ([`NominatimGeocoder`]) and one test stub ([`StaticReverseGeocoder`]).
//! Failures collapse to `Option::None`: callers MUST treat that as a soft
//! failure and still record the event (with `region_name = null`).
//!
//! Why a trait when only one impl ships today: the ROADMAP entry "Reverse
//! geocoding provider 抽象" is real and near-term. Designing the trait now
//! is roughly free and lets future provider swaps stay isolated.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

/// Default timeout per request — Nominatim may be slow but the event-submit
/// path can't afford to block the AppUser indefinitely.
pub const NOMINATIM_TIMEOUT: Duration = Duration::from_secs(2);

/// Default User-Agent. Nominatim's usage policy requires a unique UA — this
/// one is identifiable as argus.
pub const NOMINATIM_USER_AGENT: &str = "argus-api/0.1.0 (https://github.com/mosluce/argus)";

/// Default Accept-Language preference. Tests / runtime can override.
pub const NOMINATIM_ACCEPT_LANGUAGE: &str = "zh-TW,en";

#[async_trait]
pub trait ReverseGeocoder: Send + Sync {
    /// Resolve `(lat, lng)` to a human-readable region label. `None` on
    /// any failure — callers must NOT treat `None` as fatal.
    async fn lookup(&self, lat: f64, lng: f64) -> Option<String>;
}

/// Type alias for the trait object kept in `AppState`.
pub type SharedReverseGeocoder = Arc<dyn ReverseGeocoder>;

/// Public Nominatim implementation. Creates one `reqwest::Client` at
/// construction, reuses it for every lookup. All errors collapse to `None`.
#[derive(Clone)]
pub struct NominatimGeocoder {
    client: reqwest::Client,
    base_url: String,
    accept_language: String,
}

impl NominatimGeocoder {
    pub fn new() -> Self {
        Self::with_options(
            "https://nominatim.openstreetmap.org/reverse",
            NOMINATIM_USER_AGENT,
            NOMINATIM_ACCEPT_LANGUAGE,
            NOMINATIM_TIMEOUT,
        )
    }

    /// Construct with explicit knobs. Test-only convenience: production code
    /// uses [`Self::new`].
    pub fn with_options(
        base_url: &str,
        user_agent: &str,
        accept_language: &str,
        timeout: Duration,
    ) -> Self {
        // If the client builder fails for any reason (very unusual on
        // rustls-tls), fall back to a default client and log. Callers will
        // see `None` for every lookup but the server stays up.
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(user_agent)
            .build()
            .unwrap_or_else(|err| {
                tracing::error!(?err, "failed to build nominatim http client; using default");
                reqwest::Client::new()
            });
        Self {
            client,
            base_url: base_url.to_string(),
            accept_language: accept_language.to_string(),
        }
    }
}

impl Default for NominatimGeocoder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct NominatimResponse {
    /// Free-text address label. Nominatim sometimes returns this even when
    /// individual `address` fields are sparse.
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    address: Option<NominatimAddress>,
}

#[derive(Debug, Deserialize)]
struct NominatimAddress {
    /// City / town / village in priority order. We collapse to one label
    /// rather than trying to be clever about administrative hierarchy.
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    town: Option<String>,
    #[serde(default)]
    village: Option<String>,
    #[serde(default)]
    suburb: Option<String>,
    #[serde(default)]
    county: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    country: Option<String>,
}

impl NominatimAddress {
    /// Pick the best-fitting label. Prefers smallest-grain location (suburb >
    /// city > town > village > county > state > country) so admin-web shows
    /// "信義區" instead of "Taiwan" when possible.
    fn best_label(&self) -> Option<String> {
        self.suburb
            .clone()
            .or_else(|| self.city.clone())
            .or_else(|| self.town.clone())
            .or_else(|| self.village.clone())
            .or_else(|| self.county.clone())
            .or_else(|| self.state.clone())
            .or_else(|| self.country.clone())
    }
}

#[async_trait]
impl ReverseGeocoder for NominatimGeocoder {
    async fn lookup(&self, lat: f64, lng: f64) -> Option<String> {
        let resp = match self
            .client
            .get(&self.base_url)
            .query(&[
                ("format", "jsonv2"),
                ("lat", lat.to_string().as_str()),
                ("lon", lng.to_string().as_str()),
                ("zoom", "14"),
                ("addressdetails", "1"),
            ])
            .header("Accept-Language", self.accept_language.as_str())
            .send()
            .await
        {
            Ok(r) => r,
            Err(err) => {
                tracing::debug!(?err, lat, lng, "nominatim request failed");
                return None;
            }
        };
        if !resp.status().is_success() {
            tracing::debug!(status = %resp.status(), lat, lng, "nominatim non-2xx");
            return None;
        }
        let body: NominatimResponse = match resp.json().await {
            Ok(b) => b,
            Err(err) => {
                tracing::debug!(?err, lat, lng, "nominatim parse failed");
                return None;
            }
        };
        // Prefer best_label from structured address; fall back to the raw
        // `display_name` so we surface *something* even when the structure
        // is sparse.
        body.address
            .as_ref()
            .and_then(|a| a.best_label())
            .or(body.display_name)
            .filter(|s| !s.trim().is_empty())
    }
}

/// Test-only stub. Always returns the configured value.
#[derive(Clone, Debug)]
pub struct StaticReverseGeocoder {
    pub fixed: Option<String>,
}

impl StaticReverseGeocoder {
    pub fn new(fixed: Option<String>) -> Self {
        Self { fixed }
    }
}

#[async_trait]
impl ReverseGeocoder for StaticReverseGeocoder {
    async fn lookup(&self, _lat: f64, _lng: f64) -> Option<String> {
        self.fixed.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn static_stub_returns_fixed() {
        let stub = StaticReverseGeocoder::new(Some("Taipei City".to_string()));
        assert_eq!(stub.lookup(0.0, 0.0).await.as_deref(), Some("Taipei City"));
    }

    #[tokio::test]
    async fn static_stub_can_return_none() {
        let stub = StaticReverseGeocoder::new(None);
        assert_eq!(stub.lookup(0.0, 0.0).await, None);
    }

    #[test]
    fn best_label_prefers_finer_grain() {
        let addr = NominatimAddress {
            suburb: Some("信義區".into()),
            city: Some("台北市".into()),
            town: None,
            village: None,
            county: None,
            state: None,
            country: Some("Taiwan".into()),
        };
        assert_eq!(addr.best_label().as_deref(), Some("信義區"));
    }

    #[test]
    fn best_label_falls_back_when_suburb_missing() {
        let addr = NominatimAddress {
            suburb: None,
            city: Some("台北市".into()),
            town: None,
            village: None,
            county: None,
            state: None,
            country: Some("Taiwan".into()),
        };
        assert_eq!(addr.best_label().as_deref(), Some("台北市"));
    }
}
