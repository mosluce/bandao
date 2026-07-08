## ADDED Requirements

### Requirement: App user authentication is resolved through a provider selected by the Org's auth source

The system SHALL resolve App user authentication through an `AppAuthProvider` selected by `current_org`'s `auth_source`. When `auth_source == internal` the system SHALL use the built-in provider (Mongo `app_users` + password hash). When `auth_source == external_db` the system SHALL use the driver-specific provider named by `external_auth.driver`. The MSSQL driver SHALL be the only external provider implemented; any other `driver` value SHALL cause external logins to fail with `EXTERNAL_AUTH_UNAVAILABLE`. A provider SHALL take `(account, password)` and return either a resolved identity `{ external_key, display_name }` or a typed error.

#### Scenario: Org with internal auth source uses built-in provider

- **WHEN** an App login is attempted for an Org whose `auth_source` is `internal` (or unset)
- **THEN** authentication is performed against Mongo `app_users` with password-hash verification
- **AND** no external database connection is opened

#### Scenario: Org with external_db auth source uses the MSSQL provider

- **WHEN** an App login is attempted for an Org whose `auth_source == external_db` and `external_auth.driver == "mssql"`
- **THEN** authentication is delegated to the MSSQL provider using the Org's `external_auth` configuration

#### Scenario: Unsupported driver fails closed

- **WHEN** an Org has `auth_source == external_db` with an `external_auth.driver` other than `mssql`
- **THEN** external logins for that Org are rejected with `EXTERNAL_AUTH_UNAVAILABLE`

### Requirement: External auth configuration is stored per Org with the connection password encrypted

The system SHALL store an Org's external auth configuration as `settings.external_auth` with fields `{ driver, host, port, database, username, password_encrypted, query, key_col, display_col }`. The connection password SHALL be stored encrypted at rest using the API's symmetric-encryption mechanism and SHALL NEVER be returned in any API response nor written to logs. API responses that surface the configuration SHALL expose a boolean `password_set` in place of the password.

#### Scenario: Configuration persists with encrypted password

- **WHEN** an admin saves an `external_auth` configuration including a connection password
- **THEN** the stored `settings.external_auth.password_encrypted` is ciphertext, not the plaintext password

#### Scenario: Configuration read never exposes the password

- **WHEN** any API response includes the Org's `external_auth` configuration
- **THEN** the response contains `password_set: true|false`
- **AND** the response contains no plaintext or ciphertext password field

### Requirement: External auth query is parameterized and validated

The system SHALL require the `external_auth.query` to contain the placeholders `@account` and `@password`, and SHALL bind the incoming account and password as query parameters — it SHALL NEVER build the query by string interpolation of user input. The system SHALL reject saving a configuration whose `query` is missing either placeholder, or whose `key_col` or `display_col` is empty. On a successful authentication the query SHALL return zero or one row; the system SHALL read `external_key` from the `key_col` column and `display_name` from the `display_col` column of the returned row.

#### Scenario: Query without required placeholders is rejected on save

- **WHEN** an admin saves an `external_auth` configuration whose `query` omits `@account` or `@password`
- **THEN** the save is rejected with a validation error
- **AND** the configuration is not persisted

#### Scenario: Credentials are bound as parameters, not interpolated

- **WHEN** a login runs the external query with an account containing SQL metacharacters (e.g. `' OR '1'='1`)
- **THEN** the value is passed as a bound parameter
- **AND** it is treated as a literal account value, never as SQL

#### Scenario: Matching row resolves identity from configured columns

- **WHEN** the external query returns exactly one row for the supplied account and password
- **THEN** `external_key` is read from the `key_col` column and `display_name` from the `display_col` column

### Requirement: Successful external authentication just-in-time provisions a shadow AppUser

On a successful external authentication the system SHALL upsert a local AppUser keyed by `(org_id, external_key)`. On first login it SHALL create the row with `auth_source = external`, `password_hash = null`, `display_name` from the resolved identity, `status = active`, and `needs_password_change = false`. On subsequent logins it SHALL reuse the same `_id`, refresh `display_name` from the resolved identity, and update `last_login_at`. The resulting `app_user_id` SHALL anchor sessions, check-in events, and location pings exactly as internal AppUsers do.

#### Scenario: First external login creates a shadow AppUser

- **WHEN** an external user authenticates successfully for the first time with resolved `external_key = "E001"`
- **THEN** a new `app_users` row is inserted with `org_id = current_org_id`, `auth_source = external`, `external_key = "E001"`, `password_hash = null`, `status = active`, `needs_password_change = false`
- **AND** a session is issued referencing that row's `_id`

#### Scenario: Repeat external login reuses the same shadow identity

- **WHEN** the same external user authenticates again after their `display_name` changed in the external database
- **THEN** the existing `app_users` row (same `_id`) is reused
- **AND** its `display_name` is refreshed and `last_login_at` is updated
- **AND** no duplicate row is created

#### Scenario: Shadow identity uniqueness is enforced per Org

- **WHEN** two external logins in the same Org resolve to the same `external_key`
- **THEN** they map to a single `app_users` row via the unique `(org_id, external_key)` index

### Requirement: External login error semantics distinguish bad credentials from system unavailability

The system SHALL respond to a failed credential match (the external query returns no row) with the generic `INVALID_CREDENTIALS`, identical to internal auth, so callers cannot distinguish unknown-account from wrong-password. The system SHALL respond to a connection failure, query-execution error, missing configuration, or unsupported driver with `EXTERNAL_AUTH_UNAVAILABLE`, so callers can tell a system problem apart from a mistyped credential. The submitted password SHALL NOT appear in any error message or log.

#### Scenario: No matching row is a credential failure

- **WHEN** the external query returns zero rows for the supplied account and password
- **THEN** the response is `INVALID_CREDENTIALS`
- **AND** no session and no shadow AppUser are created

#### Scenario: Connection failure is surfaced as unavailable

- **WHEN** the external database cannot be reached within the connection timeout, or the query fails to execute
- **THEN** the response is `EXTERNAL_AUTH_UNAVAILABLE`
- **AND** the submitted password is not written to any log

### Requirement: Admin can dry-run external auth via a test-login endpoint

The system SHALL provide `POST /orgs/me/external-auth/test-login`, admin-only and scoped to `current_org`, accepting a candidate `external_auth` configuration plus a test account and password. It SHALL run the full provider flow — connect, execute the parameterized query, resolve the columns — and return either the resolved `{ external_key, display_name }` or a specific diagnostic (cannot connect, query parse error, column not found, no matching row). It SHALL NOT create a session, SHALL NOT create or modify any shadow AppUser, and SHALL NOT write the test password to any log or database.

#### Scenario: Successful dry-run returns the resolved identity

- **WHEN** an admin posts a valid configuration with a real test account and password that matches one external row
- **THEN** the response reports success with the resolved `external_key` and `display_name`
- **AND** no session is issued and no `app_users` row is created or modified

#### Scenario: Dry-run reports a specific diagnostic on misconfiguration

- **WHEN** an admin posts a configuration whose `key_col` does not exist in the query result
- **THEN** the response reports a column-not-found diagnostic distinct from a connection or credential failure

#### Scenario: Test-login is admin-only and org-scoped

- **WHEN** a dashboard `member`, or a request with `current_org_id == null`, calls `POST /orgs/me/external-auth/test-login`
- **THEN** the request is rejected (`FORBIDDEN` for member, `NO_ACTIVE_ORG` when no active Org)
