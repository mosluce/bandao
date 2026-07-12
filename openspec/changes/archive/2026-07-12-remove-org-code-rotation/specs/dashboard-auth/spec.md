## MODIFIED Requirements

### Requirement: Org-scoped endpoints reject calls with no active Org

The system SHALL define a class of endpoints as **org-scoped** (any endpoint that needs `current_org_id` to operate, e.g. `/orgs/me/*`, `/dashboard-users/*` excluding `/me/*`). When such an endpoint is called by an authenticated user whose session has `current_org_id == null`, the request SHALL be rejected with `NO_ACTIVE_ORG` (HTTP 403). The system SHALL define the following endpoints as **org-agnostic** (callable with `current_org_id == null`): `GET /me`, `POST /me/orgs`, `POST /me/memberships`, `POST /me/current-org`, `POST /auth/logout`.

#### Scenario: Org-scoped endpoint rejected when current_org is null

- **WHEN** an authenticated user with `current_org_id == null` sends a request to any org-scoped endpoint (e.g. `POST /orgs/me/owner`, `GET /dashboard-users/cooldowns`)
- **THEN** the request is rejected with `NO_ACTIVE_ORG`
- **AND** no records are modified

#### Scenario: Org-agnostic endpoints succeed regardless of current_org

- **WHEN** an authenticated user with `current_org_id == null` sends a request to `GET /me`, `POST /me/orgs`, `POST /me/memberships`, `POST /me/current-org`, or `POST /auth/logout`
- **THEN** the request is processed normally (subject to its own validation and authentication rules)
