# location-tracking Specification

## Purpose
TBD - created by archiving change add-location-tracking-server. Update Purpose after archive.
## Requirements
### Requirement: Org-level location tracking toggle defaults to disabled

The system SHALL store `Org.settings.checkin.location_tracking_enabled: bool`
defaulting to `false` for all Orgs. A missing field SHALL read as `false`
(no behavioral difference from explicit `false`). Admins MAY update the
flag via `PATCH /orgs/me/settings` by including `location_tracking_enabled`
in the request body. The flag SHALL be exposed in the `OrgCheckinSettingsDto`
returned by `PATCH /orgs/me/settings`, in the `OrgCheckinDto` consumed by
`/me` (dashboard auth) and `/app/me` (mobile auth), so clients can read the
current value from their cached `Org` snapshot.

#### Scenario: New Org has location tracking off by default

- **WHEN** a new Org is created
- **THEN** `Org.settings.checkin.location_tracking_enabled` reads as `false`
- **AND** the `OrgCheckinDto` returned to clients reports `location_tracking_enabled = false`

#### Scenario: Admin enables location tracking

- **WHEN** an admin sends `PATCH /orgs/me/settings` with body
  `{ "location_tracking_enabled": true }`
- **AND** all AppUsers in the Org have `checkin_user_status.status == off_duty`
- **THEN** the response is `200`
- **AND** subsequent reads of `Org.settings.checkin.location_tracking_enabled` return `true`

#### Scenario: Existing Org with no field reads as disabled

- **WHEN** an Org row in MongoDB has no
  `settings.checkin.location_tracking_enabled` field at all
- **THEN** the API treats it as `false`
- **AND** any AppUser-side ping submission for that Org is rejected with
  `403 LOCATION_TRACKING_DISABLED`

### Requirement: Location pings are persisted with dual timestamps and 90-day server-time TTL

The system SHALL persist every accepted location ping in a `location_pings`
MongoDB collection. Each document SHALL carry `org_id`, `app_user_id`,
`lat: f64`, `lng: f64`, `accuracy_meters: Option<f64>`,
`occurred_at_client: DateTime`, and `occurred_at_server: DateTime`.
`occurred_at_client` SHALL come from the request body (RFC3339, with the
client's wall-clock); `occurred_at_server` SHALL be set by the server at
the moment of insertion (UTC, monotonic on the server side).

A TTL index SHALL be installed on `occurred_at_server` configured to expire
documents 90 days after that field's value. The system SHALL NOT install a
TTL index on `occurred_at_client` (client-clock skew would cause incorrect
deletion timing).

#### Scenario: Ping insert sets both timestamps

- **WHEN** the server accepts a ping with `occurred_at_client = 2026-05-04T08:00:00+08:00`
- **THEN** the persisted document has `occurred_at_client` equal to the parsed UTC equivalent of the supplied value
- **AND** `occurred_at_server` is the server's `DateTime::now()` at insert time
- **AND** the two timestamps are within seconds of each other for clients with correctly-set clocks

#### Scenario: TTL deletes documents 90 days after server time

- **GIVEN** a `location_pings` document with `occurred_at_server` set to a moment more than 90 days in the past
- **WHEN** MongoDB's TTL monitor next runs (approximately every 60 seconds)
- **THEN** the document is removed from the collection

### Requirement: AppUser may submit a batch of 1–100 pings via POST /app/checkin/locations

The system SHALL provide `POST /app/checkin/locations` accepting Bearer
auth. The request body SHALL be `{ "pings": [Ping, ...] }` where each
`Ping` has `lat: f64`, `lng: f64`, optional `accuracy: f64`, and
`occurred_at_client: String` (RFC3339). The system SHALL resolve the
caller's `app_user_id` from the bearer token (NOT from the request body)
and attribute every persisted ping to that user. The batch SHALL contain
between 1 and 100 pings inclusive; otherwise the system SHALL reject the
entire request with `400 INVALID_BATCH`.

#### Scenario: Empty batch rejected

- **WHEN** an authenticated AppUser sends `POST /app/checkin/locations` with body `{ "pings": [] }`
- **THEN** the response is `400` with code `INVALID_BATCH`

#### Scenario: Oversized batch rejected

- **WHEN** an authenticated AppUser sends a batch of 101 pings
- **THEN** the response is `400` with code `INVALID_BATCH`

#### Scenario: AppUser identity comes from token, not body

- **WHEN** AppUser X (resolved from token) submits a batch where the body contains no `app_user_id` field at all
- **THEN** every persisted document carries `app_user_id = X`

#### Scenario: Body-supplied `app_user_id` is ignored

- **WHEN** AppUser X submits a batch and (incorrectly) includes `app_user_id = Y` in the body
- **THEN** the body field is ignored
- **AND** persisted documents carry `app_user_id = X` (from the token)

### Requirement: Batch submission rejects when Org toggle is off

The system SHALL, before any per-ping validation, check the caller's
`Org.settings.checkin.location_tracking_enabled` flag. When the flag is
`false`, the system SHALL reject the entire request with
`403 LOCATION_TRACKING_DISABLED`, regardless of how many pings the body
contains or whether they would otherwise validate.

#### Scenario: Toggle off blocks the entire batch

- **GIVEN** Org X has `location_tracking_enabled = false`
- **WHEN** an AppUser of Org X sends a valid `POST /app/checkin/locations` with 5 pings
- **THEN** the response is `403` with code `LOCATION_TRACKING_DISABLED`
- **AND** no document is inserted into `location_pings`

### Requirement: Per-ping validation produces a partial-accept response

After the toggle and batch-size checks pass, the system SHALL validate
each ping independently and SHALL return `201 Created` with a body of
shape `{ "accepted_count": <int>, "rejected": [{ "index": <int>,
"code": <string>, "message": <string> }, ...] }`. Per-ping validation
rules:

- `lat` MUST be in `[-90.0, 90.0]`; otherwise `INVALID_PING_COORDINATES`.
- `lng` MUST be in `[-180.0, 180.0]`; otherwise `INVALID_PING_COORDINATES`.
- `accuracy_meters`, if present, MUST be `>= 0`; otherwise `INVALID_PING_COORDINATES`.
- `occurred_at_client` MUST be parseable as RFC3339; otherwise `INVALID_PING_TIMESTAMP`.
- `occurred_at_client` MUST NOT be in the future relative to server time at
  request handling; otherwise `INVALID_PING_TIMESTAMP`.
- `occurred_at_client` MUST NOT be more than 30 days older than server time;
  otherwise `INVALID_PING_TIMESTAMP`.

Pings failing validation SHALL appear in `rejected[]` with their original
batch index. Pings passing validation SHALL be inserted via
`insert_many(ordered: false)` so a single bad row cannot abort the whole
write. The response status SHALL be `201` even when `accepted_count == 0`
(the request was processed; per-index feedback is the channel for
failures).

#### Scenario: All pings valid

- **WHEN** an AppUser submits a batch of 3 valid pings
- **THEN** the response is `201`
- **AND** `accepted_count = 3`
- **AND** `rejected = []`
- **AND** 3 documents are inserted into `location_pings`

#### Scenario: One ping has out-of-range latitude

- **WHEN** a batch of 3 pings has `lat = 91.0` at index 1; the others valid
- **THEN** the response is `201`
- **AND** `accepted_count = 2`
- **AND** `rejected` contains one entry with `index = 1, code = "INVALID_PING_COORDINATES"`

#### Scenario: One ping is in the future

- **WHEN** a batch of 5 pings has the index-3 entry's `occurred_at_client` set 10 minutes after the server's current time
- **THEN** `rejected` contains `{ index: 3, code: "INVALID_PING_TIMESTAMP" }`
- **AND** the other 4 are accepted

#### Scenario: One ping is older than 30 days

- **WHEN** a batch contains a ping with `occurred_at_client` set 31 days before server time
- **THEN** that ping appears in `rejected` with `code = "INVALID_PING_TIMESTAMP"`

#### Scenario: All pings rejected still returns 201

- **WHEN** every ping in a batch fails validation
- **THEN** the response status is `201`
- **AND** `accepted_count = 0`
- **AND** `rejected` lists every original index

### Requirement: Admin lists pings for one AppUser via cursor pagination

The system SHALL provide `GET /checkin/users/:id/locations` accepting
dashboard cookie auth and admin role. The endpoint SHALL accept query
parameters `before` (optional, RFC3339 timestamp) and `limit` (optional,
integer; default 200, max 1000). Results SHALL be returned newest-first
by `occurred_at_client`. When `before` is supplied, only pings with
`occurred_at_client < before` SHALL be included. The path's `:id` SHALL
identify an AppUser whose Org matches the caller's `current_org`;
mismatches SHALL return `404`.

#### Scenario: First page returns newest pings

- **WHEN** an admin requests `GET /checkin/users/<X>/locations?limit=50` for an AppUser X with 200 pings on file
- **THEN** the response is `200` with the 50 newest pings ordered descending by `occurred_at_client`

#### Scenario: Cursor pagination via `before`

- **WHEN** the admin requests the next page using the oldest `occurred_at_client` from the prior response as `before`
- **THEN** the response excludes any ping at or after that timestamp
- **AND** returns the next 50 older pings

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
MUST NOT exceed 90 days; `from` MUST NOT be older than 90 days from
server time. Range failures SHALL return `400 INVALID_RANGE`.

#### Scenario: Valid export within the 90-day window

- **WHEN** an admin requests `GET /checkin/users/<X>/locations/export?from=2026-04-04T00:00:00Z&to=2026-05-04T00:00:00Z` and AppUser X has 1500 pings in that range
- **THEN** the response is `200`
- **AND** `Content-Type` is the xlsx MIME type
- **AND** the xlsx body contains 1501 rows (1 header + 1500 data) ordered ascending by `occurred_at_client`

#### Scenario: Range exceeds 90 days

- **WHEN** an admin requests an export with `to - from = 91 days`
- **THEN** the response is `400` with code `INVALID_RANGE`

#### Scenario: `from` older than 90 days from now

- **WHEN** an admin requests an export with `from` set to 100 days before server time
- **THEN** the response is `400` with code `INVALID_RANGE`

#### Scenario: Missing `from` or `to`

- **WHEN** an admin requests an export omitting either query parameter
- **THEN** the response is `400` with code `INVALID_RANGE`

#### Scenario: Cross-org AppUser id rejected

- **GIVEN** an AppUser Y belongs to a different Org than the caller's `current_org`
- **WHEN** the admin requests `GET /checkin/users/<Y>/locations/export?from=&to=`
- **THEN** the response is `404`

### Requirement: Pings carry no reverse-geocoded region name

The system SHALL NOT call any reverse-geocoding service (Nominatim or
otherwise) on submitted pings. Persisted ping documents SHALL NOT
contain a `region_name` field. The admin trajectory map and the xlsx
export SHALL render only raw `lat` / `lng` coordinates plus optional
`accuracy_meters`.

#### Scenario: No region_name on persisted ping

- **WHEN** a ping is persisted via `POST /app/checkin/locations`
- **THEN** the resulting document has no `region_name` field

#### Scenario: No reverse-geocoding round-trip

- **WHEN** a batch of 100 pings is submitted
- **THEN** the server makes zero outbound calls to any reverse-geocoding service
- **AND** the response time is bounded by the database insert, not by network calls
