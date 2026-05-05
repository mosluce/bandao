## Why

The `add-app-checkin-polish` archive shipped a queue + GPS + workmanager
scaffolding sized to absorb a continuous location stream. The next ROADMAP
item is to fill in that capability — let workers' phones report periodic
position pings during work shifts so admins can see trajectories on a map
later (`add-location-tracking-app` + `add-location-tracking-dashboard`).

This change is the API-layer foundation: the data structures, the org
toggle, the batch ingest endpoint, the admin query endpoint, and the
xlsx export endpoint. Without this, neither client side has anywhere to
push or pull. Privacy policy disclosure (`add-org-privacy-policy`) is
also already shipped and references location tracking with 90-day
retention — landing the server side now keeps the policy text aligned
with reality before workers ever consent.

## What Changes

- **New `location_pings` collection** in MongoDB. Schema: `{ _id,
  org_id, app_user_id, lat, lng, accuracy_meters?, occurred_at_client,
  occurred_at_server }`. No reverse-geocoding (volume too high; map
  rendering uses raw coordinates).
- **TTL index** on `occurred_at_server` set to **expire after 90 days**.
  Mongo's TTL monitor sweeps approximately every 60s, so the actual
  retention is "90 days ± a minute". Using server time (UTC monotonic
  on our side) rather than client time avoids client-clock-skew edge
  cases where a forward-jumped device prematurely deletes its own data.
- **Two functional indexes**: `(app_user_id, occurred_at_client)` for
  cursor pagination + admin queries; `(org_id, occurred_at_client)`
  for the export query path.
- **`Org.settings.checkin.location_tracking_enabled: bool`** flag,
  defaulting to `false` on Org creation. Admin toggles via the existing
  `PATCH /orgs/me/settings` endpoint by adding the new optional field.
- **State-lock applies to the new toggle exactly as it does to
  `transfer_enabled`**: when any AppUser in the org is non-`off_duty`
  the flip is rejected with `409 STATE_LOCKED`. One unified lock; no
  separate code path.
- **`POST /app/checkin/locations`** (Bearer auth, AppUser):
  - Body: `{ pings: [{ lat, lng, accuracy?, occurred_at_client }, ...] }`
  - `app_user_id` resolved from the bearer token (NOT from the body).
  - Batch size 1–100; 0 or > 100 → `400 INVALID_BATCH`.
  - Response 201: `{ accepted_count, rejected: [{ index, code, message }, ...] }`
  - Per-ping validation: `lat ∈ [-90, 90]`, `lng ∈ [-180, 180]`,
    `accuracy_meters ≥ 0` if present, RFC3339 parseable
    `occurred_at_client`, **timestamp not in the future**, **timestamp
    not older than 30 days**. Failures land in `rejected[]` with
    `code = INVALID_PING_TIMESTAMP` or `INVALID_PING_COORDINATES`;
    valid pings still inserted via `insert_many(ordered: false)`.
  - Toggle off → entire batch rejected with
    `403 LOCATION_TRACKING_DISABLED`. (Single error response, not
    per-ping; the whole submission is meaningless without consent.)
  - AppUser must be `active` (not `disabled`); standard
    `RequireAppUser` extractor handles it.
- **`GET /checkin/users/:id/locations`** (Cookie auth, admin):
  - `?before=<RFC3339>&limit=<int, default 200, max 1000>` cursor
    pagination, newest-first by `occurred_at_client`.
  - 404 if the AppUser is not in the caller's `current_org`.
  - Response: array of `{ id, app_user_id, lat, lng, accuracy_meters?,
    occurred_at_client, occurred_at_server }`.
- **`GET /checkin/users/:id/locations/export`** (Cookie auth, admin):
  - `?from=&to=` both required (RFC3339 timestamps).
  - Range validation: `to >= from`, `to - from ≤ 90 days`,
    `from ≥ now - 90 days`. Failures → `400 INVALID_RANGE`.
  - Response: `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`
    body, `Content-Disposition: attachment; filename=...xlsx`.
  - Columns: `occurred_at_client (Org tz)`, `occurred_at_server (UTC)`,
    `lat`, `lng`, `accuracy_meters`. Header row, frozen first row,
    sensible column widths.
- **New error variants** in `ApiError`:
  - `LocationTrackingDisabled` → `403 LOCATION_TRACKING_DISABLED`
  - `InvalidRange` → `400 INVALID_RANGE`
  - `InvalidBatch` → `400 INVALID_BATCH`
  - `InvalidPingTimestamp` and `InvalidPingCoordinates` are per-ping
    rejection codes (returned in the response body's `rejected` array,
    not raised as `ApiError` since the surrounding request still 201s).
- **DTO updates**:
  - `OrgCheckinSettingsDto` gains `location_tracking_enabled: bool`.
  - `OrgSettingsDto` re-exports it via the existing `from_org` flow.
  - `UpdateOrgSettingsRequest` gains optional `location_tracking_enabled: Option<bool>`.
  - The `OrgCheckinDto` consumed by `/me` (dashboard) and `/app/me`
    (mobile) gains the same field, so both clients see the toggle in
    their cached `Org` snapshot.
- **`Org::checkin_location_tracking_enabled() -> bool`** helper on
  the domain `Org` struct, mirroring `checkin_transfer_enabled()`,
  default `false`.
- **`rust_xlsxwriter` dependency** (pure Rust, active maintenance, no
  native deps) added to `Cargo.toml`.
- **Documentation**: `api/README.md` "打卡 / Checkin" section gains a
  "位置軌跡 / Location tracking" subsection covering the toggle,
  endpoints, retention, and the contract for the upcoming
  `add-location-tracking-app` change.

Out of scope (deferred to ROADMAP / future changes):

- Admin-initiated bulk delete of pings (e.g. for §11 erasure requests).
  MVP: 90-day TTL handles most cases; admin uses `mongoexport` + manual
  delete for the rare explicit request. Future: `DELETE
  /checkin/users/:id/locations` if real demand emerges.
- Multi-AppUser export. Per-AppUser only for v1; bulk org-wide export
  needs a background-job design (memory + size constraints) and is its
  own change.
- Reverse-geocoding pings. Volume too high; admin map shows raw
  coordinates.
- CSV format. xlsx is the only supported export format for v1.
- Server-side validation that pings fall within an `on_duty` time
  window. The client gates submission via the home-screen status
  guard; server only does cheap range checks (future / >30d old).

## Capabilities

### New Capabilities

- `location-tracking`: covers the org-level toggle, the AppUser batch
  ingest endpoint, the admin query and xlsx export endpoints, the
  retention rule, the per-ping validation, and the partial-accept
  response shape.

### Modified Capabilities

- `checkin-events`: the existing "Transfer-enabled toggle is
  state-locked" requirement extends to also lock
  `location_tracking_enabled` flips. One unified state-lock check; both
  toggles routed through the same `PATCH /orgs/me/settings` handler.

## Impact

- **Code**: new `domain::LocationPing` struct,
  `db::LocationPingRepository`, three new handler functions in
  `handlers::location_tracking` (or fold into `handlers::checkin`,
  TBD in design.md), DTO additions, error variants, settings handler
  patch, db setup additions in `db/mod.rs` (collection + 3 indexes).
- **Rust deps**: `rust_xlsxwriter` (no native, no Office runtime).
- **DB**: new collection + 3 indexes (TTL + 2 functional). Existing
  collections untouched.
- **API surface**: 3 new endpoints + 1 modified endpoint
  (`PATCH /orgs/me/settings` accepts a new optional field).
- **No mobile / web-side changes in this change** — both clients
  get the new behavior in their respective follow-up changes
  (`add-location-tracking-app` + `add-location-tracking-dashboard`).
- **Tests**: ~10–12 integration tests in `api/tests/` (batch happy
  path, partial reject scenarios, toggle-off rejection, state-lock
  on toggle, pagination, xlsx export structure, range validation,
  TTL behavior, auth checks).
- **DB size**: very rough — 100 workers × 60s sample × 8 hr × 22
  days × 3 months × ~30% distance-filter retention ≈ 1M rows over a
  90-day rolling window. ~80 bytes per row → ~80 MB on the
  collection. Indexes add ~30 MB. Acceptable for MVP.
