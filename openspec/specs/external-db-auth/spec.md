# external-db-auth Specification

## Purpose

Defines how an Org can authenticate App users against an external database instead of the built-in Mongo credential store: provider selection by the Org's `auth_source`, per-Org encrypted connection configuration, parameterized query validation, just-in-time shadow AppUser provisioning, error semantics that separate bad credentials from system unavailability, and an admin dry-run test-login endpoint.

## Requirements

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

The system SHALL store an Org's external auth configuration as `settings.external_auth` with fields `{ driver, host, port, database, username, password_encrypted, query, key_col, display_col, encrypt, trust_server_certificate, list_query }`. The connection password SHALL be stored encrypted at rest using the API's symmetric-encryption mechanism and SHALL NEVER be returned in any API response nor written to logs. API responses that surface the configuration SHALL expose a boolean `password_set` in place of the password. `list_query` is optional — see the "Admin can manually sync the external user roster" requirement for its shape and validation.

The `encrypt` field SHALL be one of `off`, `optional`, or `required`, controlling the transport encryption the driver negotiates with the external database (`off` = no encryption, `optional` = encrypt when the server supports it, `required` = encryption mandatory). The `trust_server_certificate` field SHALL be a boolean controlling whether a server certificate that fails normal validation (e.g. self-signed) is trusted. Both fields are non-secret configuration: unlike the password they SHALL be stored in plaintext and MAY be returned in configuration responses. When either field is absent from a stored document, the system SHALL treat `encrypt` as `optional` and `trust_server_certificate` as `true`, so existing configurations remain valid without migration.

The driver SHALL apply the configured transport encryption level, and SHALL trust an otherwise-invalid server certificate only when `trust_server_certificate` is `true`.

#### Scenario: Configuration persists with encrypted password

- **WHEN** an admin saves an `external_auth` configuration including a connection password
- **THEN** the stored `settings.external_auth.password_encrypted` is ciphertext, not the plaintext password

#### Scenario: Configuration read never exposes the password

- **WHEN** any API response includes the Org's `external_auth` configuration
- **THEN** the response contains `password_set: true|false`
- **AND** the response contains no plaintext or ciphertext password field
- **AND** the response includes the non-secret `encrypt` and `trust_server_certificate` values as stored

#### Scenario: Missing encryption fields default to optional + trusted

- **WHEN** a stored `external_auth` document predates this change and has no `encrypt` or `trust_server_certificate` field
- **THEN** the system treats `encrypt` as `optional` and `trust_server_certificate` as `true`
- **AND** no migration of the stored document is required

#### Scenario: Encryption level and cert trust are applied to the connection

- **WHEN** a login or test-login opens the external database connection with `encrypt = off`
- **THEN** the driver negotiates no transport encryption
- **AND WHEN** `encrypt = required` the driver requires transport encryption
- **AND** an otherwise-invalid server certificate is trusted only when `trust_server_certificate` is `true`

#### Scenario: Configuration predating this change has no list_query

- **WHEN** a stored `external_auth` document has no `list_query` field
- **THEN** the system treats the manual-sync feature as unavailable for that Org until an admin sets one
- **AND** no migration of the stored document is required

### Requirement: Admin can manually sync the external user roster

The system SHALL provide `POST /orgs/me/external-auth/sync`, admin-only, scoped to `current_org`, available only when `current_org.auth_source == external_db`. The system SHALL reject the request with `EXTERNAL_AUTH_NOT_ENABLED` when the Org's `auth_source` is not `external_db`, and with a validation error when no `list_query` is configured. On invocation the system SHALL execute the stored `list_query` against the Org's configured external database with no bound parameters, read `external_key` from `key_col` and `display_name` from `display_col` for each returned row, and for each row:

- If `key_col` is missing (NULL) or empty for that row, SHALL skip the row and record it in the response's skip list with a reason.
- Otherwise, if no local `AppUser` exists for that `(org_id, external_key)`, SHALL create one with `status = active`, `last_login_at = null`, and the resolved `display_name`.
- Otherwise (a local `AppUser` already exists for that `external_key`), SHALL update its `display_name` to the resolved value, and SHALL NOT modify `last_login_at`.

The system SHALL NOT modify or remove any local `AppUser` whose `external_key` was not present in the `list_query` result. If `key_col` or `display_col` does not exist as a column name in the query result at all (as opposed to being NULL for a specific row), the system SHALL fail the entire sync with no writes to any `AppUser`, distinct from the per-row skip case. Connection failures and query-execution errors SHALL also fail the entire sync with no writes.

#### Scenario: New external users are created without a fabricated login time

- **WHEN** an admin runs sync and a row's `external_key` has no matching local `AppUser`
- **THEN** a new `AppUser` is created with `status = active`, `auth_source = external`, and `last_login_at = null`

#### Scenario: Existing external users have their display name refreshed, login time untouched

- **WHEN** an admin runs sync and a row's `external_key` matches an existing local `AppUser`
- **THEN** that `AppUser`'s `display_name` is updated to the row's resolved value
- **AND** its `last_login_at` is unchanged

#### Scenario: Local users absent from the sync result are left untouched

- **WHEN** an admin runs sync and a local `AppUser` with `auth_source = external` has an `external_key` not present in the `list_query` result
- **THEN** that `AppUser`'s `status`, `display_name`, and every other field are unchanged

#### Scenario: A row with an empty key column is skipped, not fatal

- **WHEN** an admin runs sync and one row in the result has a NULL or empty `key_col` value while other rows do not
- **THEN** that row is skipped and recorded in the response's skip list with a reason
- **AND** every other row is processed normally
- **AND** the response is still a success

#### Scenario: A misconfigured key_col or display_col fails the whole sync

- **WHEN** an admin runs sync and the configured `key_col` or `display_col` does not exist as a column name anywhere in the `list_query` result
- **THEN** the request fails
- **AND** no `AppUser` is created or modified

#### Scenario: Sync is rejected when the Org is not in external_db mode

- **WHEN** an admin of an Org whose `auth_source == internal` calls `POST /orgs/me/external-auth/sync`
- **THEN** the request is rejected with `EXTERNAL_AUTH_NOT_ENABLED`

#### Scenario: Sync is rejected when no list_query is configured

- **WHEN** an admin of an Org with `auth_source == external_db` but no `list_query` set calls `POST /orgs/me/external-auth/sync`
- **THEN** the request is rejected with a validation error

### Requirement: The sync query is validated as an unparameterized read, distinct from the login query

The system SHALL reject saving a `list_query` that contains the `@account` or `@password` placeholders — unlike the login `query`, `list_query` SHALL be executed with no bound parameters. The system SHALL otherwise apply the same driver-support validation as the login query (only `mssql` is supported).

#### Scenario: Saving a list_query containing @account or @password is rejected

- **WHEN** an admin saves an `external_auth` configuration whose `list_query` contains `@account` or `@password`
- **THEN** the save is rejected with a validation error
- **AND** the configuration is not persisted

### Requirement: External auth query is parameterized and validated

The system SHALL require the `external_auth.query` to contain the placeholders `@account` and `@password`, and SHALL bind the incoming account and password as query parameters — it SHALL NEVER build the query by string interpolation of user input. The system SHALL reject saving a configuration whose `query` is missing either placeholder, or whose `key_col` or `display_col` is empty, or whose `encrypt` is not one of `off`, `optional`, or `required`. On a successful authentication the query SHALL return zero or one row; the system SHALL read `external_key` from the `key_col` column and `display_name` from the `display_col` column of the returned row.

#### Scenario: Query without required placeholders is rejected on save

- **WHEN** an admin saves an `external_auth` configuration whose `query` omits `@account` or `@password`
- **THEN** the save is rejected with a validation error
- **AND** the configuration is not persisted

#### Scenario: Invalid encrypt value is rejected on save

- **WHEN** an admin saves an `external_auth` configuration whose `encrypt` is not `off`, `optional`, or `required`
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

### Requirement: External-auth configuration is only visible to dashboard admins

The system SHALL include the `external_auth` field in any API response only when the caller is resolved as a dashboard `admin` of the Org the configuration belongs to. Every other caller context — a dashboard `member`, an unauthenticated caller, or an AppUser (mobile) session — SHALL receive a response with the `external_auth` field entirely absent, not an empty object and not a partially-redacted one.

#### Scenario: Dashboard admin sees the configuration

- **WHEN** a dashboard `admin` of an Org with `auth_source == external_db` requests any endpoint that returns that Org as part of the response (e.g. `GET /me`, `POST /auth/login`)
- **THEN** the response's Org representation includes the `external_auth` field with the password-free configuration summary

#### Scenario: Dashboard member does not see the configuration

- **WHEN** a dashboard `member` of an Org with `auth_source == external_db` requests any endpoint that returns that Org as part of the response
- **THEN** the response's Org representation does NOT include an `external_auth` field at all

#### Scenario: AppUser session does not see the configuration

- **WHEN** an authenticated AppUser calls `POST /app/auth/login` or `GET /app/me` for an Org with `auth_source == external_db`
- **THEN** the response's Org representation does NOT include an `external_auth` field at all, regardless of the AppUser's own `auth_source`

#### Scenario: Endpoints already restricted to admin are unaffected

- **WHEN** a dashboard `admin` calls an endpoint that already requires the `admin` role to reach at all (e.g. `POST /orgs/me/external-auth`, `POST /orgs/me/owner`)
- **THEN** the response continues to include `external_auth` as before — this requirement changes visibility for callers who were never required to be admin, not for already-admin-gated endpoints
