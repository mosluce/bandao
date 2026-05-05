## 1. Domain + Cargo

- [x] 1.1 Add `domain::LocationPing` struct to `api/src/domain.rs` with fields `id: ObjectId`, `org_id: ObjectId`, `app_user_id: ObjectId`, `lat: f64`, `lng: f64`, `accuracy_meters: Option<f64>`, `occurred_at_client: DateTime`, `occurred_at_server: DateTime`. Derive `Debug, Clone, Serialize, Deserialize`.
- [x] 1.2 Add `Org::checkin_location_tracking_enabled() -> bool` helper on `domain::Org`, mirroring `checkin_transfer_enabled()` — reads `settings.checkin.location_tracking_enabled`, defaults to `false` on missing field.
- [x] 1.3 Add `rust_xlsxwriter` to `api/Cargo.toml` (latest stable, no native deps).
- [x] 1.4 `cargo check` clean after deps update.

## 2. Database layer

- [x] 2.1 Create `api/src/db/location_pings.rs` with `LocationPingRepository` wrapping a `Collection<LocationPing>`. Implement methods:
  - `insert_many_unordered(pings: &[LocationPing]) -> ApiResult<InsertManyOutcome>` where `InsertManyOutcome` carries `inserted_indices: Vec<usize>` and `failed_indices: Vec<(usize, String /* error code */)>`. Use `insert_many` with `InsertManyOptions::builder().ordered(false).build()`. On `BulkWriteError` parse `write_errors` and map indices.
  - `list_by_app_user_paginated(app_user_id, before: Option<DateTime>, limit: i64) -> ApiResult<Vec<LocationPing>>` — newest-first by `occurred_at_client`.
  - `list_for_export(app_user_id, from: DateTime, to: DateTime) -> ApiResult<Vec<LocationPing>>` — ascending by `occurred_at_client`.
- [x] 2.2 Wire the new repo into `Db` struct (`api/src/db/mod.rs`) — add field `location_pings`, populate in `Db::new`.
- [x] 2.3 In `Db::new`, create three indexes on `location_pings`:
  - Functional `(app_user_id: 1, occurred_at_client: -1)` for cursor pagination.
  - Functional `(org_id: 1, occurred_at_client: -1)` for export queries.
  - TTL on `(occurred_at_server: 1)` with `expire_after = Duration::from_secs(90 * 24 * 3600)`.
- [x] 2.4 Add a constant `pub const LOCATION_PING_BATCH_MAX: usize = 100` somewhere in the `db` or `domain` module (referenced by handler and tests).

## 3. DTO additions

- [x] 3.1 In `handlers/checkin_dto.rs`, extend `OrgCheckinSettingsDto` with `location_tracking_enabled: bool` field (serde).
- [x] 3.2 In `handlers/checkin_dto.rs`, extend `UpdateOrgSettingsRequest` with `location_tracking_enabled: Option<bool>` (serde default).
- [x] 3.3 In `handlers/checkin_dto.rs`, update `OrgSettingsDto::from_org` to populate the new field via `org.checkin_location_tracking_enabled()`.
- [x] 3.4 In `handlers/auth.rs`'s `OrgCheckinDto` and `handlers/app_dto.rs`'s `OrgCheckinDto`, add `location_tracking_enabled: bool` and populate in the corresponding `from_org` paths.
- [x] 3.5 New module `handlers/location_tracking_dto.rs` (or fold into `checkin_dto.rs` if it stays under ~250 lines; designer call). DTOs:
  - `SubmitLocationPingsRequest { pings: Vec<LocationPingInput> }`
  - `LocationPingInput { lat: f64, lng: f64, accuracy: Option<f64>, occurred_at_client: String }`
  - `SubmitLocationPingsResponse { accepted_count: u32, rejected: Vec<RejectedPingDto> }`
  - `RejectedPingDto { index: usize, code: String, message: String }`
  - `LocationPingDto { id: String, app_user_id: String, lat: f64, lng: f64, accuracy_meters: Option<f64>, occurred_at_client: String, occurred_at_server: String }` — admin list response item; also Output struct from `LocationPing::to_dto()` or similar.

## 4. Error variants

- [x] 4.1 In `api/src/error.rs`, add `ApiError` variants: `LocationTrackingDisabled`, `InvalidRange`, `InvalidBatch`. Update `ApiError::status_and_code()` mapping:
  - `LocationTrackingDisabled` → `(StatusCode::FORBIDDEN, "LOCATION_TRACKING_DISABLED")`
  - `InvalidRange` → `(StatusCode::BAD_REQUEST, "INVALID_RANGE")`
  - `InvalidBatch` → `(StatusCode::BAD_REQUEST, "INVALID_BATCH")`
- [x] 4.2 Note: `INVALID_PING_TIMESTAMP` and `INVALID_PING_COORDINATES` are NOT `ApiError` variants — they only appear inside `RejectedPingDto.code`. No global mapping needed.

## 5. State-lock unification

- [x] 5.1 In `handlers/checkin.rs::update_settings`, change the state-lock guard so it triggers when EITHER `transfer_enabled` or `location_tracking_enabled` is `Some(_)` in the request — not just `transfer_enabled`. Comment block above the guard explaining the unification (matches design.md "State-lock unification" decision).
- [x] 5.2 Pass `req.location_tracking_enabled` through to `db.orgs.update_settings` (the repo method needs a third optional parameter).
- [x] 5.3 In `db/orgs.rs::update_settings`, accept `location_tracking_enabled: Option<bool>` and write it under `settings.checkin.location_tracking_enabled` when `Some`.

## 6. POST /app/checkin/locations handler

- [x] 6.1 Create new module `handlers/location_tracking.rs`. Wire it into `handlers/mod.rs`.
- [x] 6.2 Implement `submit_location_pings(State, RequireAppUser, Json<SubmitLocationPingsRequest>) -> ApiResult<Json<SubmitLocationPingsResponse>>`. Flow:
  1. Read `Org` from db via `ctx.org_id`. If `!org.checkin_location_tracking_enabled()` → `Err(ApiError::LocationTrackingDisabled)`.
  2. Validate batch size: `pings.is_empty() || pings.len() > LOCATION_PING_BATCH_MAX` → `Err(ApiError::InvalidBatch)`.
  3. Iterate pings with index. For each, run pure-Rust validation:
     - `lat ∈ [-90, 90]` AND `lng ∈ [-180, 180]` AND (`accuracy` is None OR `accuracy >= 0`) → coordinates OK; else collect into rejected with `code = "INVALID_PING_COORDINATES"`.
     - Parse `occurred_at_client` as RFC3339; on parse error → `INVALID_PING_TIMESTAMP`.
     - Parsed value must be `≤ now()` and `> now() - 30 days`; else `INVALID_PING_TIMESTAMP`.
  4. Build `Vec<LocationPing>` for valid pings, with `occurred_at_server = DateTime::now()`.
  5. Call `repo.insert_many_unordered(&valid)`. Map `InsertManyOutcome` back: `failed_indices` (sub-array indices) → original batch indices via the valid-index list.
  6. Build response: `accepted_count = inserted_indices.len()`, `rejected = combined pre-validation rejections + post-insert failures`.
  7. Return `Json(response)` with implicit `201`.
- [x] 6.3 Register route `POST /app/checkin/locations` in the app router (next to existing `/app/checkin/events`). Apply `RequireAppUser` extractor.

## 7. GET /checkin/users/:id/locations handler

- [x] 7.1 Implement `list_locations(State, RequireAdmin, Path<String /* app_user_id_hex */>, Query<PaginationParams>) -> ApiResult<Json<Vec<LocationPingDto>>>`.
- [x] 7.2 Resolve the path's `app_user_id`: parse hex, look up in `db.app_users`, verify `app_user.org_id == active.org_id` else `404`.
- [x] 7.3 Parse query: `before` optional RFC3339 (parse via existing `parse_rfc3339`), `limit` optional with default 200 max 1000.
- [x] 7.4 Call `repo.list_by_app_user_paginated(app_user_id, before, limit)` and map each `LocationPing` to `LocationPingDto`.
- [x] 7.5 Register route `GET /checkin/users/:id/locations` in the admin router.

## 8. GET /checkin/users/:id/locations/export handler

- [x] 8.1 Implement `export_locations(State, RequireAdmin, Path<String>, Query<ExportRangeParams>) -> ApiResult<Response>`.
- [x] 8.2 Range validation: `from`, `to` both required; `to >= from`; `to - from ≤ 90 days`; `from >= now - 90 days`. Failures → `Err(ApiError::InvalidRange)`.
- [x] 8.3 Resolve the AppUser (same cross-org check as task 7.2).
- [x] 8.4 Fetch pings via `repo.list_for_export(app_user_id, from, to)`.
- [x] 8.5 Build xlsx via `rust_xlsxwriter::Workbook`:
  - One sheet named `軌跡` (or English `Locations`).
  - Header row: `occurred_at_client`, `occurred_at_server`, `lat`, `lng`, `accuracy_meters`. Bold + freeze row 1.
  - Format `occurred_at_client` using the AppUser's Org's `timezone` (load Org via existing helper, format via `chrono-tz` if not already pulled — fall back to UTC if not feasible without new dep).
  - Format `occurred_at_server` as RFC3339 UTC string.
  - Numeric columns for `lat` / `lng` / `accuracy_meters`; empty cell for missing accuracy.
  - Column widths: `occurred_at_*` ~22, lat/lng ~12, accuracy ~10.
- [x] 8.6 Save to in-memory `Vec<u8>` via `Workbook::save_to_buffer`. Build `Response` with `Content-Type: application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`, `Content-Disposition: attachment; filename="argus-locations-{username}-{from-date}-{to-date}.xlsx"`, body bytes.
- [x] 8.7 Register route `GET /checkin/users/:id/locations/export` in the admin router.

## 9. Integration tests

- [x] 9.1 `api/tests/location_tracking_settings.rs`: enabling `location_tracking_enabled` succeeds when no one on duty; fails with `STATE_LOCKED` when someone on duty; both toggles together fail when someone on duty; `OrgCheckinSettingsDto` round-trip carries the new field.
- [x] 9.2 `api/tests/location_tracking_submit.rs`: happy path 5 valid pings → `accepted_count = 5, rejected = []`; toggle off → `403 LOCATION_TRACKING_DISABLED`; empty batch → `400 INVALID_BATCH`; oversized batch (101) → `400 INVALID_BATCH`; one out-of-range lat → `accepted_count = N-1`, rejected has correct index + code; one >30-day-old timestamp → rejected; one future timestamp → rejected; bad RFC3339 → rejected; mixed valid/invalid in same batch → correct partial accept; body-supplied `app_user_id` ignored → persisted attribution from token.
- [x] 9.3 `api/tests/location_tracking_list.rs`: pagination works (descending by `occurred_at_client`); `before` cursor filters; cross-org AppUser id → `404`; non-admin member → `403`.
- [x] 9.4 `api/tests/location_tracking_export.rs`: valid range produces xlsx with correct row count and header; missing `from` → `INVALID_RANGE`; range > 90 days → `INVALID_RANGE`; `from` older than 90 days → `INVALID_RANGE`; cross-org AppUser → `404`. Parse the xlsx body via `calamine` or similar to verify columns programmatically (or alternative: parse the zip + xml directly if `calamine` is heavy).
- [x] 9.5 `api/tests/location_tracking_ttl.rs`: insert a ping with `occurred_at_server` set to a moment > 90 days ago, then query the index metadata to confirm the TTL config (don't wait for the actual TTL monitor sweep — too slow for a unit test). Optional: a longer-running test that asserts the index `expireAfterSeconds` field is exactly `7776000` (90 × 24 × 3600).

## 10. Documentation

- [x] 10.1 Update `api/README.md`'s "打卡 / Checkin" section, adding a "位置軌跡 / Location tracking" subsection with: the toggle name + default + state-lock note, the three endpoint signatures (POST batch / GET list / GET export), the partial-accept response shape, the 90-day TTL note, the per-ping validation rules (>30d / future), and a one-line forward reference to `add-location-tracking-app` for the client side.

## 11. Smoke

- [x] 11.1 `cargo build --release` clean.
- [x] 11.2 `cargo test` all green (TTL test may be marked `#[ignore]` if it exercises the 60-second monitor sweep — short variants run by default).
- [x] 11.3 Manual curl smoke:
  - PATCH `/orgs/me/settings` to enable toggle (admin cookie). Verify `OrgCheckinSettingsDto` carries `location_tracking_enabled: true`.
  - POST `/app/checkin/locations` (AppUser bearer) with 3 valid pings + 1 with `lat = 100`. Verify response body shape.
  - GET `/checkin/users/<id>/locations` (admin cookie) returns 3 entries newest-first.
  - GET `/checkin/users/<id>/locations/export?from=&to=` saves a valid xlsx file (open in Numbers/Excel, verify columns).
  - PATCH `/orgs/me/settings` to disable toggle, then POST a ping → 403.
