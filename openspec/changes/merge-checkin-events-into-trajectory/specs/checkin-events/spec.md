## MODIFIED Requirements

### Requirement: AppUser fetches own status and history

The system SHALL provide `GET /app/checkin/status` returning `{ status, current_shift_started_at, last_event }` and `GET /app/checkin/events` returning a cursor-paginated list of the AppUser's own events (newest first by `occurred_at_client`, default page size 50). Both endpoints SHALL scope strictly to the caller; AppUsers SHALL NOT see another AppUser's events through this surface.

`GET /app/checkin/events` SHALL additionally accept the optional date-range pair `from` / `to` (each RFC3339). When `from` is supplied, only events with `occurred_at_client >= from` SHALL be included. When `to` is supplied, only events with `occurred_at_client < to` SHALL be included. Either side MAY be omitted; absent sides skip their respective check. The range filters compose with the existing `before` cursor and `limit` using AND. Omitting both `from` and `to` SHALL preserve the endpoint's prior behaviour exactly.

When either `from` or `to` is supplied the system SHALL validate the range using the same rules as `GET /app/checkin/me/locations`: parse failures, `to < from`, or a span exceeding 90 days SHALL return `INVALID_RANGE` (HTTP 400). `from` being more than 90 days in the past SHALL NOT be a rejection condition on its own — `legacy_backfill`-imported events can be arbitrarily old and must remain readable.

#### Scenario: AppUser fetches their own status

- **WHEN** an authenticated AppUser sends `GET /app/checkin/status`
- **THEN** the response is the caller's `checkin_user_status` row plus the resolved `last_event` document if `last_event_id` is non-null

#### Scenario: AppUser lists own events

- **WHEN** an authenticated AppUser sends `GET /app/checkin/events`
- **THEN** the response contains up to 50 of the caller's events ordered by `occurred_at_client` descending
- **AND** events belonging to other AppUsers are not included

#### Scenario: Date range filter via from and to

- **WHEN** an authenticated AppUser sends `GET /app/checkin/events?from=2026-05-15T00:00:00%2B08:00&to=2026-05-16T00:00:00%2B08:00`
- **THEN** the response includes only the caller's events with `occurred_at_client >= from` AND `occurred_at_client < to`
- **AND** the response is ordered newest-first

#### Scenario: Range far in the past is allowed when the span fits

- **WHEN** an authenticated AppUser sends `GET /app/checkin/events?from=<200 days ago>&to=<199 days ago>`
- **THEN** the response is `200` with that day's events, not rejected on the basis of `from` alone

#### Scenario: Span exceeding 90 days rejected

- **WHEN** an authenticated AppUser sends `GET /app/checkin/events?from=<T>&to=<T + 91 days>`
- **THEN** the response is `400 INVALID_RANGE`

#### Scenario: to before from rejected

- **WHEN** an authenticated AppUser sends `GET /app/checkin/events?from=<T>&to=<T - 1 day>`
- **THEN** the response is `400 INVALID_RANGE`

#### Scenario: Omitting the range preserves prior behaviour

- **WHEN** an authenticated AppUser sends `GET /app/checkin/events` with neither `from` nor `to`
- **THEN** the response is the same cursor-paginated newest-first page the endpoint returned before the range parameters existed

### Requirement: Any Org member can view one AppUser's event history

The system SHALL provide `GET /checkin/users/:id/events` for any authenticated dashboard user with an active membership in `current_org` (`admin` or `member`), returning the target AppUser's events (cursor-paginated, newest first by `occurred_at_client`, default page size 50). The endpoint SHALL be scoped to `current_org`; targeting an AppUser belonging to another Org SHALL return `NOT_FOUND`.

The endpoint SHALL additionally accept the optional date-range pair `from` / `to` (each RFC3339), with semantics, composition, and validation identical to `GET /app/checkin/events`: `occurred_at_client >= from`, `occurred_at_client < to`, either side omittable, composing with `before` and `limit` via AND, and `INVALID_RANGE` (HTTP 400) on parse failure, `to < from`, or a span exceeding 90 days. `from` being more than 90 days in the past SHALL NOT be a rejection condition on its own. Omitting both SHALL preserve the endpoint's prior behaviour exactly.

Range validation SHALL be shared with the location-ping endpoints rather than reimplemented, so the two surfaces cannot drift.

#### Scenario: Admin views in-org AppUser events

- **WHEN** an authenticated admin sends `GET /checkin/users/:id/events` for an AppUser in `current_org`
- **THEN** the response contains the target's events with `event_type`, `occurred_at_client`, `occurred_at_server`, location, `source`, `initiated_by_kind`, and `has_skew_warning` per event

#### Scenario: Cross-Org target rejected

- **WHEN** an admin sends `GET /checkin/users/:id/events` for an AppUser whose `org_id != current_org_id`
- **THEN** the response is `NOT_FOUND`

#### Scenario: Member can view AppUser event history, identically to admin

- **WHEN** a `member` sends `GET /checkin/users/:id/events` for an AppUser in `current_org`
- **THEN** the response is `200 OK` with the same content a same-Org admin would receive

#### Scenario: Date range filter via from and to

- **WHEN** an admin sends `GET /checkin/users/<X>/events?from=2026-03-01T00:00:00%2B08:00&to=2026-03-02T00:00:00%2B08:00`
- **THEN** the response includes only events with `occurred_at_client >= from` AND `occurred_at_client < to`
- **AND** the response is ordered newest-first

#### Scenario: Legacy-imported day beyond the default page reach is retrievable

- **GIVEN** an AppUser with `legacy_backfill` events 200 days old and more than 100 newer events
- **WHEN** an admin sends `GET /checkin/users/<X>/events?from=<that day>&to=<next day>`
- **THEN** the response contains that day's events
- **AND** the newer events do not crowd them out of the page

#### Scenario: Span exceeding 90 days rejected

- **WHEN** an admin sends `GET /checkin/users/<X>/events?from=<T>&to=<T + 91 days>`
- **THEN** the response is `400 INVALID_RANGE`

#### Scenario: to before from rejected

- **WHEN** an admin sends `GET /checkin/users/<X>/events?from=<T>&to=<T - 1 day>`
- **THEN** the response is `400 INVALID_RANGE`

#### Scenario: Cross-Org target rejected with a range supplied

- **WHEN** an admin sends `GET /checkin/users/<Y>/events?from=&to=` for an AppUser in another Org
- **THEN** the response is `NOT_FOUND` (Org scoping is checked before the range)

Force-checkout (`POST /checkin/users/:id/force-checkout`) and the Org checkin settings update (`PATCH /orgs/me/settings`) are unaffected by this change and remain `admin`-only — see the unmodified "Admin can force checkout an AppUser on shift" requirement.
