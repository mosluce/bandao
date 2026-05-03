## ADDED Requirements

### Requirement: AppUser checkin status is governed by a three-state machine

The system SHALL maintain a `checkin_user_status` row per AppUser containing `status: off_duty | on_site | in_transit`, `current_shift_started_at: DateTime | null`, and `last_event_id: ObjectId | null`. Newly created AppUsers SHALL start with `status = off_duty`, `current_shift_started_at = null`, `last_event_id = null`. Each successful event SHALL atomically update both the `checkin_events` row (insert) and the `checkin_user_status` row (update). The legal transitions are exactly:

| From | Event | To |
|---|---|---|
| `off_duty` | `clock_in` | `on_site` |
| `on_site` | `clock_out` | `off_duty` |
| `on_site` | `transfer_out` | `in_transit` |
| `in_transit` | `transfer_in` | `on_site` |
| `in_transit` | `clock_out` | `off_duty` |

Any other (status, event) pair SHALL be rejected with `422 INVALID_TRANSITION`. The error body SHALL include the AppUser's current `status` and the attempted `event_type`.

#### Scenario: New AppUser starts off duty

- **WHEN** an admin creates a new AppUser
- **THEN** a `checkin_user_status` row is inserted with `app_user_id`, `org_id`, `status = off_duty`, `current_shift_started_at = null`, `last_event_id = null`

#### Scenario: Successful clock_in transitions off_duty to on_site

- **WHEN** an AppUser with `status = off_duty` submits a valid `clock_in` event
- **THEN** the event is recorded with `event_type = clock_in`
- **AND** the AppUser's status row is updated to `status = on_site`, `current_shift_started_at = event.occurred_at_client`, `last_event_id = new_event.id`

#### Scenario: Successful transfer_out from on_site

- **WHEN** an AppUser with `status = on_site` submits a `transfer_out` event
- **THEN** the AppUser's status becomes `in_transit`
- **AND** `current_shift_started_at` is unchanged (the shift continues across transfers)

#### Scenario: Successful transfer_in from in_transit

- **WHEN** an AppUser with `status = in_transit` submits a `transfer_in` event
- **THEN** the AppUser's status returns to `on_site`
- **AND** the event represents arrival at the next worksite (location captured per-event); the system does NOT compare the location against the original `clock_in` location

#### Scenario: Successful clock_out from in_transit

- **WHEN** an AppUser with `status = in_transit` submits a `clock_out` event
- **THEN** the AppUser's status becomes `off_duty`
- **AND** `current_shift_started_at` is reset to `null`

#### Scenario: Multi-site shift cycle is supported

- **WHEN** an AppUser performs `clock_in → transfer_out → transfer_in → transfer_out → transfer_in → clock_out` in chronological order
- **THEN** every transition is accepted (the AppUser visited 3 worksites in one shift)
- **AND** the final status is `off_duty`

#### Scenario: clock_in while already on shift rejected

- **WHEN** an AppUser with `status = on_site` or `status = in_transit` submits `clock_in`
- **THEN** the request is rejected with `INVALID_TRANSITION`
- **AND** no event is recorded and the status row is unchanged

#### Scenario: clock_out while off duty rejected

- **WHEN** an AppUser with `status = off_duty` submits `clock_out`
- **THEN** the request is rejected with `INVALID_TRANSITION`

#### Scenario: transfer_in from on_site rejected

- **WHEN** an AppUser with `status = on_site` submits `transfer_in`
- **THEN** the request is rejected with `INVALID_TRANSITION`

#### Scenario: transfer_out from in_transit rejected

- **WHEN** an AppUser with `status = in_transit` submits `transfer_out`
- **THEN** the request is rejected with `INVALID_TRANSITION`

### Requirement: Each event records dual timestamps and a location

The system SHALL store every event with `occurred_at_client: DateTime` (supplied by the AppUser in the request body) and `occurred_at_server: DateTime` (set by the server on receipt). The system SHALL accept any value for `occurred_at_client` including future or past timestamps. The system SHALL NOT use `occurred_at_client` validation as a fraud filter beyond ordering rules. Display, sorting, and pagination on both `/app/checkin/*` and `/checkin/*` SHALL use `occurred_at_client` as the canonical time. Each event SHALL also store a `location` document containing `coordinates: { lat: f64, lng: f64 }` (required), `accuracy_meters: f64?` (optional), `region_name: String?` (server-set via reverse geocoding, may be `null`), and `manual_label: String?` (optional, from the request, 1–120 characters when present).

#### Scenario: Event records both client and server timestamps

- **WHEN** an AppUser submits any valid event with `occurred_at_client = T_c`
- **AND** the server receives it at wall time `T_s`
- **THEN** the stored event row has `occurred_at_client = T_c` and `occurred_at_server = T_s`

#### Scenario: Future client time accepted

- **WHEN** an AppUser submits an event with `occurred_at_client` set to one hour in the future
- **THEN** the request is accepted (subject to other validation)
- **AND** admin-web subsequently flags the event with a skew warning (see "Admin live status board" requirement)

#### Scenario: Old client time accepted (offline sync)

- **WHEN** an AppUser submits an event with `occurred_at_client` set to 6 hours in the past
- **AND** no later event exists for this AppUser
- **THEN** the request is accepted

#### Scenario: GPS coordinates required

- **WHEN** an AppUser submits an event whose request body lacks `lat` or `lng`
- **THEN** the request is rejected with `VALIDATION` (or equivalent missing-field error)

### Requirement: Events for an AppUser are strictly ordered by client time

The system SHALL, for each AppUser, reject any incoming event whose `occurred_at_client` is less than or equal to the most recent stored event's `occurred_at_client` for that AppUser. The error code SHALL be `OUT_OF_ORDER` (HTTP 409). The first event for an AppUser SHALL accept any client time. The check SHALL be scoped per AppUser; events for different AppUsers SHALL NOT interfere.

#### Scenario: Earlier-than-last event rejected

- **WHEN** an AppUser's most recent event has `occurred_at_client = 10:00`
- **AND** the AppUser submits a new event with `occurred_at_client = 09:30`
- **THEN** the request is rejected with `OUT_OF_ORDER`
- **AND** no event is recorded

#### Scenario: Equal-to-last event rejected

- **WHEN** an AppUser's most recent event has `occurred_at_client = 10:00`
- **AND** the AppUser submits a new event with `occurred_at_client = 10:00`
- **THEN** the request is rejected with `OUT_OF_ORDER`

#### Scenario: First event accepts any time

- **WHEN** an AppUser has zero stored events
- **AND** submits an event with any `occurred_at_client` (past or future)
- **THEN** the event is accepted (subject to other validation)

#### Scenario: Per-AppUser scoping

- **WHEN** AppUser A's latest event has `occurred_at_client = 10:00`
- **AND** AppUser B submits an event with `occurred_at_client = 09:00`
- **THEN** AppUser B's request is accepted (the order check is per AppUser, not Org-wide)

### Requirement: AppUser submits checkin events via /app/checkin/events

The system SHALL accept `POST /app/checkin/events` with body `{ event_type, lat, lng, accuracy?, manual_label?, occurred_at_client }`. The system SHALL run state-machine validation, transfer-toggle validation, ordering validation, attempt reverse geocoding (fail-soft, see separate requirement), insert the event, and update `checkin_user_status` atomically. On success the response SHALL be `201 Created` with `{ event, status }`.

#### Scenario: Successful event submission

- **WHEN** an authenticated AppUser sends `POST /app/checkin/events` with valid body for a legal transition
- **THEN** a new `checkin_events` row is inserted with `app_user_id`, `org_id`, `event_type`, `occurred_at_client`, `occurred_at_server = now`, `source = app`, `initiated_by_kind = app_user`, `initiated_by_id = ctx.app_user_id`, and the location document
- **AND** the AppUser's `checkin_user_status` is updated per the state-machine table
- **AND** the response is `201` with `{ event, status }`

### Requirement: AppUser fetches own status and history

The system SHALL provide `GET /app/checkin/status` returning `{ status, current_shift_started_at, last_event }` and `GET /app/checkin/events` returning a cursor-paginated list of the AppUser's own events (newest first by `occurred_at_client`, default page size 50). Both endpoints SHALL scope strictly to the caller; AppUsers SHALL NOT see another AppUser's events through this surface.

#### Scenario: AppUser fetches their own status

- **WHEN** an authenticated AppUser sends `GET /app/checkin/status`
- **THEN** the response is the caller's `checkin_user_status` row plus the resolved `last_event` document if `last_event_id` is non-null

#### Scenario: AppUser lists own events

- **WHEN** an authenticated AppUser sends `GET /app/checkin/events`
- **THEN** the response contains up to 50 of the caller's events ordered by `occurred_at_client` descending
- **AND** events belonging to other AppUsers are not included

### Requirement: Admin lists AppUser status board

The system SHALL provide `GET /checkin/users` for dashboard admins, returning every AppUser in `current_org` with their current `checkin_user_status` (including a flag indicating whether the most recent event has `|occurred_at_client - occurred_at_server| > 1 hour`). AppUsers from other Orgs SHALL NOT appear. Members (non-admin) SHALL be rejected with `FORBIDDEN`.

#### Scenario: Admin sees current_org AppUsers and their status

- **WHEN** an authenticated admin sends `GET /checkin/users`
- **THEN** the response contains an array entry for every AppUser whose `org_id == current_org_id`, each carrying `{ user, status, current_shift_started_at, last_event, has_skew_warning }`

#### Scenario: Skew warning is computed per AppUser

- **WHEN** an AppUser's most recent event has `|occurred_at_client - occurred_at_server| > 1 hour`
- **THEN** that AppUser's response entry has `has_skew_warning = true`
- **AND** when within 1 hour, `has_skew_warning = false`

#### Scenario: Cross-Org AppUsers excluded

- **WHEN** an admin sends `GET /checkin/users` while `current_org = Org A`
- **THEN** AppUsers belonging to Org B are absent regardless of status

#### Scenario: Member cannot view checkin board

- **WHEN** a `member` sends `GET /checkin/users`
- **THEN** the request is rejected with `FORBIDDEN`

### Requirement: Admin views one AppUser's event history

The system SHALL provide `GET /checkin/users/:id/events` for dashboard admins, returning the target AppUser's events (cursor-paginated, newest first by `occurred_at_client`, default page size 50). The endpoint SHALL be scoped to `current_org`; targeting an AppUser belonging to another Org SHALL return `NOT_FOUND`. Members SHALL be rejected with `FORBIDDEN`.

#### Scenario: Admin views in-org AppUser events

- **WHEN** an authenticated admin sends `GET /checkin/users/:id/events` for an AppUser in `current_org`
- **THEN** the response contains the target's events with `event_type`, `occurred_at_client`, `occurred_at_server`, location, `source`, `initiated_by_kind`, and `has_skew_warning` per event

#### Scenario: Cross-Org target rejected

- **WHEN** an admin sends `GET /checkin/users/:id/events` for an AppUser whose `org_id != current_org_id`
- **THEN** the response is `NOT_FOUND`

### Requirement: Admin can force checkout an AppUser on shift

The system SHALL provide `POST /checkin/users/:id/force-checkout` with optional body `{ reason: String? }` (≤ 240 chars). The action SHALL only succeed when the target's current `status` is `on_site` or `in_transit`; otherwise SHALL respond `409 NOT_ON_DUTY`. On success the system SHALL insert a `clock_out` event with `source = admin_force`, `initiated_by_kind = dashboard_user`, `initiated_by_id = ctx.user_id`, `occurred_at_client = occurred_at_server = now`, and `location` copied from the AppUser's last event with `manual_label = "管理員強制收班"`. The reason text SHALL be stored on the event (separate field). The endpoint SHALL be admin-only and scoped to `current_org`.

#### Scenario: Admin force-checks-out an on-shift AppUser

- **WHEN** an admin sends `POST /checkin/users/:id/force-checkout` for an AppUser in `current_org` with `status = on_site` (or `in_transit`)
- **THEN** a `clock_out` event is inserted with `source = admin_force`, `initiated_by_kind = dashboard_user`, `initiated_by_id = caller`, location copied from the AppUser's last event, and `manual_label = "管理員強制收班"`
- **AND** the AppUser's status becomes `off_duty`

#### Scenario: Optional reason is stored

- **WHEN** an admin sends `POST /checkin/users/:id/force-checkout` with body `{ "reason": "shift ended via line manager" }`
- **THEN** the inserted event's `reason` field equals `"shift ended via line manager"`

#### Scenario: Off-duty target rejected

- **WHEN** an admin sends `POST /checkin/users/:id/force-checkout` for an AppUser with `status = off_duty`
- **THEN** the request is rejected with `NOT_ON_DUTY`

#### Scenario: Cross-Org target rejected

- **WHEN** an admin sends `POST /checkin/users/:id/force-checkout` for an AppUser whose `org_id != current_org_id`
- **THEN** the response is `NOT_FOUND`

#### Scenario: Member cannot force-checkout

- **WHEN** a `member` sends `POST /checkin/users/:id/force-checkout`
- **THEN** the request is rejected with `FORBIDDEN`

### Requirement: Org transfer-enabled toggle gates transfer events

The system SHALL store `Org.settings.checkin.transfer_enabled: bool` defaulting to `true` on Org creation. When `transfer_enabled == false`, the system SHALL reject `transfer_out` and `transfer_in` events with `403 TRANSFER_DISABLED`. When `transfer_enabled == true`, transfer events are subject only to state-machine validation. The toggle SHALL NOT affect `clock_in` or `clock_out`.

#### Scenario: New Org defaults to transfer enabled

- **WHEN** a new Org is created
- **THEN** `Org.settings.checkin.transfer_enabled` is `true`

#### Scenario: Transfer event rejected when toggle is off

- **WHEN** an AppUser whose Org has `transfer_enabled = false` submits a `transfer_out` (or `transfer_in`) event
- **THEN** the request is rejected with `TRANSFER_DISABLED`
- **AND** no event is recorded

#### Scenario: clock_in / clock_out unaffected by toggle

- **WHEN** an AppUser whose Org has `transfer_enabled = false` submits a valid `clock_in` or `clock_out` event
- **THEN** the request is processed normally

### Requirement: Transfer-enabled toggle is state-locked

The system SHALL allow an admin to update `Org.settings.checkin.transfer_enabled` via `PATCH /orgs/me/settings { transfer_enabled }` only when the count of AppUsers in `current_org` whose `checkin_user_status.status != off_duty` is zero. Otherwise the system SHALL respond `409 STATE_LOCKED` with body field `on_duty_count` indicating how many AppUsers must clock out before the toggle can change. Other settings (e.g. `timezone`) SHALL NOT be subject to the state-lock.

#### Scenario: Toggle accepted when no one is on shift

- **WHEN** every AppUser in `current_org` has `status = off_duty`
- **AND** an admin sends `PATCH /orgs/me/settings` with `{ transfer_enabled: false }`
- **THEN** `Org.settings.checkin.transfer_enabled` becomes `false`
- **AND** the response is `200 OK`

#### Scenario: Toggle rejected while someone is on shift

- **WHEN** at least one AppUser in `current_org` has `status = on_site` or `status = in_transit`
- **AND** an admin sends `PATCH /orgs/me/settings` with a `transfer_enabled` value
- **THEN** the request is rejected with `STATE_LOCKED`
- **AND** the response body contains `on_duty_count` equal to the number of non-`off_duty` AppUsers
- **AND** `Org.settings.checkin.transfer_enabled` is unchanged

#### Scenario: Timezone change not blocked by state-lock

- **WHEN** an AppUser is on shift in `current_org`
- **AND** an admin sends `PATCH /orgs/me/settings` with `{ timezone: "America/Los_Angeles" }`
- **THEN** the timezone is updated normally (no state-lock applies)

#### Scenario: Member cannot update settings

- **WHEN** a `member` sends `PATCH /orgs/me/settings` with any body
- **THEN** the request is rejected with `FORBIDDEN`

### Requirement: Reverse geocoding is fail-soft and abstracted via a trait

The system SHALL define a `ReverseGeocoder` interface with a single async method that accepts `(lat, lng)` and returns an optional human-readable region label. The system SHALL ship one implementation backed by Nominatim (or equivalent free service) configured with a fixed argus User-Agent and a 2-second per-request timeout. On any failure (timeout, non-2xx response, parse error, network error), the system SHALL store `region_name = null` on the event and SHALL still record the event normally — failure SHALL NOT cause the event-submission request to fail. The handler SHALL invoke the geocoder synchronously as part of event creation.

#### Scenario: Successful geocode populates region_name

- **WHEN** an AppUser submits a valid event and the geocoder returns a non-empty label
- **THEN** the stored event has `region_name` set to the returned label

#### Scenario: Geocoder timeout produces null region_name

- **WHEN** the geocoder times out (≥ 2 seconds)
- **THEN** the event is still recorded with `region_name = null`
- **AND** the event-submission request returns `201 Created`

#### Scenario: Geocoder error produces null region_name

- **WHEN** the geocoder returns a non-2xx response, malformed payload, or any other error
- **THEN** the event is still recorded with `region_name = null`

#### Scenario: Manual label is preserved across geocoding outcome

- **WHEN** an AppUser submits an event with `manual_label = "公司門口"`
- **THEN** the stored `manual_label` is `"公司門口"` regardless of whether geocoding succeeded
- **AND** the optional `region_name` is set independently
