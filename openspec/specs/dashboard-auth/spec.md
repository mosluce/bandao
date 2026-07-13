# dashboard-auth Specification

## Purpose

Provides registration, login, session management, and role administration for dashboard users, including bootstrapping a new Org or joining an existing one.
## Requirements
### Requirement: Dashboard user can register and create a new Org

The system SHALL allow an unauthenticated visitor to register as a dashboard user with `mode=create`, providing email, password, and an Org name. On success, a new `dashboard_user` identity SHALL be created, a new Org SHALL be created with the visitor as `owner_id`, and a `dashboard_memberships` row SHALL be inserted for `(user_id, org_id, role=admin)`. The session issued SHALL have `current_org_id` set to the newly created Org.

#### Scenario: Successful create-Org registration

- **WHEN** a visitor sends `POST /auth/register` with `{ mode: "create", email, password, org_name }` and email is not yet taken
- **THEN** a new Org is created with the given `org_name`, a freshly generated `org_code`, and `owner_id` set to the new user's id
- **AND** a `dashboard_user` identity is created (no `org_id` or `role` fields)
- **AND** a `dashboard_memberships` row is inserted with `(user_id, org_id, role=admin, joined_at=now)`
- **AND** a `dashboard_session` is created with `current_org_id` set to the new Org
- **AND** the response sets a session cookie and returns the user + memberships + current_org payload

#### Scenario: Registration rejected when email already exists

- **WHEN** a visitor sends `POST /auth/register` with an email that already exists in `dashboard_users`
- **THEN** the request is rejected with `EMAIL_TAKEN`
- **AND** no new identity, Org, or membership is created

### Requirement: Dashboard user can register and join an existing Org

The system SHALL allow an unauthenticated visitor to register with `mode=join`, providing email, password, and a valid join identifier (`org_code`, active slug, or grace-period slug). On success, a new `dashboard_user` identity SHALL be created and a `join_requests` row SHALL be inserted with `status=pending` for `(user_id, org_id)`. The system SHALL NOT create a `dashboard_memberships` row at registration time. The session issued SHALL have `current_org_id=null` (zero-Org state); the user becomes a member only after an admin of the target Org approves the request via `org-join-requests`. If `join_requests` insertion fails for any reason (cooldown, duplicate pending, db error), the system SHALL roll back the just-created `dashboard_user` so the registration call leaves no orphan identity.

#### Scenario: Successful join-Org registration creates a pending request

- **WHEN** a visitor sends `POST /auth/register` with `{ mode: "join", email, password, org_code }` where the identifier resolves to an Org and email is not yet taken and no cooldown blocks the join
- **THEN** a `dashboard_user` identity is created
- **AND** a `join_requests` row is inserted with `(user_id, org_id, status="pending", requested_at=now)`
- **AND** NO `dashboard_memberships` row is created at this point
- **AND** a `dashboard_session` is created with `current_org_id=null`
- **AND** the response sets a session cookie and returns the user + memberships=[] + current_org=null payload

#### Scenario: Cooldown blocks join-Org registration

- **WHEN** a visitor sends `POST /auth/register` with `mode=join` for an Org that has an active `removed_memberships` marker for the lower-cased email
- **THEN** the request is rejected with `EMAIL_IN_COOLDOWN` BEFORE any user creation
- **AND** no `dashboard_user`, `join_requests`, or `dashboard_memberships` row is created

#### Scenario: Failure after user creation rolls back the identity

- **WHEN** a visitor's `register mode=join` succeeds in creating the `dashboard_user` row but the subsequent `join_requests` insert fails (e.g., duplicate pending due to a parallel race)
- **THEN** the system deletes the just-created `dashboard_user`
- **AND** the response is the failure error code (e.g., `JOIN_REQUEST_PENDING`)

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

### Requirement: Dashboard user can request a password reset link without revealing whether the email exists

The system SHALL provide `POST /auth/forgot-password` accepting `{ email }`, unauthenticated. The system SHALL respond `204 No Content` regardless of whether the email matches a `DashboardUser`, whether a reset was actually issued, or whether the send succeeded — the response SHALL NOT allow a caller to distinguish "email doesn't exist" from "email exists, reset link sent" from "email exists but rate-limited" from "email exists but the send failed". When the email matches a `DashboardUser` and the requesting user is not currently rate-limited (see the cooldown requirement below), the system SHALL generate a single-use reset token, persist a hash of it (never the raw token) alongside an expiry of 60 minutes from issuance, and send an email containing a link embedding the raw token.

#### Scenario: Existing email receives a reset link

- **WHEN** `POST /auth/forgot-password` is sent for an email matching a `DashboardUser`, outside the cooldown window
- **THEN** the response is `204`
- **AND** a password-reset token is persisted (hashed) with a 60-minute expiry
- **AND** an email is sent to that address containing a reset link

#### Scenario: Non-existent email produces an identical response

- **WHEN** `POST /auth/forgot-password` is sent for an email with no matching `DashboardUser`
- **THEN** the response is `204`, identical in shape to the existing-email case
- **AND** no token is created and no email is sent

### Requirement: Dashboard user can reset their password using a valid, unexpired, unused token

The system SHALL provide `POST /auth/reset-password` accepting `{ token, new_password }`, unauthenticated. The system SHALL look up the token by its hash, and reject with `INVALID_RESET_TOKEN` (400) if no matching record exists, the record has already been used, or its expiry has passed. `new_password` SHALL be validated with the same minimum-length rule used elsewhere in this codebase (>= 8 characters). On success the system SHALL: update the target `DashboardUser`'s `password_hash`; mark the token record as used so it cannot be replayed; and delete every existing `DashboardSession` for that user (forcing re-authentication on all devices). The system SHALL NOT issue a new session as part of this request — the caller is redirected to log in separately.

#### Scenario: Valid token resets the password and kills existing sessions

- **WHEN** `POST /auth/reset-password` is sent with a valid, unexpired, unused token and a `new_password` meeting the minimum length
- **THEN** the response is `204`
- **AND** the target user's `password_hash` is updated
- **AND** every existing `DashboardSession` for that user is deleted
- **AND** the same token is rejected as `INVALID_RESET_TOKEN` if submitted again

#### Scenario: Expired or already-used token is rejected

- **WHEN** `POST /auth/reset-password` is sent with a token that has expired or was already used
- **THEN** the response is `400 INVALID_RESET_TOKEN`
- **AND** no password is changed and no sessions are affected

#### Scenario: Unknown token is rejected identically to an expired one

- **WHEN** `POST /auth/reset-password` is sent with a token that does not match any stored record
- **THEN** the response is `400 INVALID_RESET_TOKEN`, indistinguishable from the expired/used case

### Requirement: Password-reset requests for the same user are rate-limited

The system SHALL reject — silently, from the caller's perspective (still returning `204` per the requirement above) — a `POST /auth/forgot-password` request for a `DashboardUser` who already had a reset token issued within the last 60 seconds. The system SHALL NOT create a new token or send a new email while a user is within this cooldown window.

#### Scenario: Repeated requests within the cooldown window do not issue additional tokens

- **WHEN** a second `POST /auth/forgot-password` is sent for the same email within 60 seconds of the first
- **THEN** the response is still `204`
- **AND** no additional token is created and no additional email is sent

#### Scenario: A request after the cooldown window issues a new token normally

- **WHEN** a `POST /auth/forgot-password` is sent for an email whose most recent token (if any) was issued more than 60 seconds ago
- **THEN** a new token is issued and an email is sent, following the normal flow

### Requirement: Dashboard user can log out

The system SHALL allow an authenticated dashboard user to invalidate their current session.

#### Scenario: Successful logout

- **WHEN** an authenticated user sends `POST /auth/logout`
- **THEN** the corresponding `dashboard_session` row is deleted
- **AND** the response clears the session cookie

### Requirement: Sessions expire after inactivity

The system SHALL expire a session 14 days after its last activity. Each authenticated request MAY extend `expires_at` (sliding refresh). Expired sessions SHALL be rejected as unauthenticated.

#### Scenario: Expired session is rejected

- **WHEN** a request arrives with a session cookie whose `expires_at` is earlier than the current time
- **THEN** the request is treated as unauthenticated and the cookie is cleared
- **AND** authenticated endpoints respond with `UNAUTHORIZED`

### Requirement: Authenticated user can fetch their identity context

The system SHALL provide a `GET /me` endpoint that returns the current dashboard user's identity, the full list of memberships, and the currently selected Org context. When `current_org_id` is set, the response SHALL include the resolved `current_org` and `role`. When `current_org_id` is null (the user has no memberships, or the user has memberships but the session has no active selection), `current_org` and `role` SHALL be `null`.

#### Scenario: Successful me lookup with active org

- **WHEN** an authenticated user with `current_org_id` set sends `GET /me`
- **THEN** the response contains `{ user: { id, email }, memberships: [{ org, role }, ...], current_org: { id, name, code, ... }, role }` where `role` corresponds to the membership for `current_org_id`

#### Scenario: Successful me lookup with no active org

- **WHEN** an authenticated user with `current_org_id == null` sends `GET /me`
- **THEN** the response contains `{ user: { id, email }, memberships: [...], current_org: null, role: null }`
- **AND** `memberships` MAY be empty (zero-Org user) or non-empty (memberships exist but session has no selection)

#### Scenario: Unauthenticated me lookup

- **WHEN** an unauthenticated visitor sends `GET /me`
- **THEN** the response is `UNAUTHORIZED`

### Requirement: Admin can change another user's role

The system SHALL allow an admin (relative to `current_org`) to promote a `member` to `admin` or demote an `admin` to `member` within `current_org`. The change SHALL operate on the `dashboard_memberships` row matching `(target_user_id, current_org_id)`. Cross-Org role changes SHALL be rejected with `NOT_FOUND` (the target's membership for this Org is not visible). The system SHALL refuse to demote the Org owner: any attempt to set the role of the user whose id equals `Org.owner_id` to `member` SHALL be rejected with `OWNER_PROTECTED`, regardless of who initiates the request.

#### Scenario: Promote member to admin

- **WHEN** an authenticated admin sends `PATCH /dashboard-users/:id/role` with `{ role: "admin" }` for a member of `current_org`
- **THEN** that user's `dashboard_memberships` row for `current_org_id` is updated to `role=admin`

#### Scenario: Cross-Org role change rejected

- **WHEN** an admin sends `PATCH /dashboard-users/:id/role` for a user who has no membership in `current_org`
- **THEN** the request is rejected with `NOT_FOUND`

#### Scenario: Owner cannot be demoted

- **WHEN** any admin (including the owner themselves) sends `PATCH /dashboard-users/:id/role` with `{ role: "member" }` where `id == current_org.owner_id`
- **THEN** the request is rejected with `OWNER_PROTECTED`
- **AND** the membership is unchanged

#### Scenario: Owner promotion is a no-op

- **WHEN** an admin sends `PATCH /dashboard-users/:id/role` with `{ role: "admin" }` where `id == current_org.owner_id`
- **THEN** the request succeeds with the existing `admin` role unchanged (the owner is already admin by construction)

### Requirement: Admin can remove another dashboard user from the Org

The system SHALL allow a user with role `admin` (in `current_org`) to remove another dashboard user's membership from `current_org` via `DELETE /dashboard-users/:id`. The action SHALL hard delete the target's `dashboard_memberships` row for `(target_user_id, current_org_id)`, hard delete every `dashboard_sessions` row where `user_id == target_user_id AND current_org_id == current_org_id`, and write a `removed_memberships` marker for `(current_org_id, lowercase(target.email))` with `removal_kind = "kicked"`. The endpoint SHALL NOT delete the target's user identity, other memberships, or sessions pointing at other Orgs. The endpoint SHALL refuse to target the caller themselves; self-removal MUST go through `POST /me/leave`. The endpoint SHALL refuse to target the Org's owner.

#### Scenario: Admin removes a member

- **WHEN** an authenticated admin sends `DELETE /dashboard-users/:id` for a `member` of `current_org`
- **THEN** the target's `dashboard_memberships` row for `current_org_id` is deleted
- **AND** every `dashboard_sessions` row with `user_id == target.id AND current_org_id == current_org_id` is deleted
- **AND** the target's `dashboard_user` identity is preserved
- **AND** the target's memberships in other Orgs are preserved
- **AND** a `removed_memberships` marker is inserted with `org_id = current_org_id`, `email = lowercase(target.email)`, `removed_at = now`, `cooldown_until = now + 7 days`, `removal_kind = "kicked"`
- **AND** the response is `204 No Content`

#### Scenario: Admin removes another admin who is not the owner

- **WHEN** an authenticated admin sends `DELETE /dashboard-users/:id` for another `admin` of `current_org` whose id ≠ `current_org.owner_id`
- **THEN** the same membership delete + session scoped delete + marker behavior applies
- **AND** the response is `204 No Content`

#### Scenario: Admin cannot remove the Org owner

- **WHEN** an authenticated admin sends `DELETE /dashboard-users/:id` where `id == current_org.owner_id`
- **THEN** the request is rejected with `OWNER_PROTECTED`
- **AND** no records are modified

#### Scenario: Admin cannot remove themselves via this endpoint

- **WHEN** an authenticated admin sends `DELETE /dashboard-users/:id` where `id == ctx.user_id`
- **THEN** the request is rejected with `FORBIDDEN`
- **AND** no records are modified

#### Scenario: Member cannot remove anyone

- **WHEN** an authenticated user with role `member` (in `current_org`) sends `DELETE /dashboard-users/:id`
- **THEN** the request is rejected with `FORBIDDEN`

#### Scenario: Cross-Org removal rejected

- **WHEN** an admin sends `DELETE /dashboard-users/:id` for a user who has no membership in `current_org`
- **THEN** the request is rejected with `NOT_FOUND`

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

### Requirement: Authenticated user can leave the Org

The system SHALL allow any authenticated dashboard user to leave `current_org` via `POST /me/leave`. The action SHALL hard delete the caller's `dashboard_memberships` row for `(ctx.user_id, current_org_id)`, hard delete every `dashboard_sessions` row where `user_id == ctx.user_id AND current_org_id == current_org_id` (which includes the caller's current session), and write a `removed_memberships` marker for `(current_org_id, lowercase(caller.email))` with `removal_kind = "left"`. The endpoint SHALL NOT delete the caller's user identity, other memberships, or sessions pointing at other Orgs. The Org's owner SHALL NOT be allowed to self-leave; the owner must transfer ownership first.

#### Scenario: Successful self-leave

- **WHEN** an authenticated `member` or non-owner `admin` sends `POST /me/leave`
- **THEN** the caller's `dashboard_memberships` row for `current_org_id` is deleted
- **AND** every `dashboard_sessions` row with `user_id == ctx.user_id AND current_org_id == current_org_id` is deleted
- **AND** the caller's `dashboard_user` identity is preserved
- **AND** the caller's memberships in other Orgs are preserved
- **AND** sessions pointing at other Orgs are preserved
- **AND** a `removed_memberships` marker is inserted with `org_id = current_org_id`, `email = lowercase(caller.email)`, `removed_at = now`, `cooldown_until = now + 7 days`, `removal_kind = "left"`
- **AND** the response clears the session cookie and returns `204 No Content`

#### Scenario: Owner cannot self-leave

- **WHEN** the owner of `current_org` sends `POST /me/leave`
- **THEN** the request is rejected with `OWNER_PROTECTED`
- **AND** no records are modified
- **AND** the response message indicates that the owner must transfer ownership before leaving

### Requirement: Removed memberships hold a 7-day rejoin cooldown

The system SHALL, on every removal or self-leave, write exactly one `removed_memberships` marker pinned to `(org_id, lowercase(email))`. Each marker SHALL set `cooldown_until = removed_at + 7 days`. The system SHALL reject duplicate markers for the same `(org_id, email)` at the database level. The system SHALL automatically purge markers whose `cooldown_until` has passed, via a TTL index on `cooldown_until`.

#### Scenario: Marker fields are populated correctly

- **WHEN** any removal or self-leave action succeeds
- **THEN** the inserted `removed_memberships` document contains `org_id`, `email` (already lowercased), `removed_at = now`, `cooldown_until = removed_at + 7 days`, and `removal_kind` set to either `"kicked"` or `"left"`

#### Scenario: Email casing is normalized in markers

- **WHEN** a user with email `Alice@Example.com` is removed or leaves
- **THEN** the marker stores `email = "alice@example.com"`

#### Scenario: Expired markers are auto-purged

- **WHEN** a marker's `cooldown_until` is earlier than the current time
- **THEN** the document is removed from the collection by the TTL index without explicit application action

### Requirement: Cooldown blocks rejoin during `register mode=join`

The system SHALL, when creating any new `join_requests` row, look up `removed_memberships` by `(target_org.id, lowercase(email))` after resolving the Org but before inserting the request. If a non-expired marker exists, the request SHALL be rejected with `EMAIL_IN_COOLDOWN`. The cooldown SHALL apply identically regardless of `removal_kind`. The cooldown SHALL be scoped to one Org. The check SHALL apply to both `register mode=join` (creating identity + join_request in one step) and `POST /me/memberships` (existing identity adding a new join_request). The check SHALL NOT apply to `register mode=create` or `POST /me/orgs`. Additionally, when an admin approves a pending join_request, the system SHALL re-run the cooldown check before inserting the membership row, providing defense-in-depth against the rare race where a cooldown becomes active between request submission and approval.

#### Scenario: Same email rejoining same Org during cooldown rejected via register

- **WHEN** a visitor sends `POST /auth/register` with `mode=join`, an `org_code` resolving to Org A, and an `email` that has a `removed_memberships` marker for Org A with `cooldown_until > now`
- **THEN** the request is rejected with `EMAIL_IN_COOLDOWN`
- **AND** no `dashboard_user` identity, `join_requests` row, or membership is created

#### Scenario: Same email rejoining same Org during cooldown rejected via /me/memberships

- **WHEN** an authenticated user sends `POST /me/memberships` with an `org_code` resolving to Org A, and the user's email has a `removed_memberships` marker for Org A with `cooldown_until > now`
- **THEN** the request is rejected with `EMAIL_IN_COOLDOWN`
- **AND** no `join_requests` row is created
- **AND** the user's other memberships and sessions are unaffected

#### Scenario: Same email joining different Org during cooldown succeeds

- **WHEN** any join request (register mode=join or /me/memberships) targets Org B and the only cooldown marker for that email is for Org A
- **THEN** the request proceeds (subject to all other validation) and creates a pending `join_requests` row for Org B

#### Scenario: Cooldown lookup is case-insensitive

- **WHEN** any join request uses email `Bob@Example.com` and a marker exists for the same target Org with stored `email = "bob@example.com"` and `cooldown_until > now`
- **THEN** the request is rejected with `EMAIL_IN_COOLDOWN`

#### Scenario: Rejoin succeeds after cooldown elapses

- **WHEN** a marker's `cooldown_until` has already passed (or has been auto-purged)
- **AND** any join request targets the same email + Org
- **THEN** the request proceeds and creates a pending `join_requests` row

#### Scenario: Cooldown re-check at approve time blocks late-arriving cooldown

- **WHEN** an admin approves a pending `join_requests` row
- **AND** between submission and approval a `removed_memberships` marker for the same email + Org became active
- **THEN** the approve endpoint rejects with `EMAIL_IN_COOLDOWN`
- **AND** no membership is inserted
- **AND** the join_request's `status` remains `pending`

### Requirement: Admin can list and clear rejoin cooldown markers

The system SHALL allow a user with role `admin` to inspect and manually clear cooldown markers scoped to their own Org. `GET /dashboard-users/cooldowns` SHALL return all current `removed_memberships` markers belonging to the caller's Org. `DELETE /dashboard-users/cooldowns/:email` SHALL delete the marker matching `(caller.org_id, lowercase(:email))`. Both endpoints SHALL be admin-only.

#### Scenario: Admin lists cooldowns for their Org

- **WHEN** an authenticated admin sends `GET /dashboard-users/cooldowns`
- **THEN** the response contains an array of markers for the caller's Org, each including `email`, `removed_at`, `cooldown_until`, and `removal_kind`
- **AND** markers belonging to other Orgs are NOT included

#### Scenario: Admin clears a cooldown early

- **WHEN** an authenticated admin sends `DELETE /dashboard-users/cooldowns/:email` where a marker exists for `(caller.org_id, lowercase(:email))`
- **THEN** the marker is deleted
- **AND** a subsequent `register mode=join` with that email + Org is no longer blocked by `EMAIL_IN_COOLDOWN`
- **AND** the response is `204 No Content`

#### Scenario: Clearing a non-existent cooldown is a no-op

- **WHEN** an authenticated admin sends `DELETE /dashboard-users/cooldowns/:email` where no marker exists for `(caller.org_id, lowercase(:email))`
- **THEN** the response is `204 No Content`
- **AND** no error is raised

#### Scenario: Member cannot list or clear cooldowns

- **WHEN** an authenticated user with role `member` sends `GET /dashboard-users/cooldowns` or `DELETE /dashboard-users/cooldowns/:email`
- **THEN** the request is rejected with `FORBIDDEN`

### Requirement: Authenticated user can create a new Org

The system SHALL allow an authenticated dashboard user to create a new Org without re-registering, via `POST /me/orgs` with `{ org_name }`. On success the system SHALL create a new Org with the caller as `owner_id`, insert a `dashboard_memberships` row for `(ctx.user_id, new_org.id, role=admin)`, and update the caller's current session `current_org_id` to the new Org. The endpoint SHALL be available even when the caller has zero memberships (zero-Org state).

#### Scenario: Logged-in user creates a new Org

- **WHEN** an authenticated user sends `POST /me/orgs` with `{ org_name: "New Co" }`
- **THEN** a new Org is created with `owner_id = ctx.user_id` and a generated `org_code`
- **AND** a `dashboard_memberships` row is inserted with `(user_id=ctx.user_id, org_id=new_org.id, role=admin, joined_at=now)`
- **AND** the caller's current session has `current_org_id` updated to the new Org
- **AND** the response returns the new Org and updated `/me` payload

#### Scenario: Zero-Org user creates a new Org

- **WHEN** an authenticated user with `memberships.len() == 0` and `current_org_id = null` sends `POST /me/orgs` with valid input
- **THEN** the request succeeds (no `NO_ACTIVE_ORG` rejection — this endpoint is org-agnostic)
- **AND** the resulting `current_org_id` becomes the new Org

### Requirement: Authenticated user can join an existing Org

The system SHALL allow an authenticated dashboard user to join an existing Org via `POST /me/memberships` with `{ org_code }`. The endpoint SHALL accept the same identifier formats as `register mode=join` (random `org_code`, active slug, or grace-period slug). On success the system SHALL insert a `join_requests` row with `status=pending` for `(ctx.user_id, target_org.id)`. The system SHALL NOT create a `dashboard_memberships` row and SHALL NOT change `current_org_id`. The endpoint SHALL apply the cooldown rule (see "Cooldown blocks rejoin during membership creation"), reject existing active memberships with `ALREADY_MEMBER`, and reject duplicate pending requests with `JOIN_REQUEST_PENDING`.

#### Scenario: Logged-in user submits a join request via org_code

- **WHEN** an authenticated user sends `POST /me/memberships` with a valid `org_code` and the user has no membership in that Org, no pending request for that Org, and no cooldown blocks the join
- **THEN** a `join_requests` row is inserted with `status=pending`
- **AND** NO `dashboard_memberships` row is created
- **AND** the caller's session `current_org_id` is unchanged
- **AND** the response returns the unchanged `/me` payload

#### Scenario: Existing active membership rejected with ALREADY_MEMBER

- **WHEN** an authenticated user sends `POST /me/memberships` for an Org they already have a membership in
- **THEN** the request is rejected with `ALREADY_MEMBER`
- **AND** no `join_requests` row is created

#### Scenario: Duplicate pending request rejected with JOIN_REQUEST_PENDING

- **WHEN** an authenticated user sends `POST /me/memberships` for an Org where they already have a `join_requests` row with `status=pending`
- **THEN** the request is rejected with `JOIN_REQUEST_PENDING`
- **AND** no second `join_requests` row is created

#### Scenario: Invalid identifier rejected without lookup

- **WHEN** an authenticated user sends `POST /me/memberships` with an identifier matching neither `^[a-z0-9]{2,24}$` nor `^[2-9A-HJ-NP-Z]{10}$`
- **THEN** the request is rejected with `INVALID_ORG_CODE` without consulting the database

### Requirement: Authenticated user can switch the active Org

The system SHALL allow an authenticated dashboard user to switch which of their Orgs is `current_org` for the active session, via `POST /me/current-org` with `{ org_id }`. The target Org SHALL be one the user is currently a member of; otherwise the request SHALL be rejected with `NOT_A_MEMBER`. On success the caller's current session `current_org_id` is updated; other sessions of the same user are not affected.

#### Scenario: Switch to another Org the user is a member of

- **WHEN** an authenticated user with memberships in Orgs A and B and `current_org_id == A` sends `POST /me/current-org` with `{ org_id: B }`
- **THEN** the caller's current session `current_org_id` becomes B
- **AND** other sessions of the same user are unchanged
- **AND** the response returns the updated `/me` payload (with `current_org = B` and `role` reflecting the membership in B)

#### Scenario: Switch to an Org the user is not a member of is rejected

- **WHEN** an authenticated user sends `POST /me/current-org` with an `org_id` for which they have no membership
- **THEN** the request is rejected with `NOT_A_MEMBER`
- **AND** the session `current_org_id` is unchanged

#### Scenario: Switch to current_org is a no-op

- **WHEN** an authenticated user sends `POST /me/current-org` with `org_id == current_org_id`
- **THEN** the request succeeds with no state change

### Requirement: Org-scoped endpoints reject calls with no active Org

The system SHALL define a class of endpoints as **org-scoped** (any endpoint that needs `current_org_id` to operate, e.g. `/orgs/me/*`, `/dashboard-users/*` excluding `/me/*`). When such an endpoint is called by an authenticated user whose session has `current_org_id == null`, the request SHALL be rejected with `NO_ACTIVE_ORG` (HTTP 403). The system SHALL define the following endpoints as **org-agnostic** (callable with `current_org_id == null`): `GET /me`, `POST /me/orgs`, `POST /me/memberships`, `POST /me/current-org`, `POST /auth/logout`.

#### Scenario: Org-scoped endpoint rejected when current_org is null

- **WHEN** an authenticated user with `current_org_id == null` sends a request to any org-scoped endpoint (e.g. `POST /orgs/me/owner`, `GET /dashboard-users/cooldowns`)
- **THEN** the request is rejected with `NO_ACTIVE_ORG`
- **AND** no records are modified

#### Scenario: Org-agnostic endpoints succeed regardless of current_org

- **WHEN** an authenticated user with `current_org_id == null` sends a request to `GET /me`, `POST /me/orgs`, `POST /me/memberships`, `POST /me/current-org`, or `POST /auth/logout`
- **THEN** the request is processed normally (subject to its own validation and authentication rules)

### Requirement: Membership uniqueness

The system SHALL guarantee that for any pair `(user_id, org_id)` there is at most one `dashboard_memberships` row at all times. The collection SHALL enforce this with a unique index on `(user_id, org_id)`.

#### Scenario: Duplicate membership rejected at insert

- **WHEN** any code path attempts to insert a second `dashboard_memberships` row with the same `(user_id, org_id)` as an existing row
- **THEN** the database rejects the insert with a duplicate-key error
- **AND** the calling endpoint translates this to its appropriate user-facing error (e.g. `ALREADY_MEMBER` for `/me/memberships`)

### Requirement: Auth context resolves role per request

The system SHALL, for every authenticated request, resolve `role` by looking up `dashboard_memberships(ctx.user_id, ctx.current_org_id)` after looking up the session. The `role` SHALL NOT be cached on the session row. If the membership lookup fails for a non-null `current_org_id` (the user lost the membership while the session was alive — a race or stale state), the request SHALL be treated as `UNAUTHORIZED` and the session cookie SHALL be cleared.

#### Scenario: Role is read fresh from membership

- **WHEN** an admin's role for `current_org` was just changed from admin to member by another admin
- **AND** the user's next authenticated request arrives
- **THEN** the auth middleware resolves `role = member` based on the current membership row
- **AND** any admin-only endpoint in that request returns `FORBIDDEN`

#### Scenario: Stale session detected and cleared

- **WHEN** an authenticated request arrives with a session whose `current_org_id` has no matching `dashboard_memberships` row (and `current_org_id` is not null)
- **THEN** the request is rejected with `UNAUTHORIZED`
- **AND** the response clears the session cookie

