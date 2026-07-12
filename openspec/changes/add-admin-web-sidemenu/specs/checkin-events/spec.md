## MODIFIED Requirements

### Requirement: Any Org member can list the AppUser status board

The system SHALL provide `GET /checkin/users` for any authenticated dashboard user with an active membership in `current_org` (`admin` or `member`), returning every AppUser in `current_org` with their current `checkin_user_status` (including a flag indicating whether the most recent event has `|occurred_at_client - occurred_at_server| > 1 hour`). AppUsers from other Orgs SHALL NOT appear.

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

#### Scenario: Member can view the checkin board, identically to admin

- **WHEN** a `member` sends `GET /checkin/users`
- **THEN** the response is `200 OK` with the same content a same-Org admin would receive

### Requirement: Any Org member can view one AppUser's event history

The system SHALL provide `GET /checkin/users/:id/events` for any authenticated dashboard user with an active membership in `current_org` (`admin` or `member`), returning the target AppUser's events (cursor-paginated, newest first by `occurred_at_client`, default page size 50). The endpoint SHALL be scoped to `current_org`; targeting an AppUser belonging to another Org SHALL return `NOT_FOUND`.

#### Scenario: Admin views in-org AppUser events

- **WHEN** an authenticated admin sends `GET /checkin/users/:id/events` for an AppUser in `current_org`
- **THEN** the response contains the target's events with `event_type`, `occurred_at_client`, `occurred_at_server`, location, `source`, `initiated_by_kind`, and `has_skew_warning` per event

#### Scenario: Cross-Org target rejected

- **WHEN** an admin sends `GET /checkin/users/:id/events` for an AppUser whose `org_id != current_org_id`
- **THEN** the response is `NOT_FOUND`

#### Scenario: Member can view AppUser event history, identically to admin

- **WHEN** a `member` sends `GET /checkin/users/:id/events` for an AppUser in `current_org`
- **THEN** the response is `200 OK` with the same content a same-Org admin would receive

Force-checkout (`POST /checkin/users/:id/force-checkout`) and the Org checkin settings update (`PATCH /orgs/me/settings`) are unaffected by this change and remain `admin`-only — see the unmodified "Admin can force checkout an AppUser on shift" requirement.
