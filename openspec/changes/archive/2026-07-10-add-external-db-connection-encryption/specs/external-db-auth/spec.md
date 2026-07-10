## MODIFIED Requirements

### Requirement: External auth configuration is stored per Org with the connection password encrypted

The system SHALL store an Org's external auth configuration as `settings.external_auth` with fields `{ driver, host, port, database, username, password_encrypted, query, key_col, display_col, encrypt, trust_server_certificate }`. The connection password SHALL be stored encrypted at rest using the API's symmetric-encryption mechanism and SHALL NEVER be returned in any API response nor written to logs. API responses that surface the configuration SHALL expose a boolean `password_set` in place of the password.

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
