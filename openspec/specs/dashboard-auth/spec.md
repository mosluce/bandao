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

The system SHALL allow an unauthenticated visitor to register with `mode=join`, providing email, password, and a valid join identifier (`org_code`, active slug, or grace-period slug). On success, a new `dashboard_user` identity SHALL be created and a `dashboard_memberships` row SHALL be inserted for `(user_id, org_id, role=member)`. The session issued SHALL have `current_org_id` set to that Org.

#### Scenario: Successful join-Org registration

- **WHEN** a visitor sends `POST /auth/register` with `{ mode: "join", email, password, org_code }` where the identifier resolves to an Org and email is not yet taken and no cooldown blocks the join
- **THEN** a `dashboard_user` identity is created
- **AND** a `dashboard_memberships` row is inserted with `(user_id, org_id, role=member, joined_at=now)`
- **AND** a `dashboard_session` is created with `current_org_id` set to that Org
- **AND** the response sets a session cookie and returns the user + memberships + current_org payload

### Requirement: Dashboard user logs in with email and password

The system SHALL authenticate a dashboard user by email and password. On success, a server-side session SHALL be created and a session cookie SHALL be returned. On failure, the system MUST NOT disclose whether the email exists. On successful login the system SHALL select a default `current_org_id` for the new session: the oldest Org the user owns (`org.owner_id == user._id`); failing that, the oldest membership (smallest `joined_at`); failing that, `null`.

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

The system SHALL, when creating any new membership for an existing-or-newly-created identity, look up `removed_memberships` by `(target_org.id, lowercase(input_email))` after resolving the Org but before inserting the membership. If a non-expired marker exists, the request SHALL be rejected with `EMAIL_IN_COOLDOWN`. The cooldown SHALL apply identically regardless of `removal_kind`. The cooldown SHALL be scoped to one Org: a cooldown for Org A SHALL NOT block joining Org B with the same email. The check SHALL apply to both `register mode=join` (creating identity + membership in one step) and `POST /me/memberships` (existing identity adding a new membership). The check SHALL NOT apply to `register mode=create` or `POST /me/orgs` (creating a brand-new Org cannot collide with a cooldown for that Org).

#### Scenario: Same email rejoining same Org during cooldown rejected via register

- **WHEN** a visitor sends `POST /auth/register` with `mode=join`, an `org_code` resolving to Org A, and an `email` that has a `removed_memberships` marker for Org A with `cooldown_until > now`
- **THEN** the request is rejected with `EMAIL_IN_COOLDOWN`
- **AND** no `dashboard_user` identity or membership is created

#### Scenario: Same email rejoining same Org during cooldown rejected via /me/memberships

- **WHEN** an authenticated user sends `POST /me/memberships` with an `org_code` resolving to Org A, and the user's email has a `removed_memberships` marker for Org A with `cooldown_until > now`
- **THEN** the request is rejected with `EMAIL_IN_COOLDOWN`
- **AND** no membership is created
- **AND** the user's other memberships and sessions are unaffected

#### Scenario: Same email joining different Org during cooldown succeeds

- **WHEN** any join request (register mode=join or /me/memberships) targets Org B and the only cooldown marker for that email is for Org A
- **THEN** the request proceeds (subject to all other validation)

#### Scenario: Cooldown lookup is case-insensitive

- **WHEN** any join request uses email `Bob@Example.com` and a marker exists for the same target Org with stored `email = "bob@example.com"` and `cooldown_until > now`
- **THEN** the request is rejected with `EMAIL_IN_COOLDOWN`

#### Scenario: Rejoin succeeds after cooldown elapses

- **WHEN** a marker's `cooldown_until` has already passed (or has been auto-purged)
- **AND** any join request targets the same email + Org
- **THEN** the request proceeds (subject to all other validation)

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

The system SHALL allow an authenticated dashboard user to join an existing Org without re-registering, via `POST /me/memberships` with `{ org_code }` (accepting the same identifier formats as `register mode=join`: random `org_code`, active slug, or grace-period slug). The system SHALL enforce the cooldown rule from "Cooldown blocks rejoin during membership creation". On success the system SHALL insert a `dashboard_memberships` row for `(ctx.user_id, target_org.id, role=member)` and update the caller's current session `current_org_id` to the joined Org. The endpoint SHALL reject duplicate memberships with `ALREADY_MEMBER`.

#### Scenario: Logged-in user joins an Org via org_code

- **WHEN** an authenticated user sends `POST /me/memberships` with a valid `org_code` and the user has no membership in that Org and no cooldown blocks the join
- **THEN** a `dashboard_memberships` row is inserted with `role=member`
- **AND** the caller's current session has `current_org_id` updated to the joined Org
- **AND** the response returns the joined Org and updated `/me` payload

#### Scenario: Logged-in user joins via active slug

- **WHEN** an authenticated user sends `POST /me/memberships` with the active slug of an Org
- **THEN** the same insert + session update behavior applies

#### Scenario: Logged-in user joins via grace-period slug

- **WHEN** an authenticated user sends `POST /me/memberships` with a slug currently held in another Org's grace history (`expires_at > now`)
- **THEN** the membership is created in the Org that previously held the slug

#### Scenario: Joining an Org the user is already a member of is rejected

- **WHEN** an authenticated user sends `POST /me/memberships` for an Org they already have a membership in
- **THEN** the request is rejected with `ALREADY_MEMBER`
- **AND** no new membership is inserted
- **AND** `current_org_id` is unchanged

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

- **WHEN** an authenticated user with `current_org_id == null` sends a request to any org-scoped endpoint (e.g. `POST /orgs/me/code/rotate`, `GET /dashboard-users/cooldowns`)
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

