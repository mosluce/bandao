## MODIFIED Requirements

### Requirement: Location pings are persisted with dual timestamps

The system SHALL persist every accepted location ping in a `location_pings`
MongoDB collection. Each document SHALL carry `org_id`, `app_user_id`,
`lat: f64`, `lng: f64`, `accuracy_meters: Option<f64>`,
`occurred_at_client: DateTime`, and `occurred_at_server: DateTime`.
`occurred_at_client` SHALL come from the request body (RFC3339, with the
client's wall-clock); `occurred_at_server` SHALL be set by the server at
the moment of insertion (UTC, monotonic on the server side).

The system SHALL NOT install a TTL index on `location_pings`. Location
pings SHALL be retained indefinitely until a future rotation/archival
mechanism (not part of this change) is introduced. This is a deliberate,
temporary state: MongoDB TTL indexes apply to an entire collection, so
retention could not be relaxed only for legacy-imported pings (see
`legacy-checkin-backfill`) without also removing the previous 90-day
expiry for all pings.

#### Scenario: Ping insert sets both timestamps

- **WHEN** the server accepts a ping with `occurred_at_client = 2026-05-04T08:00:00+08:00`
- **THEN** the persisted document has `occurred_at_client` equal to the parsed UTC equivalent of the supplied value
- **AND** `occurred_at_server` is the server's `DateTime::now()` at insert time
- **AND** the two timestamps are within seconds of each other for clients with correctly-set clocks

#### Scenario: Pings older than 90 days are not deleted

- **GIVEN** a `location_pings` document with `occurred_at_server` set to a moment more than 90 days in the past
- **WHEN** MongoDB's background processes run
- **THEN** the document remains in the collection (no TTL index removes it)

### Requirement: Admin lists pings for one AppUser via cursor pagination

The system SHALL provide `GET /checkin/users/:id/locations` accepting
dashboard cookie auth and admin role. The endpoint SHALL accept query
parameters `before` (optional, RFC3339 timestamp), `limit` (optional,
integer; default 200, max 1000), and the optional date-range pair
`from` / `to` (each RFC3339 timestamp). Results SHALL be returned
newest-first by `occurred_at_client`. When `before` is supplied, only
pings with `occurred_at_client < before` SHALL be included. When `from`
is supplied, only pings with `occurred_at_client >= from` SHALL be
included. When `to` is supplied, only pings with `occurred_at_client < to`
SHALL be included. Multiple filters compose with AND.

When either `from` or `to` is supplied the system SHALL validate the
range using the same rules as the export endpoint: parse failures or
`to < from` or span exceeding 90 days SHALL return `INVALID_RANGE` (HTTP
400). `from` being more than 90 days in the past is no longer a rejection
condition — `location_pings` no longer has a TTL, so legacy-imported pings
can be older than 90 days and must remain readable. Either side may be
omitted; absent sides skip their respective check.

The path's `:id` SHALL identify an AppUser whose Org matches the
caller's `current_org`; mismatches SHALL return `404`.

#### Scenario: First page returns newest pings

- **WHEN** an admin requests `GET /checkin/users/<X>/locations?limit=50` for an AppUser X with 200 pings on file
- **THEN** the response is `200` with the 50 newest pings ordered descending by `occurred_at_client`

#### Scenario: Cursor pagination via `before`

- **WHEN** the admin requests the next page using the oldest `occurred_at_client` from the prior response as `before`
- **THEN** the response excludes any ping at or after that timestamp
- **AND** returns the next 50 older pings

#### Scenario: Date range filter via `from` and `to`

- **WHEN** an admin requests `GET /checkin/users/<X>/locations?from=2026-03-01T00:00:00%2B08:00&to=2026-03-02T00:00:00%2B08:00`
- **THEN** the response includes only pings with `occurred_at_client >= from` AND `occurred_at_client < to`
- **AND** the response is ordered newest-first

#### Scenario: from older than 90 days is allowed when span fits

- **WHEN** an admin requests `GET /checkin/users/<X>/locations?from=<91+ days ago>&to=<a point within 90 days of from>`
- **THEN** the response is `200`, not rejected on the basis of `from` alone

#### Scenario: span exceeding 90 days rejected

- **WHEN** an admin requests `GET /checkin/users/<X>/locations?from=<T>&to=<T + 91 days>`
- **THEN** the response is `400 INVALID_RANGE`

#### Scenario: to before from rejected

- **WHEN** an admin requests `GET /checkin/users/<X>/locations?from=<T>&to=<T - 1 day>`
- **THEN** the response is `400 INVALID_RANGE`

#### Scenario: Single-sided range allowed

- **WHEN** an admin requests `GET /checkin/users/<X>/locations?to=<T>` without `from`
- **THEN** the response includes only pings with `occurred_at_client < to` and is `200`

#### Scenario: Cross-org AppUser id rejected

- **GIVEN** an AppUser Y belongs to a different Org than the caller's `current_org`
- **WHEN** the admin requests `GET /checkin/users/<Y>/locations`
- **THEN** the response is `404`

#### Scenario: Member without admin role rejected

- **WHEN** a `member` (non-admin) requests `GET /checkin/users/<X>/locations`
- **THEN** the response is `403`

### Requirement: Admin exports one AppUser's pings as xlsx

The system SHALL provide `GET /checkin/users/:id/locations/export`
accepting dashboard cookie auth and admin role. The endpoint SHALL accept
required query parameters `from` and `to` (both RFC3339 timestamps).
The response SHALL be `200` with `Content-Type:
application/vnd.openxmlformats-officedocument.spreadsheetml.sheet` and a
`Content-Disposition: attachment; filename=<…>.xlsx` header. The xlsx
file SHALL contain a single sheet with the columns `occurred_at_client`
(formatted in the Org's `timezone`), `occurred_at_server` (UTC ISO8601),
`lat`, `lng`, `accuracy_meters`, with a header row at row 1 and the data
rows ordered ascending by `occurred_at_client`. Range validation SHALL
enforce: both `from` and `to` MUST be present; `to >= from`; `to - from`
MUST NOT exceed 90 days. `from` being more than 90 days before server
time is no longer a rejection condition, for the same reason as the list
endpoint above. Range failures SHALL return `400 INVALID_RANGE`.

#### Scenario: Valid export within the 90-day window

- **WHEN** an admin requests `GET /checkin/users/<X>/locations/export?from=2026-04-04T00:00:00Z&to=2026-05-04T00:00:00Z` and AppUser X has 1500 pings in that range
- **THEN** the response is `200`
- **AND** `Content-Type` is the xlsx MIME type
- **AND** the xlsx body contains 1501 rows (1 header + 1500 data) ordered ascending by `occurred_at_client`

#### Scenario: Range exceeds 90 days

- **WHEN** an admin requests an export with `to - from = 91 days`
- **THEN** the response is `400` with code `INVALID_RANGE`

#### Scenario: `from` older than 90 days from now is allowed when span fits

- **WHEN** an admin requests an export with `from` set to 100 days before server time and `to` set to 95 days before server time (5-day span)
- **THEN** the response is `200`, not rejected on the basis of `from` alone

#### Scenario: Missing `from` or `to`

- **WHEN** an admin requests an export omitting either query parameter
- **THEN** the response is `400` with code `INVALID_RANGE`

#### Scenario: Cross-org AppUser id rejected

- **GIVEN** an AppUser Y belongs to a different Org than the caller's `current_org`
- **WHEN** the admin requests `GET /checkin/users/<Y>/locations/export?from=&to=`
- **THEN** the response is `404`

### Requirement: AppUser may list their own pings via GET /app/checkin/me/locations

The system SHALL provide `GET /app/checkin/me/locations` accepting Bearer auth. The endpoint SHALL resolve the caller's `app_user_id` from the bearer token (NOT from a path or query parameter) and return only that AppUser's own pings. The endpoint SHALL accept query parameters `before` (optional, RFC3339), `from` (optional, RFC3339), `to` (optional, RFC3339), and `limit` (optional, integer; default 200, max 1000). Range validation, ordering (newest-first by `occurred_at_client`), pagination semantics, and the `INVALID_RANGE` error code SHALL match the admin `GET /checkin/users/:id/locations` endpoint exactly — including that `from` being more than 90 days in the past is not, on its own, a rejection condition.

The endpoint SHALL NOT be gated by `Org.settings.checkin.location_tracking_enabled`. An AppUser SHALL be able to read pings already persisted under their `app_user_id` even after their Org has subsequently set the toggle to `false`. The toggle continues to gate ingest (`POST /app/checkin/locations`) only.

The response body SHALL be the same `LocationPingDto` shape as the admin list endpoint.

#### Scenario: AppUser identity comes from token

- **WHEN** AppUser X (resolved from token) calls `GET /app/checkin/me/locations?limit=50` and has 200 pings on file
- **THEN** the response is `200` with the 50 newest pings ordered descending by `occurred_at_client`
- **AND** every returned ping has `app_user_id = X`

#### Scenario: Date range filter via from and to

- **WHEN** AppUser X calls `GET /app/checkin/me/locations?from=2026-05-15T00:00:00%2B08:00&to=2026-05-16T00:00:00%2B08:00`
- **THEN** the response includes only pings with `occurred_at_client >= from` AND `occurred_at_client < to`
- **AND** the response is ordered newest-first

#### Scenario: span exceeding 90 days rejected

- **WHEN** AppUser X calls `GET /app/checkin/me/locations?from=<T>&to=<T + 91 days>`
- **THEN** the response is `400 INVALID_RANGE`

#### Scenario: from older than 90 days is allowed when span fits

- **WHEN** AppUser X calls `GET /app/checkin/me/locations?from=<91+ days ago>&to=<a point within 90 days of from>`
- **THEN** the response is `200`, not rejected on the basis of `from` alone

#### Scenario: Toggle off does not block self-read

- **GIVEN** AppUser X's Org currently has `location_tracking_enabled = false`
- **AND** AppUser X has pings persisted from when the toggle was previously `true`
- **WHEN** AppUser X calls `GET /app/checkin/me/locations` covering those pings
- **THEN** the response is `200` with the AppUser's own pings
- **AND** the response is NOT `403 LOCATION_TRACKING_DISABLED`

#### Scenario: AppUser cannot read another user's pings

- **GIVEN** AppUser Y exists with pings on file
- **WHEN** AppUser X (different `app_user_id`) calls `GET /app/checkin/me/locations`
- **THEN** no ping with `app_user_id = Y` appears in the response

#### Scenario: Unauthenticated request rejected

- **WHEN** an unauthenticated request hits `GET /app/checkin/me/locations`
- **THEN** the response is `401 Unauthorized`

## REMOVED Requirements

### Requirement: Location pings are persisted with dual timestamps and 90-day server-time TTL

**Reason**: Replaced by "Location pings are persisted with dual timestamps"
above, which drops the 90-day TTL. Retention is temporarily unbounded
pending a future rotation/archival mechanism; see `add-legacy-backfill-windows-and-pings`
proposal and design for rationale (legacy-imported path data years old
cannot coexist with a collection-wide 90-day TTL).

**Migration**: No data migration needed — removing a MongoDB TTL index
only stops future deletions; it does not need to be reconciled with
existing documents. Downstream consumers (admin-web privacy policy page,
App location-consent dialog) still state a 90-day retention promise; that
copy is intentionally left unchanged until the future rotation mechanism
is designed, and is a known, accepted temporary inconsistency.
