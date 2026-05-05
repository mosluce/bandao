## 1. Cargo dependency

- [x] 1.1 Add `lru = "0.12"` (or latest 0.x compatible) to `api/Cargo.toml` `[dependencies]`. This is the only new direct dep — TTL is layered on top in our own struct.
- [x] 1.2 Run `cargo build -p argus_api` to verify the dep resolves and compiles clean.

## 2. NominatimGeocoder updates

- [x] 2.1 In `api/src/services/reverse_geocoder.rs`, add `road: Option<String>` to `NominatimAddress` (with `#[serde(default)]`).
- [x] 2.2 Replace `best_label()` on `NominatimAddress` with two helpers: `district()` (the existing fallback chain `suburb > city > town > village > county > state > country`) and `road()` (just returns `self.road`).
- [x] 2.3 In `NominatimGeocoder::lookup`, after parsing the response, compose the label as: both present → `format!("{} · {}", district, road)`; one present → that one; neither → existing `display_name` fallback. Existing fail-soft path unchanged.
- [x] 2.4 Bump the request `zoom` query param from `"14"` to `"17"`. `addressdetails=1` stays.
- [x] 2.5 Update the `best_label_*` unit tests:
  - rename to reflect compose semantics
  - new test `compose_uses_district_and_road` (both present → composed)
  - new test `compose_falls_back_to_district_alone`
  - new test `compose_falls_back_to_road_alone`
  - keep the existing `falls_back_to_display_name` style test
- [x] 2.6 `cargo test -p argus_api services::reverse_geocoder` passes.

## 3. CachedReverseGeocoder

- [x] 3.1 Create new file `api/src/services/reverse_geocoder/cache.rs` (split the module — keep the trait + Nominatim impl in `mod.rs` or a sibling `nominatim.rs`). Or keep inline at the bottom of `reverse_geocoder.rs` — designer call (see design.md D3 / D6).
- [x] 3.2 Define `pub struct CachedReverseGeocoder<G: ReverseGeocoder>` holding `inner: G`, `cache: Arc<RwLock<LruCache<(i64, i64), CacheEntry>>>`, and the TTL/capacity constants. `CacheEntry { value: Option<String>, expires_at: Instant }`.
- [x] 3.3 Public constants `CACHE_CAPACITY: usize = 10_000` and `CACHE_TTL: Duration = Duration::from_secs(3600)`. Constructor `pub fn new(inner: G) -> Self` builds the LRU with capacity.
- [x] 3.4 Key helper: `fn key(lat: f64, lng: f64) -> (i64, i64)` — multiply by 10_000, round to nearest, cast to i64. Cover the negative-coordinate edge case.
- [x] 3.5 `impl<G: ReverseGeocoder> ReverseGeocoder for CachedReverseGeocoder<G>` with async `lookup`:
  - Compute key
  - Take read lock, peek at cache: if entry exists AND `expires_at > Instant::now()`, return `entry.value.clone()`
  - Drop read lock; call `self.inner.lookup(lat, lng).await`
  - Take write lock, insert/update entry with new `expires_at`
  - Return value
- [x] 3.6 Unit tests in the same module:
  - `cache_hit_skips_inner` — wrap a counting fake inner, second lookup with same coords doesn't bump the counter
  - `cache_negative_result` — None gets cached too
  - `cache_ttl_expires` — manipulate clock or use a configurable `now: Fn() -> Instant` for test-injection
  - `cache_evicts_oldest_when_full` — fill past capacity, oldest key returns inner-call again
  - `cache_key_4_decimals` — coordinates that round to same 4-decimal key share the cache; coordinates that don't are independent
- [x] 3.7 `cargo test -p argus_api services::reverse_geocoder::cache` passes.

## 4. Wiring

- [x] 4.1 In `api/src/state.rs`, change `AppState::new` to construct the geocoder as `Arc::new(CachedReverseGeocoder::new(NominatimGeocoder::new()))`. Keep `with_geocoder<G>(...)` test escape hatch unchanged (passes `G` straight through, no auto-cache).
- [x] 4.2 Verify `cargo test -p argus_api` (full suite) is green — the `app_checkin` integration tests use `with_geocoder(StaticReverseGeocoder::new(...))` so they should be unaffected by D3.

## 5. admin-web privacy policy update

- [x] 5.1 In `admin-web/pages/privacy.vue`, update the Section 2 「出勤事件」 example: change `（如「信義區」這類區域名稱）` to a phrase reflecting the new format, e.g. `（如「信義區 · 忠孝東路五段」這類包含區域與街道的名稱）`.
- [x] 5.2 Append a clarifying sentence in the same paragraph: `本平台僅收集到街道層級，不包含巷弄與門牌號碼。`
- [x] 5.3 If there's a "保留期" or "蒐集範圍" summary box that also mentions the example, update it consistently.
- [x] 5.4 `pnpm dev` smoke locally: navigate to `/privacy`, eyeball the wording.

## 6. Documentation

- [x] 6.1 Update `api/README.md` (or its reverse-geocoding section if any) to mention the new compose format and the cache layer (capacity, TTL, key precision). Keep it short — one paragraph in the relevant section.
- [x] 6.2 No root-README change needed; the system-level capability description hasn't changed.

## 7. CI verification

- [x] 7.1 Run `cargo fmt --all -- --check` (or whatever the api convention is) and fix any formatting issues.
- [x] 7.2 Run `cargo clippy -p argus_api --all-targets -- -D warnings` clean.
- [x] 7.3 Push and verify the api workflow passes on the PR / branch.

## 8. Smoke (manual)

- [x] 8.1 Run `cargo run -p argus_api` against a real Mongo. Use a known coord (Taipei `25.0330, 121.5654` near 信義區) via `POST /app/checkin/events` and confirm the inserted event's `region_name` matches the expected `"信義區 · 忠孝東路五段"` shape (exact street depends on geographic data — accept any plausible compose).
- [x] 8.2 Submit two events with the same lat/lng within seconds — observe via tracing logs that the second one is cache-hit (no Nominatim call). Optional: add a `tracing::debug!` line in the cache hit path before merging if it's not already there.
- [x] 8.3 Submit an event with coords in a remote / sea area to verify fail-soft → `region_name = null` still works.
