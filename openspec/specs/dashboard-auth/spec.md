# dashboard-auth Specification

## Purpose

Provides registration, login, session management, and role administration for dashboard users, including bootstrapping a new Org or joining an existing one.

## Requirements

### Requirement: Dashboard user can register and create a new Org

The system SHALL allow an unauthenticated visitor to register as a dashboard user with `mode=create`, providing email, password, and an Org name. On success, a new Org SHALL be created and the visitor SHALL be assigned `role=admin` of that Org.

#### Scenario: Successful create-Org registration

- **WHEN** a visitor sends `POST /auth/register` with `{ mode: "create", email, password, org_name }` and email is not yet taken
- **THEN** a new Org is created with the given `org_name` and a freshly generated `org_code`
- **AND** a `dashboard_user` record is created with `role=admin` and `org_id` pointing to the new Org
- **AND** the response sets a session cookie and returns the user + Org payload

#### Scenario: Registration rejected when email already exists

- **WHEN** a visitor sends `POST /auth/register` with an email that already exists in `dashboard_users`
- **THEN** the request is rejected with `EMAIL_TAKEN`
- **AND** no new Org or user is created

### Requirement: Dashboard user can register and join an existing Org

The system SHALL allow an unauthenticated visitor to register with `mode=join`, providing email, password, and a valid `org_code`. On success, the visitor SHALL be added to that Org with `role=member`.

#### Scenario: Successful join-Org registration

- **WHEN** a visitor sends `POST /auth/register` with `{ mode: "join", email, password, org_code }` where `org_code` matches a current Org code and email is not yet taken
- **THEN** a `dashboard_user` record is created with `role=member` and `org_id` pointing to that Org
- **AND** the response sets a session cookie and returns the user + Org payload

### Requirement: Dashboard user logs in with email and password

The system SHALL authenticate a dashboard user by email and password. On success, a server-side session SHALL be created and a session cookie SHALL be returned. On failure, the system MUST NOT disclose whether the email exists.

#### Scenario: Successful login

- **WHEN** a visitor sends `POST /auth/login` with valid email and password
- **THEN** a new `dashboard_session` row is created with a random opaque token and `expires_at = now + 14d`
- **AND** the response sets `Set-Cookie` with the token (`HttpOnly; Secure; SameSite=Lax`) and returns 200

#### Scenario: Login fails for wrong password or unknown email

- **WHEN** a visitor sends `POST /auth/login` with an email that does not exist OR with a wrong password
- **THEN** the response is rejected with a generic `INVALID_CREDENTIALS` error
- **AND** the response body MUST NOT distinguish between the two cases

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

The system SHALL provide a `GET /me` endpoint that returns the current dashboard user, their Org, and their role.

#### Scenario: Successful me lookup

- **WHEN** an authenticated user sends `GET /me`
- **THEN** the response contains `{ user: { id, email }, org: { id, name, code }, role }`

#### Scenario: Unauthenticated me lookup

- **WHEN** an unauthenticated visitor sends `GET /me`
- **THEN** the response is `UNAUTHORIZED`

### Requirement: Admin can change another user's role

The system SHALL allow an admin to promote a `member` to `admin` or demote an `admin` to `member` within their own Org. Cross-Org role changes SHALL be rejected. The system SHALL refuse to demote the Org owner: any attempt to set the role of the user whose id equals `Org.owner_id` to `member` SHALL be rejected with `OWNER_PROTECTED`, regardless of who initiates the request.

#### Scenario: Promote member to admin

- **WHEN** an authenticated admin sends `PATCH /dashboard-users/:id/role` with `{ role: "admin" }` for a member of the same Org
- **THEN** that user's `role` is updated to `admin`

#### Scenario: Cross-Org role change rejected

- **WHEN** an admin sends `PATCH /dashboard-users/:id/role` for a user belonging to a different Org
- **THEN** the request is rejected with `NOT_FOUND` (the target user is not visible to this admin)

#### Scenario: Owner cannot be demoted

- **WHEN** any admin (including the owner themselves) sends `PATCH /dashboard-users/:id/role` with `{ role: "member" }` where `id == Org.owner_id`
- **THEN** the request is rejected with `OWNER_PROTECTED`
- **AND** the role is unchanged

#### Scenario: Owner promotion is a no-op

- **WHEN** an admin sends `PATCH /dashboard-users/:id/role` with `{ role: "admin" }` where `id == Org.owner_id`
- **THEN** the request succeeds with the existing `admin` role unchanged (the owner is already admin by construction)

### Requirement: Admin can remove another dashboard user from the Org

The system SHALL allow a user with role `admin` to remove another dashboard user from the same Org via `DELETE /dashboard-users/:id`. The action SHALL hard delete the target `dashboard_user` record, hard delete every `dashboard_session` referencing that user, and write a `removed_memberships` marker for `(org_id, lowercase(email))` with `removal_kind = "kicked"`. The endpoint SHALL refuse to target the caller themselves; self-removal MUST go through `POST /me/leave`. The endpoint SHALL refuse to target the Org's owner.

#### Scenario: Admin removes a member

- **WHEN** an authenticated admin sends `DELETE /dashboard-users/:id` for a `member` of the same Org
- **THEN** the target's `dashboard_user` row is deleted
- **AND** every `dashboard_session` belonging to that user is deleted
- **AND** a `removed_memberships` marker is inserted with `org_id`, `email = lowercase(target.email)`, `removed_at = now`, `cooldown_until = now + 7 days`, `removal_kind = "kicked"`
- **AND** the response is `204 No Content`

#### Scenario: Admin removes another admin who is not the owner

- **WHEN** an authenticated admin sends `DELETE /dashboard-users/:id` for another `admin` of the same Org whose id ≠ `Org.owner_id`
- **THEN** the same delete + marker behavior applies
- **AND** the response is `204 No Content`

#### Scenario: Admin cannot remove the Org owner

- **WHEN** an authenticated admin sends `DELETE /dashboard-users/:id` where `id == Org.owner_id`
- **THEN** the request is rejected with `OWNER_PROTECTED`
- **AND** no records are modified

#### Scenario: Admin cannot remove themselves via this endpoint

- **WHEN** an authenticated admin sends `DELETE /dashboard-users/:id` where `id == ctx.user_id`
- **THEN** the request is rejected with `FORBIDDEN`
- **AND** no records are modified

#### Scenario: Member cannot remove anyone

- **WHEN** an authenticated user with role `member` sends `DELETE /dashboard-users/:id`
- **THEN** the request is rejected with `FORBIDDEN`

#### Scenario: Cross-Org removal rejected

- **WHEN** an admin sends `DELETE /dashboard-users/:id` for a user belonging to a different Org
- **THEN** the request is rejected with `NOT_FOUND`

### Requirement: Authenticated user can leave the Org

The system SHALL allow any authenticated dashboard user to leave their Org via `POST /me/leave`. The action SHALL hard delete the caller's `dashboard_user` record, hard delete every `dashboard_session` referencing that user (including the caller's current session), and write a `removed_memberships` marker for `(org_id, lowercase(email))` with `removal_kind = "left"`. The Org's owner SHALL NOT be allowed to self-leave.

#### Scenario: Successful self-leave

- **WHEN** an authenticated `member` or non-owner `admin` sends `POST /me/leave`
- **THEN** the caller's `dashboard_user` row is deleted
- **AND** every `dashboard_session` belonging to that user is deleted
- **AND** a `removed_memberships` marker is inserted with `org_id`, `email = lowercase(caller.email)`, `removed_at = now`, `cooldown_until = now + 7 days`, `removal_kind = "left"`
- **AND** the response clears the session cookie and returns `204 No Content`

#### Scenario: Owner cannot self-leave

- **WHEN** the Org owner sends `POST /me/leave`
- **THEN** the request is rejected with `OWNER_PROTECTED`
- **AND** no records are modified

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

The system SHALL, when processing `register mode=join`, look up `removed_memberships` by `(target_org.id, lowercase(input_email))` after resolving the Org but before creating the `dashboard_user`. If a non-expired marker exists, the request SHALL be rejected with `EMAIL_IN_COOLDOWN`. The cooldown SHALL apply identically regardless of `removal_kind`. The cooldown SHALL be scoped to one Org: a cooldown for Org A SHALL NOT block joining Org B with the same email.

#### Scenario: Same email rejoining same Org during cooldown rejected

- **WHEN** a visitor sends `POST /auth/register` with `mode=join`, an `org_code` resolving to Org A, and an `email` that has a `removed_memberships` marker for Org A with `cooldown_until > now`
- **THEN** the request is rejected with `EMAIL_IN_COOLDOWN`
- **AND** no `dashboard_user` is created

#### Scenario: Same email joining different Org during cooldown succeeds

- **WHEN** a visitor sends `POST /auth/register` with `mode=join`, an `org_code` resolving to Org B, and an `email` whose only cooldown marker is for Org A
- **THEN** the request proceeds (subject to all other validation)

#### Scenario: Cooldown lookup is case-insensitive

- **WHEN** a visitor registers with email `Bob@Example.com` and a marker exists for the same Org with stored `email = "bob@example.com"` and `cooldown_until > now`
- **THEN** the request is rejected with `EMAIL_IN_COOLDOWN`

#### Scenario: Rejoin succeeds after cooldown elapses

- **WHEN** a marker's `cooldown_until` has already passed (or has been auto-purged)
- **AND** a visitor registers with `mode=join` and the same email + Org
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
