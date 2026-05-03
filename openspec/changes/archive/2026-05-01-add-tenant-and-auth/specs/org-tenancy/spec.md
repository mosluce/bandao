## ADDED Requirements

### Requirement: Org has a unique Org code on creation

When an Org is created, the system SHALL generate a 10-character `org_code` drawn from the alphabet `23456789ABCDEFGHJKLMNPQRSTUVWXYZ` (no `0`, `O`, `1`, `I`, `L`). The system SHALL guarantee that `org_code` is globally unique across all Orgs at all times.

#### Scenario: New Org gets a generated code

- **WHEN** a new Org is successfully created (via dashboard user registration in `create` mode)
- **THEN** the Org record contains a 10-character `org_code` matching `^[2-9A-HJ-NP-Z]{10}$`
- **AND** no other Org in the system has the same `org_code`

#### Scenario: Code generation collision retries

- **WHEN** the random code generator returns a value that already exists for another Org
- **THEN** the system retries generation until a unique value is produced or the operation fails with a server error after 3 retries

### Requirement: Admin can rotate the Org code

The system SHALL allow a user with role `admin` to rotate the Org code of their own Org. After rotation, the previous code SHALL be invalid immediately and SHALL NOT be usable for joining.

#### Scenario: Admin rotates code successfully

- **WHEN** an authenticated admin sends `POST /orgs/me/code/rotate`
- **THEN** the response contains a new `org_code` different from the previous one
- **AND** the Org record is updated with the new code
- **AND** any subsequent registration attempt using the previous code is rejected with `INVALID_ORG_CODE`

#### Scenario: Member cannot rotate code

- **WHEN** an authenticated user with role `member` sends `POST /orgs/me/code/rotate`
- **THEN** the request is rejected with `FORBIDDEN`
- **AND** the Org code is unchanged

### Requirement: Org code grants member access to the Org

The system SHALL accept a valid `org_code` as the sole authorization for joining an existing Org as `member` during registration. The system SHALL NOT require additional approval from existing admins for this MVP.

#### Scenario: Valid code joins existing Org

- **WHEN** a visitor registers with `mode=join`, a valid email, password, and the current `org_code` of an existing Org
- **THEN** a `dashboard_user` record is created with `org_id` set to that Org and `role=member`
- **AND** the visitor receives a session cookie and is logged in

#### Scenario: Invalid or stale code rejected

- **WHEN** a visitor registers with `mode=join` and an `org_code` that does not match any current Org code
- **THEN** the request is rejected with `INVALID_ORG_CODE`
- **AND** no `dashboard_user` record is created

### Requirement: Org has a settings container

Each Org SHALL have a `settings` object that future changes can extend with feature toggles (e.g. `transferEnabled`, `trackingEnabled`). The MVP `settings` MAY be empty but the field MUST be present.

#### Scenario: New Org has settings field

- **WHEN** a new Org is created
- **THEN** the Org record contains a `settings` object (may be empty `{}`)
