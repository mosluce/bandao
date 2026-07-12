## MODIFIED Requirements

### Requirement: Any Org member lists pings for one AppUser via cursor pagination

The system SHALL provide `GET /checkin/users/:id/locations` for any authenticated dashboard user with an active membership in `current_org` (`admin` or `member`). The endpoint SHALL accept query
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
caller's `current_org`; mismatches SHALL return `404`. Exporting pings as
xlsx (`GET /checkin/users/:id/locations/export`) remains `admin`-only —
this requirement only changes read access to the paginated JSON listing.

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

#### Scenario: Member can list pings, identically to admin

- **WHEN** a `member` requests `GET /checkin/users/<X>/locations` for an AppUser X in `current_org`
- **THEN** the response is `200 OK` with the same content a same-Org admin would receive

#### Scenario: Member cross-org AppUser id still rejected

- **GIVEN** an AppUser Y belongs to a different Org than the caller's `current_org`
- **WHEN** a `member` requests `GET /checkin/users/<Y>/locations`
- **THEN** the response is `404`
