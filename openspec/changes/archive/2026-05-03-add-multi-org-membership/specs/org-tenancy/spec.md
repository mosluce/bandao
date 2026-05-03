## MODIFIED Requirements

### Requirement: Org has a permanent owner

Each Org SHALL hold an `owner_id: ObjectId` field referencing the `dashboard_user` who is the current owner. The system SHALL set `owner_id` exactly once at creation, during `register mode=create` or `POST /me/orgs`, to the id of the user who created the Org. The system SHALL change `owner_id` only via the dedicated owner-transfer endpoint described under "Owner can transfer ownership"; no other endpoint SHALL be permitted to mutate this field. The owner SHALL always have an active `dashboard_memberships` row in this Org with `role=admin`; this is enforced via the role-update rules in `dashboard-auth`. The owner SHALL be protected against demotion, removal, and self-leave; this is enforced via the membership-lifecycle rules in `dashboard-auth`.

#### Scenario: New Org records its creator as owner

- **WHEN** a visitor successfully sends `POST /auth/register` with `{ mode: "create", email, password, org_name }`, OR an authenticated user successfully sends `POST /me/orgs` with `{ org_name }`
- **THEN** the new Org's `owner_id` equals the id of the creating user
- **AND** the creating user has a `dashboard_memberships` row for that Org with `role=admin`

#### Scenario: Owner persists across the Org lifetime when not transferred

- **WHEN** any Org is loaded after creation and no owner-transfer has been performed
- **THEN** the loaded Org carries the same `owner_id` value it was created with
- **AND** no endpoint other than `POST /orgs/me/owner` is allowed to change this field

#### Scenario: Owner-transfer is the only mutation path for owner_id

- **WHEN** any non-transfer endpoint (slug operations, code rotation, role changes, removals, self-leave, etc.) is invoked
- **THEN** `owner_id` is unchanged

## ADDED Requirements

### Requirement: Owner can transfer ownership

The system SHALL allow the current owner of an Org to transfer ownership to another user who is currently an admin of the same Org, via `POST /orgs/me/owner` with body `{ new_owner_user_id, current_password }`. The system SHALL verify the caller is the current owner, that `new_owner_user_id` differs from the caller, that the target has an active `dashboard_memberships` row in `current_org` with `role=admin`, and that `current_password` matches the caller's stored password hash. On success the system SHALL update `org.owner_id` to `new_owner_user_id` and update `org.updated_at`; both the previous and new owner SHALL retain `role=admin` in their respective membership rows. After transfer the previous owner SHALL no longer be protected by owner-protection rules (becomes a regular admin: demotable, removable, can self-leave), and the new owner SHALL be protected by them. The system SHALL NOT touch sessions, cooldowns, slug state, or any other Org field. There SHALL be no rate limit on transfers in this MVP.

#### Scenario: Successful ownership transfer

- **WHEN** the current owner sends `POST /orgs/me/owner` with `{ new_owner_user_id, current_password }` where the target is an admin of `current_org` and the password is correct
- **THEN** `org.owner_id` is updated to `new_owner_user_id`
- **AND** `org.updated_at` is updated to `now`
- **AND** both users' `dashboard_memberships` rows are unchanged (both remain `role=admin`)
- **AND** sessions, cooldowns, slug state, and other Org fields are untouched
- **AND** the response returns the updated Org payload

#### Scenario: Non-owner cannot transfer

- **WHEN** an admin who is not the current owner sends `POST /orgs/me/owner`
- **THEN** the request is rejected with `FORBIDDEN`
- **AND** no records are modified

#### Scenario: Member cannot transfer

- **WHEN** a user with `role=member` sends `POST /orgs/me/owner`
- **THEN** the request is rejected with `FORBIDDEN`

#### Scenario: Wrong password rejected

- **WHEN** the current owner sends `POST /orgs/me/owner` with a `current_password` that does not match the stored hash
- **THEN** the request is rejected with `INVALID_PASSWORD`
- **AND** no records are modified

#### Scenario: Target must be an admin in the same Org

- **WHEN** the current owner sends `POST /orgs/me/owner` with a `new_owner_user_id` who has no membership in `current_org`, OR has a membership but with `role=member`
- **THEN** the request is rejected with `INVALID_TARGET`
- **AND** no records are modified

#### Scenario: Self-transfer rejected

- **WHEN** the current owner sends `POST /orgs/me/owner` with `new_owner_user_id == ctx.user_id`
- **THEN** the request is rejected with `SAME_OWNER`
- **AND** no records are modified

#### Scenario: After transfer the previous owner can self-leave

- **WHEN** an ownership transfer has just succeeded, and the previous owner subsequently sends `POST /me/leave`
- **THEN** the request succeeds (the previous owner is no longer protected by owner-protection rules)

#### Scenario: After transfer the new owner is protected

- **WHEN** an ownership transfer has just succeeded, and any admin (including the previous owner) sends `DELETE /dashboard-users/:id` or `PATCH /dashboard-users/:id/role { "role": "member" }` targeting the new owner
- **THEN** the request is rejected with `OWNER_PROTECTED`
