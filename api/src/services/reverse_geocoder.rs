//! Reverse-geocoding abstraction. The trait has one production impl
//! ([`NominatimGeocoder`]) and one test stub ([`StaticReverseGeocoder`]).
//! Failures collapse to `Option::None`: callers MUST treat that as a soft
//! failure and still record the event (with `region_name = null`).
//!
//! Why a trait when only one impl ships today: the ROADMAP entry "Reverse
//! geocoding provider 抽象" is real and near-term. Designing the trait now
//! is roughly free and lets future provider swaps stay isolated.

use std::num::NonZeroUsize;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use lru::LruCache;
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
    #[serde(default)]
    road: Option<String>,
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
    /// Smallest-grain administrative label — same fallback chain as before
    /// (suburb > city > town > village > county > state > country) so a
    /// missing road still surfaces something meaningful.
    fn district(&self) -> Option<String> {
        self.suburb
            .clone()
            .or_else(|| self.city.clone())
            .or_else(|| self.town.clone())
            .or_else(|| self.village.clone())
            .or_else(|| self.county.clone())
            .or_else(|| self.state.clone())
            .or_else(|| self.country.clone())
    }

    fn road(&self) -> Option<String> {
        self.road.clone()
    }

    /// Compose `"{district} · {road}"` when both are present. Falls back to
    /// whichever single field is set, or `None` when neither is.
    fn compose(&self) -> Option<String> {
        match (self.district(), self.road()) {
            (Some(d), Some(r)) => Some(format!("{d} · {r}")),
            (Some(d), None) => Some(d),
            (None, Some(r)) => Some(r),
            (None, None) => None,
        }
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
                ("zoom", "17"),
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
        // Prefer the structured `{district} · {road}` compose; fall back to
        // the raw `display_name` so we surface *something* even when the
        // structure is sparse.
        body.address
            .as_ref()
            .and_then(|a| a.compose())
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

/// Default LRU capacity. ~10k unique grid cells × ~200 bytes ≈ 2 MB.
pub const CACHE_CAPACITY: usize = 10_000;

/// Default per-entry TTL. Long enough that warm areas keep hitting cache,
/// short enough that the underlying Nominatim data doesn't drift.
pub const CACHE_TTL: Duration = Duration::from_secs(3600);

/// Round `(lat, lng)` to ~11 m grid cells (4 decimal places). Same building
/// or parking lot collapses to one cache key.
fn cache_key(lat: f64, lng: f64) -> (i64, i64) {
    let scale = 10_000.0_f64;
    ((lat * scale).round() as i64, (lng * scale).round() as i64)
}

#[derive(Clone)]
struct CacheEntry {
    value: Option<String>,
    expires_at: Instant,
}

/// Decorator that caches `lookup` results in a process-local LRU. Wrap any
/// `ReverseGeocoder` (production wraps [`NominatimGeocoder`]; tests typically
/// inject [`StaticReverseGeocoder`] without a cache wrapper).
pub struct CachedReverseGeocoder<G: ReverseGeocoder> {
    inner: G,
    cache: Arc<RwLock<LruCache<(i64, i64), CacheEntry>>>,
    ttl: Duration,
    now: fn() -> Instant,
}

impl<G: ReverseGeocoder> CachedReverseGeocoder<G> {
    pub fn new(inner: G) -> Self {
        Self::with_options(inner, CACHE_CAPACITY, CACHE_TTL)
    }

    /// Test-only knobs. Production code uses [`Self::new`].
    pub fn with_options(inner: G, capacity: usize, ttl: Duration) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).expect("non-zero capacity");
        Self {
            inner,
            cache: Arc::new(RwLock::new(LruCache::new(cap))),
            ttl,
            now: Instant::now,
        }
    }
}

#[async_trait]
impl<G: ReverseGeocoder> ReverseGeocoder for CachedReverseGeocoder<G> {
    async fn lookup(&self, lat: f64, lng: f64) -> Option<String> {
        let key = cache_key(lat, lng);
        let now = (self.now)();

        // Read path: take a write lock anyway because LruCache::get marks the
        // entry as recently used (which mutates the linked list). RwLock would
        // need `peek` to avoid that, but then we lose LRU semantics.
        if let Ok(mut guard) = self.cache.write()
            && let Some(entry) = guard.get(&key)
        {
            if entry.expires_at > now {
                return entry.value.clone();
            }
            // expired — fall through, will be replaced below
            guard.pop(&key);
        }

        let value = self.inner.lookup(lat, lng).await;

        if let Ok(mut guard) = self.cache.write() {
            guard.put(
                key,
                CacheEntry {
                    value: value.clone(),
                    expires_at: now + self.ttl,
                },
            );
        }

        value
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

    fn addr() -> NominatimAddress {
        NominatimAddress {
            road: None,
            suburb: None,
            city: None,
            town: None,
            village: None,
            county: None,
            state: None,
            country: None,
        }
    }

    #[test]
    fn district_prefers_finer_grain() {
        let a = NominatimAddress {
            suburb: Some("信義區".into()),
            city: Some("台北市".into()),
            country: Some("Taiwan".into()),
            ..addr()
        };
        assert_eq!(a.district().as_deref(), Some("信義區"));
    }

    #[test]
    fn district_falls_back_when_suburb_missing() {
        let a = NominatimAddress {
            city: Some("台北市".into()),
            country: Some("Taiwan".into()),
            ..addr()
        };
        assert_eq!(a.district().as_deref(), Some("台北市"));
    }

    #[test]
    fn compose_uses_district_and_road() {
        let a = NominatimAddress {
            road: Some("忠孝東路五段".into()),
            suburb: Some("信義區".into()),
            city: Some("台北市".into()),
            ..addr()
        };
        assert_eq!(a.compose().as_deref(), Some("信義區 · 忠孝東路五段"));
    }

    #[test]
    fn compose_falls_back_to_district_alone() {
        let a = NominatimAddress {
            suburb: Some("信義區".into()),
            ..addr()
        };
        assert_eq!(a.compose().as_deref(), Some("信義區"));
    }

    #[test]
    fn compose_falls_back_to_road_alone() {
        let a = NominatimAddress {
            road: Some("忠孝東路五段".into()),
            ..addr()
        };
        assert_eq!(a.compose().as_deref(), Some("忠孝東路五段"));
    }

    #[test]
    fn compose_returns_none_when_empty() {
        assert_eq!(addr().compose(), None);
    }

    #[test]
    fn compose_handles_western_address() {
        let a = NominatimAddress {
            road: Some("Stevens Creek Boulevard".into()),
            city: Some("Cupertino".into()),
            ..addr()
        };
        assert_eq!(
            a.compose().as_deref(),
            Some("Cupertino · Stevens Creek Boulevard"),
        );
    }

    // ---- CachedReverseGeocoder ----

    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Counts upstream calls so cache hits / misses are observable.
    struct CountingGeocoder {
        calls: Arc<AtomicUsize>,
        response: Option<String>,
    }

    #[async_trait]
    impl ReverseGeocoder for CountingGeocoder {
        async fn lookup(&self, _lat: f64, _lng: f64) -> Option<String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.response.clone()
        }
    }

    #[test]
    fn cache_key_4_decimals() {
        // 25.03301 rounds to 25.0330 → key 250_330
        assert_eq!(cache_key(25.03301, 121.56541).0, 250_330);
        // close enough to share a key
        assert_eq!(cache_key(25.0330, 121.5654), cache_key(25.03304, 121.56539));
        // far enough apart to differ
        assert_ne!(cache_key(25.0330, 121.5654), cache_key(25.0335, 121.5654));
    }

    #[tokio::test]
    async fn cache_hit_skips_inner() {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner = CountingGeocoder {
            calls: calls.clone(),
            response: Some("信義區 · 忠孝東路五段".into()),
        };
        let cached = CachedReverseGeocoder::new(inner);

        let _ = cached.lookup(25.0330, 121.5654).await;
        let _ = cached.lookup(25.03301, 121.56541).await; // same key
        let _ = cached.lookup(25.03304, 121.56539).await; // same key

        assert_eq!(calls.load(Ordering::SeqCst), 1, "inner called once");
    }

    #[tokio::test]
    async fn cache_negative_result_is_cached() {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner = CountingGeocoder {
            calls: calls.clone(),
            response: None,
        };
        let cached = CachedReverseGeocoder::new(inner);

        assert_eq!(cached.lookup(0.0, 0.0).await, None);
        assert_eq!(cached.lookup(0.0, 0.0).await, None);

        assert_eq!(calls.load(Ordering::SeqCst), 1, "None is cached too");
    }

    #[tokio::test]
    async fn cache_distinct_keys_each_call_inner() {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner = CountingGeocoder {
            calls: calls.clone(),
            response: Some("X".into()),
        };
        let cached = CachedReverseGeocoder::new(inner);

        let _ = cached.lookup(25.0330, 121.5654).await;
        let _ = cached.lookup(25.0335, 121.5654).await; // different 4-decimal key

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn cache_evicts_oldest_when_full() {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner = CountingGeocoder {
            calls: calls.clone(),
            response: Some("X".into()),
        };
        let cached = CachedReverseGeocoder::with_options(inner, 2, CACHE_TTL);

        // Fill: A, B → cache [A (LRU), B (MRU)]
        let _ = cached.lookup(0.0, 0.0).await; // A
        let _ = cached.lookup(0.0, 1.0).await; // B
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        // Hit A → cache [B (LRU), A (MRU)]
        let _ = cached.lookup(0.0, 0.0).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2, "A still cached");

        // Insert C → evicts B (LRU) → cache [A, C]
        let _ = cached.lookup(0.0, 2.0).await; // C
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        // A still cached
        let _ = cached.lookup(0.0, 0.0).await;
        assert_eq!(calls.load(Ordering::SeqCst), 3, "A still cached");

        // B was evicted → miss
        let _ = cached.lookup(0.0, 1.0).await;
        assert_eq!(calls.load(Ordering::SeqCst), 4, "B was evicted");
    }

    #[tokio::test]
    async fn cache_ttl_expires() {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner = CountingGeocoder {
            calls: calls.clone(),
            response: Some("X".into()),
        };
        // 1 ms TTL — sleep past it.
        let cached = CachedReverseGeocoder::with_options(inner, 100, Duration::from_millis(1));

        let _ = cached.lookup(0.0, 0.0).await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        let _ = cached.lookup(0.0, 0.0).await;

        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "second lookup re-hit inner after TTL",
        );
    }
}
