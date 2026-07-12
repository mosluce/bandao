## ADDED Requirements

### Requirement: An Org can own multiple independently-managed API tokens

The system SHALL allow a dashboard `admin` to create any number of named API tokens scoped to `current_org`. Each token SHALL be independently manageable — rotating, disabling, enabling, or deleting one token SHALL NOT affect any other token belonging to the same Org.

#### Scenario: Admin creates a second token without affecting the first

- **WHEN** an Org already has one active API token
- **AND** the admin creates a second token with a different name
- **THEN** both tokens exist and authenticate independently
- **AND** neither token's creation affects the other's `token_hash` or `status`

### Requirement: Each API token is bound to one or more scopes from a known, closed set

The system SHALL require every API token to carry at least one scope drawn from a fixed set of known scope values (`ApiTokenScope`). The system SHALL NOT accept arbitrary free-text scope values. Creating a token with an empty scope list SHALL be rejected. Consuming endpoints SHALL check the presented token's scopes and SHALL reject requests where the required scope is absent, independent of the token's `status`.

#### Scenario: Creating a token with no scopes is rejected

- **WHEN** an admin attempts to create an API token with `scopes: []`
- **THEN** the request is rejected with a validation error
- **AND** no `org_api_tokens` row is persisted

#### Scenario: Creating a token with an unknown scope value is rejected

- **WHEN** an admin attempts to create an API token whose `scopes` includes a value outside the known set
- **THEN** the request is rejected with a validation error

### Requirement: Token secrets are shown once and stored only as a hash

The system SHALL generate the token secret using a cryptographically secure random source, return the full plaintext secret to the caller exactly once (at creation and at each rotation), and SHALL NEVER persist or subsequently return the plaintext. The system SHALL store a cryptographic hash of the secret (`token_hash`) for authentication lookups and a short, non-reconstructable `token_prefix` for UI display purposes.

#### Scenario: Plaintext is returned once at creation

- **WHEN** an admin creates a new API token
- **THEN** the creation response includes the full plaintext secret
- **AND** no subsequent API response (list, get) ever includes the plaintext secret

#### Scenario: Listing tokens never exposes the secret

- **WHEN** an admin lists an Org's API tokens
- **THEN** each entry includes `name`, `scopes`, `status`, `token_prefix`, `created_at`, and `last_used_at`
- **AND** no entry includes `token_hash` or any reconstructable form of the plaintext secret

### Requirement: API tokens authenticate via a distinctly-prefixed Bearer credential

The system SHALL accept API token authentication via the `Authorization: Bearer <token>` header, using the same header as AppUser session authentication. Every generated API token SHALL carry a fixed, distinguishing prefix so the two credential types can be told apart without a database lookup. A presented bearer value carrying the API-token prefix SHALL be resolved exclusively against `org_api_tokens`; a bearer value without that prefix SHALL be resolved exclusively via the existing AppUser session path. A token whose `status` is not `active`, or whose hash matches no stored token, SHALL be rejected with a generic unauthorized error that does not distinguish "disabled" from "not found."

#### Scenario: Prefixed bearer value resolves against org_api_tokens

- **WHEN** a request carries `Authorization: Bearer bandao_at_<...>`
- **THEN** the system looks up the token by its hash in `org_api_tokens`
- **AND** does not attempt to resolve it as an AppUser session token

#### Scenario: Non-prefixed bearer value is unaffected by this feature

- **WHEN** a request carries an `Authorization: Bearer <token>` value without the API-token prefix
- **THEN** the system resolves it via the existing AppUser session path exactly as before this feature existed

#### Scenario: Disabled token is rejected

- **WHEN** a request presents a token whose `status == disabled`
- **THEN** the request is rejected as unauthorized
- **AND** the error response does not reveal that the token exists but is disabled

#### Scenario: Successful authentication updates last_used_at

- **WHEN** a request authenticates successfully via an API token
- **THEN** that token's `last_used_at` is updated to the current time

### Requirement: API tokens never expire on their own

The system SHALL NOT enforce any time-based expiration on API tokens. A token created or rotated at any point in the past SHALL remain valid indefinitely until an admin explicitly disables, rotates, or deletes it.

#### Scenario: Token remains valid indefinitely

- **WHEN** an API token was created a long time ago and has never been rotated, disabled, or deleted
- **THEN** it continues to authenticate successfully

### Requirement: Admins can rotate, disable, enable, and delete individual tokens

The system SHALL allow a dashboard `admin` to rotate a token (generating a new secret while preserving its `name` and `scopes`; the previous secret SHALL immediately stop authenticating), disable a token (immediately stops authenticating, reversible), re-enable a disabled token (restores authentication with its existing secret, no new secret is generated), and delete a token (immediately and irreversibly stops authenticating; the row is removed).

#### Scenario: Rotating a token invalidates the previous secret immediately

- **WHEN** an admin rotates an active token
- **THEN** a new plaintext secret is returned once
- **AND** the previous secret no longer authenticates
- **AND** the token's `name` and `scopes` are unchanged

#### Scenario: Disabling a token is reversible

- **WHEN** an admin disables an active token and later re-enables it
- **THEN** the token authenticates again using its existing (unchanged) secret

#### Scenario: Deleting a token is irreversible

- **WHEN** an admin deletes a token
- **THEN** the token immediately stops authenticating
- **AND** no admin action can restore it — a replacement requires creating a new token

### Requirement: API token management is admin-only and Org-scoped

The system SHALL restrict all API token management endpoints (list, create, rotate, status change, delete) to dashboard users with `admin` role on `current_org`. A member (non-admin) SHALL receive `FORBIDDEN`. An admin of one Org SHALL NOT be able to view or manage another Org's API tokens.

#### Scenario: Member is forbidden from managing API tokens

- **WHEN** a dashboard user with `member` role attempts to create, list, rotate, disable, or delete an API token
- **THEN** the request is rejected with `FORBIDDEN`

#### Scenario: Cross-Org access is rejected

- **WHEN** an admin of Org A attempts to rotate, disable, or delete a token belonging to Org B
- **THEN** the request is rejected as not found, without revealing that the token exists under Org B
