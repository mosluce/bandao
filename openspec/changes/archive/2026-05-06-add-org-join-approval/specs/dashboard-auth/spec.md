## MODIFIED Requirements

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
