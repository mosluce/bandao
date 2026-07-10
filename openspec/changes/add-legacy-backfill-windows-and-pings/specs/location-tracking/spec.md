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
