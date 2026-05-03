## ADDED Requirements

### Requirement: Org has a permanent owner

Each Org SHALL hold an `owner_id: ObjectId` field referencing the `dashboard_user` who created the Org. The system SHALL set `owner_id` exactly once, during `register mode=create`, to the id of the freshly created user. The system SHALL NOT provide any endpoint to change `owner_id` in this MVP. The owner SHALL always have `role=admin`; this is enforced via the role-update rules in `dashboard-auth`. The owner SHALL be protected against removal and self-leave; this is enforced via the membership-lifecycle rules in `dashboard-auth`.

#### Scenario: New Org records its creator as owner

- **WHEN** a visitor successfully sends `POST /auth/register` with `{ mode: "create", email, password, org_name }`
- **THEN** the new Org's `owner_id` equals the id of the newly created `dashboard_user`
- **AND** the new `dashboard_user` has `role = admin`

#### Scenario: Owner persists across the Org lifetime

- **WHEN** any Org is loaded after creation
- **THEN** the loaded Org carries the same `owner_id` value it was created with
- **AND** no API endpoint allows changing this field

#### Scenario: Owner has no membership management impact during MVP role and slug operations

- **WHEN** an admin rotates the Org code, sets / clears the slug, or performs any other Org-level operation
- **THEN** `owner_id` is unchanged
