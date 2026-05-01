## ADDED Requirements

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

The system SHALL allow an admin to promote a `member` to `admin` or demote an `admin` to `member` within their own Org. Cross-Org role changes SHALL be rejected.

#### Scenario: Promote member to admin

- **WHEN** an authenticated admin sends `PATCH /dashboard-users/:id/role` with `{ role: "admin" }` for a member of the same Org
- **THEN** that user's `role` is updated to `admin`

#### Scenario: Cross-Org role change rejected

- **WHEN** an admin sends `PATCH /dashboard-users/:id/role` for a user belonging to a different Org
- **THEN** the request is rejected with `NOT_FOUND` (the target user is not visible to this admin)

### Requirement: Org always has at least one admin

The system SHALL refuse any operation that would leave an Org with zero admins.

#### Scenario: Demoting the last admin is rejected

- **WHEN** an admin attempts to demote themselves or another admin while they are the only admin of the Org
- **THEN** the request is rejected with `LAST_ADMIN`
- **AND** the role is unchanged
