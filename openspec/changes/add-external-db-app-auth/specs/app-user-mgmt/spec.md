## MODIFIED Requirements

### Requirement: AppUser logs in with org identifier, username, and password

The system SHALL accept `POST /app/auth/login` with body `{ org_code, username, password }`. The system SHALL resolve `org_code` to an Org using the same identifier rules as `register mode=join` (random 10-char code, active slug, or grace-period slug; lowercase-normalized for slug shapes). The system SHALL then delegate credential verification to the authentication provider selected by that Org's `auth_source` (see `external-db-auth`): the built-in provider for `internal`, the driver-specific external provider for `external_db`. On success the system SHALL ensure a local AppUser exists (looked up for `internal`, just-in-time provisioned for `external_db`), verify it is `active`, and issue a new `app_sessions` row referencing its `_id`. On any credential-failure path (Org not found, account not found, password mismatch, status disabled), the response SHALL be a generic `INVALID_CREDENTIALS` so callers cannot distinguish between cases. When the external provider cannot complete verification (connection/query/config error), the response SHALL be `EXTERNAL_AUTH_UNAVAILABLE`.

#### Scenario: Successful AppUser login (internal auth source)

- **WHEN** a client sends `POST /app/auth/login` with `{ org_code, username, password }` for an `internal`-auth Org, matching an `active` AppUser whose password verifies
- **THEN** a new `app_sessions` row is inserted with a randomly generated opaque token (≥256 bits of entropy), `app_user_id = user._id`, `created_at = now`, and `expires_at = now + session_ttl`
- **AND** the AppUser's `last_login_at` is updated to `now`
- **AND** the response is `200 OK` with body `{ token, expires_at, user: AppUserDto, org: OrgDto, needs_password_change }`

#### Scenario: Successful AppUser login (external_db auth source)

- **WHEN** a client sends `POST /app/auth/login` for an `external_db`-auth Org with credentials that the external provider resolves to an identity
- **THEN** a shadow AppUser is looked up or provisioned per `external-db-auth` and a session is issued referencing its `_id`
- **AND** the response is `200 OK` with body `{ token, expires_at, user: AppUserDto, org: OrgDto, needs_password_change }`

#### Scenario: Unknown org_code rejected

- **WHEN** a client sends `POST /app/auth/login` with an `org_code` that resolves to no Org
- **THEN** the response is `INVALID_CREDENTIALS`
- **AND** no `app_sessions` row is created
- **AND** no information distinguishes this case from "wrong username" or "wrong password"

#### Scenario: Credential mismatch rejected regardless of auth source

- **WHEN** a client sends `POST /app/auth/login` with a valid `org_code` but credentials that the selected provider does not accept (unknown account, wrong password, or disabled local user)
- **THEN** the response is `INVALID_CREDENTIALS`

#### Scenario: External provider unavailable is distinguishable

- **WHEN** a client sends `POST /app/auth/login` for an `external_db`-auth Org and the external database cannot be reached or the query errors
- **THEN** the response is `EXTERNAL_AUTH_UNAVAILABLE`
- **AND** no `app_sessions` row is created

#### Scenario: Disabled AppUser cannot log in

- **WHEN** a client sends `POST /app/auth/login` with credentials matching an AppUser (internal or external shadow) whose `status = disabled`
- **THEN** the response is `INVALID_CREDENTIALS`
- **AND** no `app_sessions` row is created

#### Scenario: Slug in grace period still works for AppUser login

- **WHEN** a client sends `POST /app/auth/login` with `org_code` set to a slug that the Org changed away from less than 30 days ago
- **THEN** resolution succeeds against the grace-history slug and login proceeds against that Org's configured auth source

#### Scenario: Username comparison is case-insensitive (internal auth source)

- **WHEN** a client sends `POST /app/auth/login` with `username = "Alice"` for an `internal`-auth Org where the AppUser was created with `username = "alice"`
- **THEN** the lookup matches via `username_lower` and proceeds

### Requirement: Admin can list AppUsers in current Org

The system SHALL allow a dashboard `admin` to list AppUsers within `current_org` via `GET /app-users`. The response SHALL contain an array of AppUser DTOs (each `{ id, auth_source, username, external_key, display_name, status, needs_password_change, last_login_at, created_at }`; `username` is present for internal users, `external_key` for external shadow users). The list SHALL include both internal AppUsers and external shadow AppUsers that have logged in at least once, scoped strictly to `current_org` — AppUsers from other Orgs SHALL NOT be returned. Members (non-admin) SHALL NOT be allowed to list AppUsers.

#### Scenario: Admin lists AppUsers

- **WHEN** an authenticated admin sends `GET /app-users`
- **THEN** the response contains every AppUser whose `org_id == current_org_id`, including external shadow users
- **AND** AppUsers belonging to other Orgs are absent
- **AND** the response excludes `password_hash` and any session details

#### Scenario: External shadow users appear only after first login

- **WHEN** an Org uses `external_db` auth and an external user has never logged in
- **THEN** that user does not appear in `GET /app-users`
- **AND** after they log in once, they appear with `auth_source = external` and their resolved `external_key` and `display_name`

#### Scenario: Member cannot list AppUsers

- **WHEN** an authenticated dashboard user with role `member` sends `GET /app-users`
- **THEN** the request is rejected with `FORBIDDEN`

#### Scenario: Listing requires an active Org

- **WHEN** an authenticated dashboard user with `current_org_id == null` sends `GET /app-users`
- **THEN** the request is rejected with `NO_ACTIVE_ORG`

## ADDED Requirements

### Requirement: Identity-mutating AppUser endpoints are disabled while the Org uses external auth

While `current_org.auth_source == external_db`, the system SHALL reject the internal-only identity-mutating endpoints — create (`POST /app-users`) and password reset (`POST /app-users/:id/password-reset`) — with `EXTERNAL_AUTH_MODE`, because credentials for external users are owned by the external database, not by the system. Disabling an AppUser (`PATCH /app-users/:id` with `status`) SHALL remain available in both modes as a local block. When the Org uses `internal` auth these endpoints SHALL behave exactly as before.

#### Scenario: Create AppUser rejected in external mode

- **WHEN** an admin sends `POST /app-users` while `current_org.auth_source == external_db`
- **THEN** the request is rejected with `EXTERNAL_AUTH_MODE`
- **AND** no `app_users` row is created

#### Scenario: Password reset rejected in external mode

- **WHEN** an admin sends `POST /app-users/:id/password-reset` while `current_org.auth_source == external_db`
- **THEN** the request is rejected with `EXTERNAL_AUTH_MODE`

#### Scenario: Disabling a shadow user still works in external mode

- **WHEN** an admin sends `PATCH /app-users/:id` with `{ "status": "disabled" }` for an external shadow user in `current_org`
- **THEN** the shadow user's `status` becomes `disabled`
- **AND** every `app_sessions` row referencing that `app_user_id` is deleted
- **AND** subsequent external logins resolving to that `external_key` are rejected with `INVALID_CREDENTIALS`

#### Scenario: Internal-mode endpoints unchanged

- **WHEN** an Org uses `internal` auth
- **THEN** create and password-reset behave exactly as specified for internal AppUsers
