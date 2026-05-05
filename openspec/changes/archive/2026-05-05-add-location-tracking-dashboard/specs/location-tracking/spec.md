## MODIFIED Requirements

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
range using the same rules as the export endpoint: parse failures,
`to < from`, span exceeding 90 days, or `from` older than 90 days from
the current server time SHALL all return `INVALID_RANGE` (HTTP 400).
Either side may be omitted; absent sides skip their respective check.

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

#### Scenario: from older than 90 days rejected

- **WHEN** an admin requests `GET /checkin/users/<X>/locations?from=<90+ days ago>&to=<recent>`
- **THEN** the response is `400 INVALID_RANGE`

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
