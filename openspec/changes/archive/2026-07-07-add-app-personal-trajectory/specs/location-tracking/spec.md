## ADDED Requirements

### Requirement: AppUser may list their own pings via GET /app/checkin/me/locations

The system SHALL provide `GET /app/checkin/me/locations` accepting Bearer auth. The endpoint SHALL resolve the caller's `app_user_id` from the bearer token (NOT from a path or query parameter) and return only that AppUser's own pings. The endpoint SHALL accept query parameters `before` (optional, RFC3339), `from` (optional, RFC3339), `to` (optional, RFC3339), and `limit` (optional, integer; default 200, max 1000). Range validation, ordering (newest-first by `occurred_at_client`), pagination semantics, and the `INVALID_RANGE` error code SHALL match the admin `GET /checkin/users/:id/locations` endpoint exactly.

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

#### Scenario: from older than 90 days rejected

- **WHEN** AppUser X calls `GET /app/checkin/me/locations?from=<100 days ago>&to=<recent>`
- **THEN** the response is `400 INVALID_RANGE`

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
