# org-tenancy Specification

## Purpose

Defines Org as the tenant boundary, including unique Org code generation, admin-controlled code rotation, code-based join authorization, and a forward-compatible settings container.

## Requirements

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

### Requirement: Org has an optional vanity slug

The system SHALL allow each Org to optionally hold a `slug` distinct from the random `org_code`. When set, `slug` SHALL match the regular expression `^[a-z0-9]{2,24}$` (lowercase ASCII letters and digits, 2 to 24 characters). The system SHALL normalize input to lowercase before validation and storage. The system SHALL guarantee that any active `slug` is globally unique across all Orgs and across all slugs currently held in grace period.

#### Scenario: Admin sets a slug for the first time

- **WHEN** an authenticated admin sends `POST /orgs/me/slug` with body `{ "slug": "acme" }`
- **AND** the Org currently has no slug and `slug_changed_at` is unset
- **AND** `acme` is not in the reserved list, not held by another Org's active slug, and not in any grace history
- **THEN** the Org record is updated with `slug = "acme"` and `slug_changed_at = now`
- **AND** the response is `200 OK` with `{ "slug": "acme" }`

#### Scenario: Admin updates an existing slug

- **WHEN** an authenticated admin sends `POST /orgs/me/slug` with body `{ "slug": "acmecorp" }`
- **AND** the Org currently has `slug = "acme"`
- **AND** `acmecorp` passes all validation
- **AND** the change is allowed by the rate-limit rules
- **THEN** the Org's slug becomes `"acmecorp"` and `slug_changed_at = now`
- **AND** the previous slug `"acme"` enters grace period (see Slug grace period requirement)

#### Scenario: Slug input is normalized to lowercase before validation

- **WHEN** an authenticated admin sends `POST /orgs/me/slug` with body `{ "slug": "ACME" }`
- **THEN** the system normalizes the input to `"acme"` and proceeds with validation
- **AND** if `"acme"` passes all checks, the stored slug is `"acme"`

#### Scenario: Slug fails format validation

- **WHEN** an authenticated admin sends `POST /orgs/me/slug` with a slug that does not match `^[a-z0-9]{2,24}$` after lowercase normalization (e.g. `"a"`, `"acme-corp"`, `"acme corp"`, 25-char string)
- **THEN** the request is rejected with `INVALID_SLUG_FORMAT`
- **AND** the Org record is unchanged

#### Scenario: Slug is in reserved list

- **WHEN** an authenticated admin sends `POST /orgs/me/slug` with a slug from the reserved list (e.g. `"admin"`, `"api"`, `"argus"`)
- **THEN** the request is rejected with `SLUG_RESERVED`
- **AND** the Org record is unchanged

#### Scenario: Slug already held by another Org as active slug

- **WHEN** an authenticated admin sends `POST /orgs/me/slug` with a slug that is currently the active slug of another Org
- **THEN** the request is rejected with `SLUG_TAKEN`
- **AND** the response does NOT disclose which Org holds the slug

#### Scenario: Slug held in grace period by another Org

- **WHEN** an authenticated admin sends `POST /orgs/me/slug` with a slug held in another Org's grace history with `expires_at > now`
- **THEN** the request is rejected with `SLUG_TAKEN`

#### Scenario: Member cannot set slug

- **WHEN** an authenticated user with role `member` sends `POST /orgs/me/slug`
- **THEN** the request is rejected with `FORBIDDEN`

#### Scenario: Admin clears the current slug

- **WHEN** an authenticated admin sends `DELETE /orgs/me/slug`
- **AND** the Org currently has `slug = "acme"`
- **AND** the change is allowed by the rate-limit rules
- **THEN** the Org's `slug` becomes `null`
- **AND** `"acme"` enters grace period
- **AND** `slug_changed_at` is updated to `now`

### Requirement: Slug grace period

When a slug is changed or cleared, the previous slug value SHALL enter a grace period of exactly 30 days from the moment of change. During the grace period the previous slug SHALL still resolve to the same Org for join purposes, AND no other Org SHALL be able to claim that slug. After the grace period expires the slug SHALL become free for any Org to claim.

#### Scenario: Old slug remains valid for join during grace period

- **WHEN** an Org's slug changed from `"acme"` to `"acmecorp"` 5 days ago
- **AND** a visitor registers with `mode=join`, valid email + password, and `org_code: "acme"`
- **THEN** the visitor is added as a `member` of the original Org
- **AND** the visitor receives a session cookie

#### Scenario: Old slug locked against other Orgs during grace

- **WHEN** Org A's slug changed from `"acme"` to `"acmecorp"` 5 days ago (i.e. `"acme"` is in grace under Org A)
- **AND** Org B's admin sends `POST /orgs/me/slug` with `{ "slug": "acme" }`
- **THEN** the request is rejected with `SLUG_TAKEN`

#### Scenario: Slug becomes free after grace expires

- **WHEN** Org A cleared slug `"acme"` 31 days ago and the grace history record's `expires_at` is in the past
- **AND** Org B's admin sends `POST /orgs/me/slug` with `{ "slug": "acme" }`
- **AND** all other validation passes
- **THEN** the slug `"acme"` is set on Org B

### Requirement: Slug change rate limit

The system SHALL allow an Org's first slug set to proceed without rate limit, but SHALL reject any subsequent set, change, or delete that occurs less than 30 days after the most recent slug change. The 30-day window SHALL be measured from `slug_changed_at`. Both `POST /orgs/me/slug` (set or change) and `DELETE /orgs/me/slug` (clear) SHALL update `slug_changed_at` and SHALL be subject to the limit (except the very first set).

#### Scenario: First-time slug set bypasses rate limit

- **WHEN** an Org has never had a slug (`slug_changed_at` is unset)
- **AND** an admin sends `POST /orgs/me/slug` with a valid input
- **THEN** the request succeeds regardless of timing

#### Scenario: Second slug change within 30 days rejected

- **WHEN** an Org's `slug_changed_at` was 10 days ago
- **AND** an admin sends `POST /orgs/me/slug` with another value
- **THEN** the request is rejected with `SLUG_CHANGE_TOO_SOON`
- **AND** the response body includes `retry_after` set to the absolute time when the next change becomes possible (`slug_changed_at + 30 days`)

#### Scenario: Delete within 30 days of last change rejected

- **WHEN** an Org's `slug_changed_at` was 5 days ago
- **AND** an admin sends `DELETE /orgs/me/slug`
- **THEN** the request is rejected with `SLUG_CHANGE_TOO_SOON`

#### Scenario: Set after 30+ days succeeds

- **WHEN** an Org's `slug_changed_at` was 31 days ago
- **AND** an admin sends `POST /orgs/me/slug` with a valid input
- **AND** all other validation passes
- **THEN** the request succeeds and `slug_changed_at` becomes `now`

### Requirement: Slug reserved word list

The system SHALL maintain a reserved word list and SHALL reject any slug that exactly matches a reserved word (after lowercase normalization). The reserved list SHALL include all first-level API path segments, common system identifiers (e.g. `admin`, `api`, `app`, `www`, `dashboard`, `login`, `register`, `logout`, `support`, `help`, `status`, `billing`, `settings`, `new`, `create`, `join`, `root`, `signup`, `signin`, `oauth`, `callback`), and the project name `argus`. The reserved list SHALL be a static, code-level constant maintained alongside the API; runtime modification is not supported.

#### Scenario: Reserved system word rejected

- **WHEN** an admin sends `POST /orgs/me/slug` with `{ "slug": "admin" }`
- **THEN** the request is rejected with `SLUG_RESERVED`

#### Scenario: Project name reserved

- **WHEN** an admin sends `POST /orgs/me/slug` with `{ "slug": "argus" }`
- **THEN** the request is rejected with `SLUG_RESERVED`

#### Scenario: API path segment reserved

- **WHEN** an admin sends `POST /orgs/me/slug` with `{ "slug": "auth" }` or any other first-level API path segment
- **THEN** the request is rejected with `SLUG_RESERVED`

### Requirement: Org code grants member access to the Org

The system SHALL accept a valid identifier as the sole authorization for joining an existing Org as `member` during registration. The identifier MAY be either the Org's random `org_code`, the Org's active `slug`, or any `slug` currently held in that Org's grace period. The system SHALL NOT require additional approval from existing admins for this MVP. The lookup SHALL be routed by input format: input matching `^[a-z0-9]{2,24}$` SHALL be searched first against active slugs, then against grace-history slugs with `expires_at > now`; input matching `^[2-9A-HJ-NP-Z]{10}$` SHALL be searched against `org_code`; any input matching neither format SHALL be rejected without database lookup.

#### Scenario: Valid code joins existing Org

- **WHEN** a visitor registers with `mode=join`, a valid email, password, and the current `org_code` of an existing Org
- **THEN** a `dashboard_user` record is created with `org_id` set to that Org and `role=member`
- **AND** the visitor receives a session cookie and is logged in

#### Scenario: Valid active slug joins existing Org

- **WHEN** a visitor registers with `mode=join`, a valid email, password, and an Org's current active `slug`
- **THEN** a `dashboard_user` record is created with `org_id` set to that Org and `role=member`
- **AND** the visitor receives a session cookie and is logged in

#### Scenario: Slug in grace period joins original Org

- **WHEN** a visitor registers with `mode=join`, a valid email, password, and a `slug` that an Org changed away from less than 30 days ago
- **THEN** a `dashboard_user` record is created with `org_id` set to the Org that previously held the slug, with `role=member`

#### Scenario: Invalid or stale identifier rejected

- **WHEN** a visitor registers with `mode=join` and an `org_code` that matches no current code, no active slug, and no in-grace slug
- **THEN** the request is rejected with `INVALID_ORG_CODE`
- **AND** no `dashboard_user` record is created

#### Scenario: Identifier not matching any known format rejected without lookup

- **WHEN** a visitor registers with `mode=join` and an `org_code` that does not match `^[a-z0-9]{2,24}$` nor `^[2-9A-HJ-NP-Z]{10}$`
- **THEN** the request is rejected with `INVALID_ORG_CODE` without consulting the database

### Requirement: Org has a settings container

Each Org SHALL have a `settings` object that future changes can extend with feature toggles (e.g. `transferEnabled`, `trackingEnabled`). The MVP `settings` MAY be empty but the field MUST be present.

#### Scenario: New Org has settings field

- **WHEN** a new Org is created
- **THEN** the Org record contains a `settings` object (may be empty `{}`)

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
