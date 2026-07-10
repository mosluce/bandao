## MODIFIED Requirements

### Requirement: Org has a settings container

Each Org SHALL have a `settings` object that future changes can extend with feature toggles (e.g. `transferEnabled`, `trackingEnabled`). The MVP `settings` MAY be empty but the field MUST be present. The `settings` object SHALL hold an `auth_source` field selecting how App users authenticate, with values `internal` or `external_db`; when absent it SHALL be treated as `internal`. When `auth_source == external_db`, `settings` SHALL also hold an `external_auth` sub-document holding the external database connection and query configuration (detailed in `external-db-auth`). New Orgs SHALL default to `auth_source = internal` with no `external_auth`.

The `settings` object MAY additionally hold a `legacy_backfill` sub-document configuring a legacy check-in data source (connection, field mapping, action mapping) as detailed in `legacy-checkin-backfill`. Its presence is independent of `auth_source` — an Org may have `legacy_backfill` configured regardless of whether it uses `internal` or `external_db` App-user authentication. When absent, no legacy backfill behavior is triggered for that Org.

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

#### Scenario: Missing legacy_backfill means no backfill behavior

- **WHEN** an Org's `settings` has no `legacy_backfill` sub-document
- **THEN** logging in as an AppUser of that Org never triggers a legacy backfill task, regardless of the Org's `auth_source`
