## ADDED Requirements

### Requirement: Org has a configurable timezone

Each Org SHALL hold a `timezone` field containing a valid IANA Time Zone Database identifier (e.g. `"Asia/Taipei"`, `"America/Los_Angeles"`, `"UTC"`). New Orgs SHALL default to `"Asia/Taipei"`. The system SHALL validate `timezone` against the IANA tz database on write; invalid values SHALL be rejected with `INVALID_TIMEZONE`. The `timezone` field is **display-only**: the system SHALL NOT use it for any data storage decisions, query date ranges, event ordering, or retention math. All timestamps in the database SHALL remain absolute (UTC) regardless of this field. Admin clients (admin-web, future Flutter app) SHALL render Org-scoped timestamps under this timezone.

#### Scenario: New Org defaults to Asia/Taipei

- **WHEN** a new Org is created via any mechanism (`register mode=create`, `POST /me/orgs`)
- **THEN** the Org record's `timezone` field equals `"Asia/Taipei"`

#### Scenario: Admin can update timezone

- **WHEN** an authenticated admin sends `PATCH /orgs/me/settings` with `{ "timezone": "America/Los_Angeles" }`
- **THEN** the Org record's `timezone` is updated to `"America/Los_Angeles"`
- **AND** the response is `200 OK` with the updated settings

#### Scenario: Invalid timezone rejected

- **WHEN** an admin sends `PATCH /orgs/me/settings` with a `timezone` value that is not in the IANA tz database (e.g. `"Mars/Olympus"`, `"GMT+8"`)
- **THEN** the request is rejected with `INVALID_TIMEZONE`
- **AND** the Org record is unchanged

#### Scenario: Timezone change does not affect stored timestamps

- **WHEN** an Org's `timezone` is changed from `"Asia/Taipei"` to `"America/Los_Angeles"`
- **THEN** every existing timestamp in the database (event records, audit timestamps, etc.) is unchanged
- **AND** subsequent display renders those same absolute timestamps under the new timezone

#### Scenario: Member cannot change timezone

- **WHEN** an authenticated user with role `member` sends `PATCH /orgs/me/settings` with `{ "timezone": "..." }`
- **THEN** the request is rejected with `FORBIDDEN`
