## MODIFIED Requirements

### Requirement: Dashboard user logs in with email and password

The system SHALL authenticate a dashboard user by email and password. Each `dashboard_user` SHALL carry `failed_login_attempts` (integer, default 0) and `locked_until` (optional timestamp, default null). On success, a server-side session SHALL be created and a session cookie SHALL be returned, `failed_login_attempts` SHALL be reset to 0, and `locked_until` SHALL be cleared. On failure, the system MUST NOT disclose whether the email exists, whether the account is currently locked, or how many attempts remain before a lock — every failure path SHALL respond with the identical generic `INVALID_CREDENTIALS` error. When `locked_until` is in the future, the system SHALL reject the login with `INVALID_CREDENTIALS` without checking the password. When the account is not locked and the supplied password does not verify, the system SHALL atomically increment `failed_login_attempts`; when the resulting count reaches `LOGIN_LOCKOUT_THRESHOLD` (configurable, default 3), the system SHALL set `locked_until = now + LOGIN_LOCKOUT_DURATION_SECONDS` (configurable, default 3600 seconds). On successful login the system SHALL select a default `current_org_id` for the new session: the oldest Org the user owns (`org.owner_id == user._id`); failing that, the oldest membership (smallest `joined_at`); failing that, `null`.

#### Scenario: Successful login

- **WHEN** a visitor sends `POST /auth/login` with valid email and password
- **THEN** a new `dashboard_session` row is created with a random opaque token, `expires_at = now + 14d`, and `current_org_id` populated by the default-org rule
- **AND** the response sets `Set-Cookie` with the token (`HttpOnly; Secure; SameSite=Lax`) and returns 200 with `{ user, memberships, current_org }`

#### Scenario: Login fails for wrong password or unknown email

- **WHEN** a visitor sends `POST /auth/login` with an email that does not exist OR with a wrong password
- **THEN** the response is rejected with a generic `INVALID_CREDENTIALS` error
- **AND** the response body MUST NOT distinguish between the two cases

#### Scenario: Default current_org prefers the oldest owned Org

- **WHEN** a user with at least one Org they own logs in successfully
- **THEN** the new session's `current_org_id` is the `_id` of the Org with the smallest `created_at` among Orgs where `owner_id == user._id`

#### Scenario: Default current_org falls back to the oldest membership

- **WHEN** a user logs in successfully and owns no Orgs but has at least one membership
- **THEN** the new session's `current_org_id` is the `org_id` of the membership with the smallest `joined_at`

#### Scenario: Default current_org is null when the user has no memberships

- **WHEN** a user logs in successfully and has zero memberships
- **THEN** the new session's `current_org_id` is `null`
- **AND** the response body's `current_org` is `null`

#### Scenario: A failed login increments the attempt counter

- **WHEN** a visitor sends `POST /auth/login` with a correct email and an incorrect password, and the account is not currently locked
- **THEN** the account's `failed_login_attempts` is incremented by 1
- **AND** the response is `INVALID_CREDENTIALS`

#### Scenario: Reaching the threshold locks the account

- **WHEN** a visitor's failed login attempt brings `failed_login_attempts` to `LOGIN_LOCKOUT_THRESHOLD` (default 3)
- **THEN** `locked_until` is set to `now + LOGIN_LOCKOUT_DURATION_SECONDS` (default 1 hour)
- **AND** the response is `INVALID_CREDENTIALS`, identical in status and body to any other failed login

#### Scenario: Locked account rejects login without checking the password

- **WHEN** a visitor sends `POST /auth/login` for an account whose `locked_until` is in the future, regardless of whether the supplied password is correct
- **THEN** the response is `INVALID_CREDENTIALS`
- **AND** `failed_login_attempts` and `locked_until` are unchanged (the lock window is not extended by further attempts)

#### Scenario: Successful login resets the attempt counter

- **WHEN** a visitor with a nonzero `failed_login_attempts` (but not currently locked) sends `POST /auth/login` with the correct password
- **THEN** the login succeeds as normal
- **AND** `failed_login_attempts` is reset to 0 and `locked_until` is cleared

## ADDED Requirements

### Requirement: Admin can unlock a dashboard user's account

The system SHALL allow a user with role `admin` (in `current_org`) to manually clear a dashboard user's lockout state via `POST /dashboard-users/:id/unlock`. The endpoint SHALL only operate on a target that has a `dashboard_memberships` row for `current_org_id`; a target with no membership in `current_org` SHALL be rejected with `NOT_FOUND`. On success the system SHALL set `failed_login_attempts = 0` and `locked_until = null` on the target's `dashboard_user` document and respond `204 No Content`. Calling this endpoint on an account that is not currently locked SHALL be a no-op that still returns `204 No Content`. The DTO returned by `GET /dashboard-users` SHALL include a computed `is_locked: bool` field (`locked_until` in the future) for each user; the raw `failed_login_attempts` and `locked_until` values SHALL NOT be exposed in any API response.

#### Scenario: Admin unlocks a locked account

- **WHEN** an authenticated admin sends `POST /dashboard-users/:id/unlock` for a member of `current_org` whose `locked_until` is in the future
- **THEN** the target's `failed_login_attempts` is set to 0 and `locked_until` is set to `null`
- **AND** the target can immediately log in again with their correct password
- **AND** the response is `204 No Content`

#### Scenario: Unlocking an account that isn't locked is a no-op

- **WHEN** an authenticated admin sends `POST /dashboard-users/:id/unlock` for a member of `current_org` whose `locked_until` is already `null`
- **THEN** the response is `204 No Content`
- **AND** no error is raised

#### Scenario: Cross-Org unlock rejected

- **WHEN** an admin sends `POST /dashboard-users/:id/unlock` for a user who has no membership in `current_org`
- **THEN** the request is rejected with `NOT_FOUND`

#### Scenario: Member cannot unlock

- **WHEN** an authenticated user with role `member` sends `POST /dashboard-users/:id/unlock`
- **THEN** the request is rejected with `FORBIDDEN`

#### Scenario: Dashboard user list indicates lockout status

- **WHEN** an authenticated admin sends `GET /dashboard-users` and one listed user's `locked_until` is in the future
- **THEN** that user's entry has `is_locked = true`
- **AND** all other listed users have `is_locked = false`
- **AND** no entry exposes `failed_login_attempts` or the raw `locked_until` value
