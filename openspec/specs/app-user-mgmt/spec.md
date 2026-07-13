# app-user-mgmt Specification

## Purpose
TBD - created by archiving change add-app-user-mgmt. Update Purpose after archive.
## Requirements
### Requirement: AppUser identity is per-Org and admin-managed

The system SHALL maintain a `app_users` collection holding identity records for end-users of the mobile app. Each row SHALL contain `_id`, `org_id` (FK to `Org`, immutable), `username`, `username_lower` (lowercased copy used for case-insensitive uniqueness), `display_name`, `password_hash`, `status: active | disabled`, `needs_password_change: bool`, `last_login_at: DateTime | null`, `created_by_dashboard_user_id`, `created_at`, `updated_at`. The system SHALL enforce a unique index on `(org_id, username_lower)`. The system SHALL NOT provide any self-registration path for AppUsers; identities are created exclusively through admin endpoints.

#### Scenario: New AppUser row records its creator and Org

- **WHEN** an admin successfully creates an AppUser
- **THEN** the new row's `org_id` equals the admin's `current_org_id`
- **AND** `created_by_dashboard_user_id` equals the admin's user id
- **AND** `status = active`, `needs_password_change = true`, `last_login_at = null`

#### Scenario: Same username can exist in different Orgs

- **WHEN** Org A already has an AppUser with `username_lower = "alice"`
- **AND** an admin in Org B creates an AppUser with `username = "Alice"`
- **THEN** the request succeeds — the unique index is scoped to `(org_id, username_lower)`

#### Scenario: Username is case-insensitive within an Org

- **WHEN** an admin in Org A attempts to create a second AppUser with `username = "ALICE"` while one with `username_lower = "alice"` already exists in Org A
- **THEN** the request is rejected with `USERNAME_TAKEN`

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

### Requirement: Admin can create an AppUser with a system-generated initial password

The system SHALL allow a dashboard `admin` to create an AppUser via `POST /app-users` with body `{ username, display_name }`. The system SHALL validate `username` against `^[a-zA-Z0-9_.-]{2,32}$` and `display_name` length 1–60. The system SHALL generate a fresh random initial password drawn from the alphabet `23456789ABCDEFGHJKLMNPQRSTUVWXYZ`, length 12. The response SHALL include the cleartext initial password exactly once (alongside the AppUser DTO). The server SHALL store only the bcrypt hash and SHALL set `needs_password_change = true`. The new AppUser SHALL belong to `current_org` (not transferable to another Org afterwards).

#### Scenario: Successful AppUser creation returns the cleartext initial password once

- **WHEN** an admin sends `POST /app-users` with `{ "username": "alice123", "display_name": "Alice Chen" }` and no existing AppUser in `current_org` has `username_lower = "alice123"`
- **THEN** a new `app_users` row is inserted with `org_id = current_org_id`, `username = "alice123"`, `username_lower = "alice123"`, `display_name = "Alice Chen"`, `status = active`, `needs_password_change = true`, `password_hash` set from the generated initial password, `last_login_at = null`
- **AND** the response is `201 Created` with body `{ user: { id, username, display_name, status, needs_password_change, last_login_at, created_at }, initial_password: "<12-char string>" }`

#### Scenario: Initial password format

- **WHEN** any AppUser is created (via initial create or password reset)
- **THEN** the cleartext initial password matches `^[2-9A-HJ-NP-Z]{12}$`

#### Scenario: Username format is enforced

- **WHEN** an admin sends `POST /app-users` with a `username` that does not match `^[a-zA-Z0-9_.-]{2,32}$`
- **THEN** the request is rejected with `INVALID_USERNAME_FORMAT`
- **AND** no row is inserted

#### Scenario: Duplicate username in same Org is rejected

- **WHEN** an admin sends `POST /app-users` with a `username` whose lowercase form already exists in `current_org`
- **THEN** the request is rejected with `USERNAME_TAKEN`

#### Scenario: Member cannot create AppUsers

- **WHEN** an authenticated dashboard user with role `member` sends `POST /app-users`
- **THEN** the request is rejected with `FORBIDDEN`

### Requirement: Admin can update an AppUser's display name or status

The system SHALL allow a dashboard `admin` to update an AppUser via `PATCH /app-users/:id` with body fields `display_name?` and/or `status?`. The endpoint SHALL only operate on AppUsers whose `org_id == current_org_id`; any other id SHALL respond `NOT_FOUND`. `username`, `org_id`, `password_hash`, `needs_password_change`, and `created_by_dashboard_user_id` SHALL NOT be settable through this endpoint.

#### Scenario: Admin updates display name

- **WHEN** an admin sends `PATCH /app-users/:id` with `{ "display_name": "Alice Wonderland" }` for an AppUser in `current_org`
- **THEN** `display_name` is updated
- **AND** other fields are unchanged
- **AND** `updated_at` is set to `now`

#### Scenario: Admin disables an AppUser

- **WHEN** an admin sends `PATCH /app-users/:id` with `{ "status": "disabled" }` for an active AppUser in `current_org`
- **THEN** the AppUser's `status` becomes `disabled`
- **AND** every `app_sessions` row referencing that `app_user_id` is deleted
- **AND** `updated_at` is set to `now`

#### Scenario: Admin re-enables an AppUser

- **WHEN** an admin sends `PATCH /app-users/:id` with `{ "status": "active" }` for a disabled AppUser
- **THEN** the AppUser's `status` becomes `active`
- **AND** `password_hash` and `needs_password_change` are unchanged (the user logs back in with their previous password)
- **AND** no new sessions are issued by this endpoint

#### Scenario: Cross-Org update rejected

- **WHEN** an admin sends `PATCH /app-users/:id` for an AppUser whose `org_id != current_org_id`
- **THEN** the request is rejected with `NOT_FOUND`

### Requirement: Admin can reset an AppUser's password

The system SHALL allow a dashboard `admin` to reset an AppUser's password via `POST /app-users/:id/password-reset`. The endpoint SHALL generate a new initial password using the same alphabet and length as initial creation, set `password_hash` accordingly, set `needs_password_change = true`, delete every `app_sessions` row referencing that `app_user_id`, and return the cleartext new initial password exactly once. The endpoint SHALL be admin-only and scoped to `current_org`.

#### Scenario: Password reset returns the cleartext password and forces re-login

- **WHEN** an admin sends `POST /app-users/:id/password-reset` for an AppUser in `current_org`
- **THEN** the AppUser's `password_hash` is replaced with the hash of a freshly generated 12-char password from `[2-9A-HJ-NP-Z]`
- **AND** `needs_password_change = true`
- **AND** all `app_sessions` rows for that AppUser are deleted
- **AND** the response is `200 OK` with body `{ user: AppUserDto, initial_password: "<12-char string>" }`

#### Scenario: Reset for cross-Org AppUser rejected

- **WHEN** an admin sends `POST /app-users/:id/password-reset` for an AppUser whose `org_id != current_org_id`
- **THEN** the request is rejected with `NOT_FOUND`

#### Scenario: Member cannot reset passwords

- **WHEN** a dashboard `member` sends `POST /app-users/:id/password-reset`
- **THEN** the request is rejected with `FORBIDDEN`

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

### Requirement: AppUser can fetch identity context

The system SHALL provide `GET /app/me` for authenticated AppUsers. The response SHALL be `{ user: AppUserDto, org: OrgDto, needs_password_change }`. This endpoint SHALL be reachable even when `needs_password_change == true`.

#### Scenario: Authenticated AppUser fetches their identity

- **WHEN** an authenticated AppUser sends `GET /app/me`
- **THEN** the response contains `{ user, org, needs_password_change }`
- **AND** `org` is the AppUser's `org` (1:1)

#### Scenario: Unauthenticated request rejected

- **WHEN** a client sends `GET /app/me` without a Bearer token, or with a token unknown to `app_sessions`
- **THEN** the response is `UNAUTHORIZED`

### Requirement: AppUser can change password

The system SHALL provide `POST /app/me/password` for authenticated AppUsers, accepting body `{ current_password, new_password }`. The system SHALL verify `current_password` against the stored hash; on mismatch SHALL respond `INVALID_PASSWORD`. The system SHALL validate `new_password` length ≥ 8 (matching dashboard policy). On success the system SHALL update `password_hash`, set `needs_password_change = false`, and SHALL NOT change `app_sessions` (the current token remains valid). This endpoint SHALL be reachable even when `needs_password_change == true`.

#### Scenario: Successful password change clears the forced flag

- **WHEN** an authenticated AppUser sends `POST /app/me/password` with the correct `current_password` and a `new_password` of length ≥ 8
- **THEN** `password_hash` is updated to hash of `new_password`
- **AND** `needs_password_change` becomes `false`
- **AND** existing `app_sessions` rows for that user are unchanged
- **AND** the response is `204 No Content`

#### Scenario: Wrong current password rejected

- **WHEN** an authenticated AppUser sends `POST /app/me/password` with a `current_password` that does not verify
- **THEN** the response is `INVALID_PASSWORD`
- **AND** `password_hash` and `needs_password_change` are unchanged

#### Scenario: Too-short new password rejected

- **WHEN** an authenticated AppUser sends `POST /app/me/password` with a `new_password` shorter than 8 characters
- **THEN** the response is `VALIDATION` (or equivalent length-violation error)
- **AND** `password_hash` is unchanged

### Requirement: AppUser can log out

The system SHALL provide `POST /app/auth/logout` for authenticated AppUsers. The action SHALL delete the `app_sessions` row matching the caller's Bearer token. The endpoint SHALL be reachable even when `needs_password_change == true`. The endpoint SHALL NOT affect any other `app_sessions` rows for the same user (multi-device sessions survive).

#### Scenario: Logout deletes only the current token

- **WHEN** an authenticated AppUser with two active `app_sessions` rows (e.g. phone + tablet) sends `POST /app/auth/logout` from the phone session
- **THEN** the phone's `app_sessions` row is deleted
- **AND** the tablet's `app_sessions` row is unaffected
- **AND** the response is `204 No Content`

### Requirement: Forced password change gates non-essential endpoints

The system SHALL, when an AppUser's `needs_password_change == true`, allow only `GET /app/me`, `POST /app/me/password`, and `POST /app/auth/logout` to proceed. Any other authenticated `/app/*` endpoint SHALL respond `423 LOCKED` with `error.code = NEEDS_PASSWORD_CHANGE`. Once `needs_password_change` is cleared, the gate is lifted.

#### Scenario: Gate blocks future `/app/*` endpoints

- **WHEN** an authenticated AppUser with `needs_password_change == true` sends a request to any future `/app/*` endpoint outside the allow-list
- **THEN** the response is `423 LOCKED` with `error.code = NEEDS_PASSWORD_CHANGE`

#### Scenario: Gate does not block the password-change endpoint itself

- **WHEN** the same AppUser sends `POST /app/me/password` while `needs_password_change == true`
- **THEN** the request proceeds (subject to its own validation)

#### Scenario: Gate is lifted after a successful change

- **WHEN** an AppUser successfully changes their password
- **AND** that AppUser subsequently sends a request to a previously gated `/app/*` endpoint
- **THEN** the request is no longer blocked by `NEEDS_PASSWORD_CHANGE`

### Requirement: Sessions expire and slide

The system SHALL set `app_sessions.expires_at = now + session_ttl` at issue time. Each authenticated request SHALL extend `expires_at` toward `now + session_ttl` (sliding refresh). Requests arriving after `expires_at` SHALL be treated as `UNAUTHORIZED` and the row SHALL be cleaned up by Mongo TTL on `expires_at`. The default `session_ttl` SHALL match the dashboard session TTL.

#### Scenario: Expired token is rejected

- **WHEN** a request arrives with a Bearer token whose `expires_at` is earlier than `now`
- **THEN** the request is treated as `UNAUTHORIZED`

#### Scenario: Sliding refresh extends a live session

- **WHEN** an authenticated request arrives with a Bearer token whose `expires_at` is in the future
- **THEN** the request is processed and `expires_at` is extended to `now + session_ttl`

### Requirement: Disabling an AppUser invalidates all of their sessions

The system SHALL, whenever an AppUser's `status` transitions from `active` to `disabled`, delete every `app_sessions` row whose `app_user_id` references that AppUser. The system SHALL NOT regenerate a password, change `needs_password_change`, or alter any other field as part of this operation.

#### Scenario: Disable kills all sessions

- **WHEN** an admin sends `PATCH /app-users/:id` setting `status = disabled` for an AppUser with two active `app_sessions` rows
- **THEN** both `app_sessions` rows are deleted
- **AND** subsequent requests with either token return `UNAUTHORIZED`

#### Scenario: Re-enable does not auto-issue sessions

- **WHEN** an admin re-enables a previously disabled AppUser
- **THEN** no new `app_sessions` row is created automatically
- **AND** the user must re-login with their (unchanged) password to obtain a new token

### Requirement: Disabled AppUser cannot log in

The system SHALL, on `POST /app/auth/login`, reject any login attempt whose resolved AppUser has `status == disabled` with the same generic `INVALID_CREDENTIALS` error used for other login failures. The system SHALL NOT distinguish disabled-account responses from wrong-password or unknown-user responses.

#### Scenario: Disabled AppUser receives the same error as wrong password

- **WHEN** a client sends `POST /app/auth/login` with credentials matching a disabled AppUser
- **AND** the password is correct
- **THEN** the response is `INVALID_CREDENTIALS` and indistinguishable from a wrong-password response

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

