## MODIFIED Requirements

### Requirement: Org has a settings container

Each Org SHALL have a `settings` object that future changes can extend with feature toggles (e.g. `transferEnabled`, `trackingEnabled`). The MVP `settings` MAY be empty but the field MUST be present. The `settings` object SHALL hold an `auth_source` field selecting how App users authenticate, with values `internal` or `external_db`; when absent it SHALL be treated as `internal`. When `auth_source == external_db`, `settings` SHALL also hold an `external_auth` sub-document holding the external database connection and query configuration (detailed in `external-db-auth`). New Orgs SHALL default to `auth_source = internal` with no `external_auth`.

#### Scenario: New Org has settings field

- **WHEN** a new Org is created
- **THEN** the Org record contains a `settings` object (may be empty `{}`)
- **AND** its effective `auth_source` is `internal`

#### Scenario: Missing auth_source defaults to internal

- **WHEN** an Org's `settings` has no `auth_source` field
- **THEN** the system treats the Org as using `internal` App-user authentication

#### Scenario: Switching to external_db carries an external_auth configuration

- **WHEN** an Org's `auth_source` is set to `external_db`
- **THEN** `settings.external_auth` is present with the external database connection and query configuration

## ADDED Requirements

### Requirement: Admin can switch an Org's App-user auth source

The system SHALL allow a dashboard `admin` to set `current_org`'s `auth_source` to `internal` or `external_db`. Setting it to `external_db` SHALL require a valid `external_auth` configuration (validated per `external-db-auth`). Switching the auth source SHALL NOT delete or modify any existing AppUser rows: internal AppUsers and external shadow AppUsers are preserved across switches, so switching back restores their ability to log in. Members (non-admin) SHALL NOT be allowed to change the auth source.

#### Scenario: Admin switches to external_db with valid configuration

- **WHEN** an admin sets `auth_source = external_db` with a configuration that passes validation
- **THEN** the Org's effective `auth_source` becomes `external_db`
- **AND** existing internal AppUser rows are left intact (unable to log in while external is active)

#### Scenario: Switching back to internal restores internal logins

- **WHEN** an admin sets `auth_source` back to `internal` after having used `external_db`
- **THEN** existing internal AppUsers can log in again with their previous passwords
- **AND** external shadow AppUser rows are left intact but cannot log in while internal is active

#### Scenario: Switching to external_db without a valid configuration is rejected

- **WHEN** an admin attempts to set `auth_source = external_db` without a valid `external_auth` configuration
- **THEN** the request is rejected with a validation error
- **AND** the Org's `auth_source` is unchanged

#### Scenario: Member cannot change the auth source

- **WHEN** a dashboard `member` attempts to change `auth_source`
- **THEN** the request is rejected with `FORBIDDEN`
