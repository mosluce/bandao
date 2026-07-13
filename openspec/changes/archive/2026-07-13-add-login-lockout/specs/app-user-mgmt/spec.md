## MODIFIED Requirements

### Requirement: Any Org member can list AppUsers in current Org; only admin can manage them

The system SHALL allow any authenticated dashboard user with an active membership in `current_org` (`admin` or `member`) to list AppUsers within `current_org` via `GET /app-users`. The response SHALL contain an array of AppUser DTOs (each `{ id, auth_source, username, external_key, display_name, status, needs_password_change, last_login_at, created_at, is_locked }`; `username` is present for internal users, `external_key` for external shadow users). For internal AppUsers, `is_locked` SHALL reflect whether `locked_until` is in the future; external shadow AppUsers are exempt from lockout tracking and SHALL always report `is_locked = false`. Raw `failed_login_attempts` and `locked_until` values SHALL NOT be exposed in this or any other API response. The list SHALL include both internal AppUsers and external shadow AppUsers that have logged in at least once, scoped strictly to `current_org` — AppUsers from other Orgs SHALL NOT be returned. Creating, updating, or resetting the password of an AppUser SHALL remain restricted to `admin` — this requirement only changes read access.

#### Scenario: Admin lists AppUsers

- **WHEN** an authenticated admin sends `GET /app-users`
- **THEN** the response contains every AppUser whose `org_id == current_org_id`, including external shadow users
- **AND** AppUsers belonging to other Orgs are absent
- **AND** the response excludes `password_hash`, `failed_login_attempts`, `locked_until`, and any session details

#### Scenario: External shadow users appear only after first login

- **WHEN** an Org uses `external_db` auth and an external user has never logged in
- **THEN** that user does not appear in `GET /app-users`
- **AND** after they log in once, they appear with `auth_source = external` and their resolved `external_key` and `display_name`

#### Scenario: Member can list AppUsers, identically to admin

- **WHEN** an authenticated dashboard user with role `member` sends `GET /app-users`
- **THEN** the response is `200 OK` with the same content a same-Org admin would receive

#### Scenario: Listing requires an active Org

- **WHEN** an authenticated dashboard user with `current_org_id == null` sends `GET /app-users`
- **THEN** the request is rejected with `NO_ACTIVE_ORG`

#### Scenario: Locked internal AppUser is flagged in the list

- **WHEN** an authenticated admin sends `GET /app-users` and an internal AppUser's `locked_until` is in the future
- **THEN** that user's entry has `is_locked = true`

#### Scenario: External shadow user always shows is_locked=false

- **WHEN** an authenticated admin sends `GET /app-users` for an Org with `auth_source == external_db`
- **THEN** every external shadow user's entry has `is_locked = false`, regardless of how many recent verification failures the external provider reported

### Requirement: AppUser logs in with org identifier, username, and password

The system SHALL accept `POST /app/auth/login` with body `{ org_code, username, password }`. The system SHALL resolve `org_code` to an Org using the same identifier rules as `register mode=join` (random 10-char code, active slug, or grace-period slug; lowercase-normalized for slug shapes). The system SHALL then delegate credential verification to the authentication provider selected by that Org's `auth_source` (see `external-db-auth`): the built-in provider for `internal`, the driver-specific external provider for `external_db`. On success the system SHALL ensure a local AppUser exists (looked up for `internal`, just-in-time provisioned for `external_db`), verify it is `active`, and issue a new `app_sessions` row referencing its `_id`. On any credential-failure path (Org not found, account not found, password mismatch, status disabled, account locked), the response SHALL be a generic `INVALID_CREDENTIALS` so callers cannot distinguish between cases. When the external provider cannot complete verification (connection/query/config error), the response SHALL be `EXTERNAL_AUTH_UNAVAILABLE`. Every internal AppUser SHALL carry `failed_login_attempts` (integer, default 0) and `locked_until` (optional timestamp, default null), tracked using the same threshold/duration configuration as dashboard-user lockout (`LOGIN_LOCKOUT_THRESHOLD`, default 3; `LOGIN_LOCKOUT_DURATION_SECONDS`, default 3600). While `locked_until` is in the future, the system SHALL reject the login with `INVALID_CREDENTIALS` without checking the password, and SHALL NOT modify `failed_login_attempts` or `locked_until` as a result. When an internal AppUser's password check fails and the account is not locked, the system SHALL atomically increment `failed_login_attempts`; on reaching the threshold, `locked_until` SHALL be set to `now + LOGIN_LOCKOUT_DURATION_SECONDS`. On a successful internal login, `failed_login_attempts` SHALL be reset to 0 and `locked_until` SHALL be cleared. AppUsers belonging to an Org with `auth_source == external_db` SHALL be exempt from this lockout tracking entirely — credential verification for those accounts is delegated to the external provider and is not locally rate-limited by this mechanism.

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

#### Scenario: Repeated failed attempts lock an internal AppUser

- **WHEN** a client's failed login attempt against an `internal`-auth AppUser brings `failed_login_attempts` to `LOGIN_LOCKOUT_THRESHOLD` (default 3)
- **THEN** `locked_until` is set to `now + LOGIN_LOCKOUT_DURATION_SECONDS` (default 1 hour)
- **AND** the response is `INVALID_CREDENTIALS`, identical in status and body to any other failed login

#### Scenario: Locked internal AppUser rejects login without checking the password

- **WHEN** a client sends `POST /app/auth/login` for an internal AppUser whose `locked_until` is in the future, regardless of whether the supplied password is correct
- **THEN** the response is `INVALID_CREDENTIALS`
- **AND** `failed_login_attempts` and `locked_until` are unchanged

#### Scenario: Successful login resets an internal AppUser's attempt counter

- **WHEN** an internal AppUser with a nonzero `failed_login_attempts` (but not currently locked) logs in successfully
- **THEN** `failed_login_attempts` is reset to 0 and `locked_until` is cleared

#### Scenario: External-auth AppUsers are exempt from lockout

- **WHEN** repeated failed `POST /app/auth/login` attempts are made against an Org with `auth_source == external_db`
- **THEN** no `failed_login_attempts` or `locked_until` tracking is applied to the resolved shadow AppUser, no matter how many attempts fail
- **AND** each attempt is still rejected per the external provider's own response (`INVALID_CREDENTIALS` or `EXTERNAL_AUTH_UNAVAILABLE`)

## ADDED Requirements

### Requirement: Admin can unlock an AppUser's account

The system SHALL allow a dashboard `admin` to manually clear an AppUser's lockout state via `POST /app-users/:id/unlock`. The endpoint SHALL only operate on AppUsers whose `org_id == current_org_id`; any other id SHALL respond `NOT_FOUND`. On success the system SHALL set `failed_login_attempts = 0` and `locked_until = null` on the target AppUser and respond `204 No Content`. Calling this endpoint on an account that is not currently locked (including any external shadow AppUser, which is never locked) SHALL be a no-op that still returns `204 No Content`.

#### Scenario: Admin unlocks a locked internal AppUser

- **WHEN** an admin sends `POST /app-users/:id/unlock` for an internal AppUser in `current_org` whose `locked_until` is in the future
- **THEN** the target's `failed_login_attempts` is set to 0 and `locked_until` is set to `null`
- **AND** the target can immediately log in again with their correct password
- **AND** the response is `204 No Content`

#### Scenario: Unlocking an account that isn't locked is a no-op

- **WHEN** an admin sends `POST /app-users/:id/unlock` for an AppUser in `current_org` whose `locked_until` is already `null`
- **THEN** the response is `204 No Content`
- **AND** no error is raised

#### Scenario: Cross-Org unlock rejected

- **WHEN** an admin sends `POST /app-users/:id/unlock` for an AppUser whose `org_id != current_org_id`
- **THEN** the request is rejected with `NOT_FOUND`

#### Scenario: Member cannot unlock

- **WHEN** a dashboard `member` sends `POST /app-users/:id/unlock`
- **THEN** the request is rejected with `FORBIDDEN`
