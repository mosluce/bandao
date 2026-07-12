## ADDED Requirements

### Requirement: External-auth configuration is only visible to dashboard admins

The system SHALL include the `external_auth` field in any API response only when the caller is resolved as a dashboard `admin` of the Org the configuration belongs to. Every other caller context — a dashboard `member`, an unauthenticated caller, or an AppUser (mobile) session — SHALL receive a response with the `external_auth` field entirely absent, not an empty object and not a partially-redacted one.

#### Scenario: Dashboard admin sees the configuration

- **WHEN** a dashboard `admin` of an Org with `auth_source == external_db` requests any endpoint that returns that Org as part of the response (e.g. `GET /me`, `POST /auth/login`)
- **THEN** the response's Org representation includes the `external_auth` field with the password-free configuration summary

#### Scenario: Dashboard member does not see the configuration

- **WHEN** a dashboard `member` of an Org with `auth_source == external_db` requests any endpoint that returns that Org as part of the response
- **THEN** the response's Org representation does NOT include an `external_auth` field at all

#### Scenario: AppUser session does not see the configuration

- **WHEN** an authenticated AppUser calls `POST /app/auth/login` or `GET /app/me` for an Org with `auth_source == external_db`
- **THEN** the response's Org representation does NOT include an `external_auth` field at all, regardless of the AppUser's own `auth_source`

#### Scenario: Endpoints already restricted to admin are unaffected

- **WHEN** a dashboard `admin` calls an endpoint that already requires the `admin` role to reach at all (e.g. `POST /orgs/me/external-auth`, `POST /orgs/me/owner`)
- **THEN** the response continues to include `external_auth` as before — this requirement changes visibility for callers who were never required to be admin, not for already-admin-gated endpoints
