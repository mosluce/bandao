## MODIFIED Requirements

### Requirement: Transfer-enabled toggle is state-locked

The system SHALL allow an admin to update either
`Org.settings.checkin.transfer_enabled` or
`Org.settings.checkin.location_tracking_enabled` via
`PATCH /orgs/me/settings` only when the count of AppUsers in
`current_org` whose `checkin_user_status.status != off_duty` is zero.
Otherwise the system SHALL respond `409 STATE_LOCKED` with body field
`on_duty_count` indicating how many AppUsers must clock out before the
toggle can change. The state-lock check SHALL fire when EITHER toggle
is present in the request body, and SHALL apply uniformly to both —
they share a single lock since the underlying concern (data
inconsistency caused by a settings flip mid-shift) is the same. Other
settings (e.g. `timezone`) SHALL NOT be subject to the state-lock.

#### Scenario: Transfer toggle change allowed when nobody is on duty

- **GIVEN** every AppUser in `current_org` has `checkin_user_status.status == off_duty`
- **AND** an admin sends `PATCH /orgs/me/settings` with `{ transfer_enabled: false }`
- **THEN** `Org.settings.checkin.transfer_enabled` becomes `false`

#### Scenario: Location tracking toggle change allowed when nobody is on duty

- **GIVEN** every AppUser in `current_org` has `checkin_user_status.status == off_duty`
- **AND** an admin sends `PATCH /orgs/me/settings` with `{ location_tracking_enabled: true }`
- **THEN** `Org.settings.checkin.location_tracking_enabled` becomes `true`

#### Scenario: Transfer toggle change blocked when someone is on duty

- **GIVEN** at least one AppUser in `current_org` has `checkin_user_status.status != off_duty`
- **AND** an admin sends `PATCH /orgs/me/settings` with a `transfer_enabled` value
- **THEN** the request is rejected with `STATE_LOCKED`
- **AND** the response body's `on_duty_count` reflects the actual count
- **AND** `Org.settings.checkin.transfer_enabled` is unchanged

#### Scenario: Location tracking toggle change blocked when someone is on duty

- **GIVEN** at least one AppUser in `current_org` has `checkin_user_status.status != off_duty`
- **AND** an admin sends `PATCH /orgs/me/settings` with a `location_tracking_enabled` value
- **THEN** the request is rejected with `STATE_LOCKED`
- **AND** the response body's `on_duty_count` reflects the actual count
- **AND** `Org.settings.checkin.location_tracking_enabled` is unchanged

#### Scenario: Both toggles in one request, lock applies to the whole patch

- **GIVEN** at least one AppUser in `current_org` is non-`off_duty`
- **AND** an admin sends `PATCH /orgs/me/settings` with both `transfer_enabled` and `location_tracking_enabled` values in the body
- **THEN** the request is rejected with `STATE_LOCKED` (no partial application — both fields are atomic with respect to the lock)
- **AND** neither toggle is updated

#### Scenario: Timezone change not blocked by state-lock

- **GIVEN** at least one AppUser in `current_org` is non-`off_duty`
- **AND** an admin sends `PATCH /orgs/me/settings` with `{ timezone: "America/Los_Angeles" }`
- **THEN** the timezone is updated normally (no state-lock applies)

#### Scenario: Timezone + toggle in same patch fall under state-lock

- **GIVEN** at least one AppUser in `current_org` is non-`off_duty`
- **AND** an admin sends `PATCH /orgs/me/settings` with both `timezone` and one of the toggles
- **THEN** the request is rejected with `STATE_LOCKED` (toggle presence pulls the whole patch under the lock)

#### Scenario: Member cannot update settings

- **WHEN** a `member` sends `PATCH /orgs/me/settings` with any body
- **THEN** the request is rejected with `403 FORBIDDEN`
