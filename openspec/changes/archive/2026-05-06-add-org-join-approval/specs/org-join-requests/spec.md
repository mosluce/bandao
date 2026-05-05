## ADDED Requirements

### Requirement: Authenticated user can submit a join request to an existing Org

The system SHALL provide `POST /me/join-requests` accepting body `{ org_code: String, application_message?: String }` where `org_code` is the same identifier shape as `/me/memberships` (random `org_code`, active slug, or grace-period slug) and `application_message` is an optional ≤ 500-character free-text note. The system SHALL resolve the Org via the existing slug-auth resolver, enforce the cooldown rule from `dashboard-auth`, reject existing active memberships with `ALREADY_MEMBER`, reject existing pending requests with `JOIN_REQUEST_PENDING`, and on success insert a `join_requests` row with `(user_id=ctx.user_id, org_id, status="pending", application_message, requested_at=now)`. The endpoint SHALL NOT change `current_org_id`. `POST /me/memberships` (the older endpoint name) SHALL forward to this implementation for backward-compatibility callers.

#### Scenario: Successful join request

- **WHEN** an authenticated user sends `POST /me/join-requests` with a valid `org_code` and an optional 200-character message, having no membership and no pending request for that Org and no cooldown blocking
- **THEN** a `join_requests` row is inserted with `status="pending"`, `application_message` set, `requested_at=now`
- **AND** the response returns the new `JoinRequest` representation

#### Scenario: Application message length capped at 500

- **WHEN** an authenticated user sends `POST /me/join-requests` with `application_message` of 501+ characters
- **THEN** the request is rejected with `INVALID_INPUT`
- **AND** no row is created

#### Scenario: Already an active member rejected

- **WHEN** an authenticated user sends `POST /me/join-requests` for an Org they already have a `dashboard_memberships` row in
- **THEN** the request is rejected with `ALREADY_MEMBER`

#### Scenario: Duplicate pending request rejected

- **WHEN** an authenticated user sends `POST /me/join-requests` for an Org where they already have a pending `join_requests` row
- **THEN** the request is rejected with `JOIN_REQUEST_PENDING`

### Requirement: User lists their own join requests across all statuses

The system SHALL provide `GET /me/join-requests` returning the caller's `join_requests` rows in any status (pending / approved / rejected / cancelled), newest-first by `requested_at`. The response SHALL hydrate each row with the target Org's `name` and `code` so the UI can render the request without a second lookup. Approved requests MAY be omitted by client filter but SHALL be available in the unfiltered response.

#### Scenario: Lists pending and historical requests

- **WHEN** an authenticated user with one pending request and two historical (rejected, cancelled) requests sends `GET /me/join-requests`
- **THEN** the response is `200 OK` with all three items in newest-first order
- **AND** each item includes `org.name` and `org.code`

### Requirement: User can cancel their own pending join request

The system SHALL provide `DELETE /me/join-requests/:id` allowing the caller to cancel a pending request they own. Cancellation SHALL change the row's status to `cancelled` and set `decided_at=now` while leaving the row in place for audit. The endpoint SHALL reject if the row's `user_id != ctx.user_id` (return `404 NotFound`, not `403`, to avoid leaking the row's existence) or if the row's status is anything other than `pending` (return `400 InvalidState`).

#### Scenario: Cancel a pending request

- **WHEN** an authenticated user sends `DELETE /me/join-requests/:id` for a pending request they own
- **THEN** the request's `status` is updated to `cancelled` and `decided_at=now`
- **AND** the response is `204 No Content`

#### Scenario: Cancel someone else's request returns 404

- **WHEN** an authenticated user sends `DELETE /me/join-requests/:id` for a request whose `user_id` is a different user
- **THEN** the response is `404 NotFound`
- **AND** the row is unchanged

#### Scenario: Cancel an already-decided request rejected

- **WHEN** an authenticated user sends `DELETE /me/join-requests/:id` for one of their own requests with status `approved` / `rejected` / `cancelled`
- **THEN** the response is `400 InvalidState`
- **AND** the row is unchanged

### Requirement: Admin lists pending join requests for the current Org

The system SHALL provide `GET /orgs/me/join-requests?status=pending` accepting dashboard cookie auth and admin role. The endpoint SHALL return all `join_requests` rows where `org_id = ctx.current_org_id` filtered by the optional `status` query parameter (default `pending`), newest-first by `requested_at`. Each row SHALL be hydrated with the requester's `email` and any optional `application_message`. The endpoint SHALL respond `403` for non-admin callers.

#### Scenario: Admin lists pending requests

- **WHEN** an admin sends `GET /orgs/me/join-requests` with no `status` query
- **THEN** the response is `200 OK` listing all rows with `status="pending"` for `current_org_id`
- **AND** each row includes the requester's `email` and `application_message`

#### Scenario: Admin filters by status

- **WHEN** an admin sends `GET /orgs/me/join-requests?status=rejected`
- **THEN** the response includes only `status="rejected"` rows for `current_org_id`

#### Scenario: Member without admin role rejected

- **WHEN** a `member` (non-admin) sends `GET /orgs/me/join-requests`
- **THEN** the response is `403 FORBIDDEN`

### Requirement: Admin approves a pending join request and creates a membership atomically

The system SHALL provide `POST /orgs/me/join-requests/:id/approve` accepting dashboard cookie auth and admin role. The endpoint SHALL verify the `:id` row's `org_id == ctx.current_org_id` (else `404 NotFound`) and `status == pending` (else `400 InvalidState`). The endpoint SHALL re-run the cooldown check (`removed_memberships` for the requester's email + this Org); if blocked it SHALL return `EMAIL_IN_COOLDOWN` without changing state. On success the system SHALL atomically (within a single mongo transaction or a sequenced fallback that tolerates retry):

1. Update the `join_requests` row to `{ status="approved", decided_at=now, decided_by=ctx.user_id }`
2. Insert a `dashboard_memberships` row `(user_id, org_id, role="member", joined_at=now)`

If the membership insert fails because a membership row already exists (`ALREADY_MEMBER` race), the system SHALL still mark the request as `approved` (the user is effectively in) and return success. The endpoint SHALL respond `204 No Content` on success.

#### Scenario: Successful approve creates membership

- **WHEN** an admin sends `POST /orgs/me/join-requests/:id/approve` for a pending request in their Org with no cooldown active
- **THEN** the `join_requests` row's `status` becomes `approved`
- **AND** a `dashboard_memberships` row is created with `role="member"`
- **AND** the response is `204 No Content`

#### Scenario: Approve cross-org request returns 404

- **WHEN** an admin attempts to approve a request whose `org_id` is a different Org
- **THEN** the response is `404 NotFound`
- **AND** the row is unchanged

#### Scenario: Approve already-decided request rejected

- **WHEN** an admin attempts to approve a request whose `status` is `approved`, `rejected`, or `cancelled`
- **THEN** the response is `400 InvalidState`

#### Scenario: Approve blocked by late-arriving cooldown

- **WHEN** an admin attempts to approve a pending request and a `removed_memberships` marker became active for that email + Org since submission
- **THEN** the response is `EMAIL_IN_COOLDOWN`
- **AND** the request remains `pending`
- **AND** no membership is inserted

#### Scenario: Member without admin role rejected

- **WHEN** a `member` (non-admin) attempts to approve a request
- **THEN** the response is `403 FORBIDDEN`

### Requirement: Admin rejects a pending join request with optional reason

The system SHALL provide `POST /orgs/me/join-requests/:id/reject` accepting dashboard cookie auth and admin role and an optional body `{ rejection_reason?: String }` (≤ 500 characters). The endpoint SHALL verify the `:id` row's `org_id == ctx.current_org_id` (else `404 NotFound`) and `status == pending` (else `400 InvalidState`). On success the system SHALL update the row to `{ status="rejected", decided_at=now, decided_by=ctx.user_id, rejection_reason }`. The endpoint SHALL respond `204 No Content`. The system SHALL NOT create or modify any `dashboard_memberships` row.

#### Scenario: Successful reject with reason

- **WHEN** an admin sends `POST /orgs/me/join-requests/:id/reject` with `{ rejection_reason: "外部承包商不收" }` for a pending request in their Org
- **THEN** the `join_requests` row's `status` becomes `rejected` with `decided_at`, `decided_by`, and `rejection_reason` set
- **AND** the response is `204 No Content`

#### Scenario: Reject without reason

- **WHEN** an admin sends `POST /orgs/me/join-requests/:id/reject` with no body or `{}`
- **THEN** the row's `status` becomes `rejected` with `rejection_reason=null`

#### Scenario: rejection_reason length capped at 500

- **WHEN** an admin sends `POST /orgs/me/join-requests/:id/reject` with `rejection_reason` of 501+ characters
- **THEN** the response is `400 INVALID_INPUT`
- **AND** the row remains `pending`

### Requirement: Storage and uniqueness of join requests

The system SHALL persist join requests in a `join_requests` collection with `(user_id, org_id, status)` indexed and a partial unique index restricted to `status="pending"` enforcing at most one pending request per `(user_id, org_id)` pair. The collection SHALL retain rows in all four terminal states (`approved` / `rejected` / `cancelled`) for audit; rows SHALL NOT be deleted as part of normal flow.

#### Scenario: Partial unique index enforces single pending

- **WHEN** the database already contains a `join_requests` row `(u1, o1, status="pending")` and an attempt is made to insert another `(u1, o1, status="pending")`
- **THEN** the insert fails on the unique index

#### Scenario: Multiple non-pending rows for same pair allowed

- **WHEN** a user has previously been rejected (status="rejected") for Org A and now submits a fresh request
- **THEN** the new pending row is inserted successfully (the unique index covers only pending rows)
