## MODIFIED Requirements

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

## ADDED Requirements

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
