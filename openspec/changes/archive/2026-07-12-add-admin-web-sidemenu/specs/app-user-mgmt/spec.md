## MODIFIED Requirements

### Requirement: Any Org member can list AppUsers in current Org; only admin can manage them

The system SHALL allow any authenticated dashboard user with an active membership in `current_org` (`admin` or `member`) to list AppUsers within `current_org` via `GET /app-users`. The response SHALL contain an array of AppUser DTOs (each `{ id, auth_source, username, external_key, display_name, status, needs_password_change, last_login_at, created_at }`; `username` is present for internal users, `external_key` for external shadow users). The list SHALL include both internal AppUsers and external shadow AppUsers that have logged in at least once, scoped strictly to `current_org` — AppUsers from other Orgs SHALL NOT be returned. Creating, updating, or resetting the password of an AppUser SHALL remain restricted to `admin` — this requirement only changes read access.

#### Scenario: Admin lists AppUsers

- **WHEN** an authenticated admin sends `GET /app-users`
- **THEN** the response contains every AppUser whose `org_id == current_org_id`, including external shadow users
- **AND** AppUsers belonging to other Orgs are absent
- **AND** the response excludes `password_hash` and any session details

#### Scenario: External shadow users appear only after first login

- **WHEN** an Org uses `external_db` auth and an external user has never logged in
- **THEN** that user does not appear in `GET /app-users`
- **AND** after they log in once, they appear with `auth_source = external` and their resolved `external_key` and `display_name`

#### Scenario: Member can list AppUsers, identically to admin

- **WHEN** an authenticated dashboard user with role `member` sends `GET /app-users`
- **THEN** the response is `200 OK` with the same content a same-Org admin would receive

#### Scenario: Listing requires an active Org

- **WHEN** an authenticated dashboard user with `current_org_id == null` sends `GET /app-users`
- **THEN** the request is rejected with `NO_ACTIVE_ORG`
