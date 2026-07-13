## ADDED Requirements

### Requirement: Dashboard user can request a password reset link without revealing whether the email exists

The system SHALL provide `POST /auth/forgot-password` accepting `{ email }`, unauthenticated. The system SHALL respond `204 No Content` regardless of whether the email matches a `DashboardUser`, whether a reset was actually issued, or whether the send succeeded — the response SHALL NOT allow a caller to distinguish "email doesn't exist" from "email exists, reset link sent" from "email exists but rate-limited" from "email exists but the send failed". When the email matches a `DashboardUser` and the requesting user is not currently rate-limited (see the cooldown requirement below), the system SHALL generate a single-use reset token, persist a hash of it (never the raw token) alongside an expiry of 60 minutes from issuance, and send an email containing a link embedding the raw token.

#### Scenario: Existing email receives a reset link

- **WHEN** `POST /auth/forgot-password` is sent for an email matching a `DashboardUser`, outside the cooldown window
- **THEN** the response is `204`
- **AND** a password-reset token is persisted (hashed) with a 60-minute expiry
- **AND** an email is sent to that address containing a reset link

#### Scenario: Non-existent email produces an identical response

- **WHEN** `POST /auth/forgot-password` is sent for an email with no matching `DashboardUser`
- **THEN** the response is `204`, identical in shape to the existing-email case
- **AND** no token is created and no email is sent

### Requirement: Dashboard user can reset their password using a valid, unexpired, unused token

The system SHALL provide `POST /auth/reset-password` accepting `{ token, new_password }`, unauthenticated. The system SHALL look up the token by its hash, and reject with `INVALID_RESET_TOKEN` (400) if no matching record exists, the record has already been used, or its expiry has passed. `new_password` SHALL be validated with the same minimum-length rule used elsewhere in this codebase (>= 8 characters). On success the system SHALL: update the target `DashboardUser`'s `password_hash`; mark the token record as used so it cannot be replayed; and delete every existing `DashboardSession` for that user (forcing re-authentication on all devices). The system SHALL NOT issue a new session as part of this request — the caller is redirected to log in separately.

#### Scenario: Valid token resets the password and kills existing sessions

- **WHEN** `POST /auth/reset-password` is sent with a valid, unexpired, unused token and a `new_password` meeting the minimum length
- **THEN** the response is `204`
- **AND** the target user's `password_hash` is updated
- **AND** every existing `DashboardSession` for that user is deleted
- **AND** the same token is rejected as `INVALID_RESET_TOKEN` if submitted again

#### Scenario: Expired or already-used token is rejected

- **WHEN** `POST /auth/reset-password` is sent with a token that has expired or was already used
- **THEN** the response is `400 INVALID_RESET_TOKEN`
- **AND** no password is changed and no sessions are affected

#### Scenario: Unknown token is rejected identically to an expired one

- **WHEN** `POST /auth/reset-password` is sent with a token that does not match any stored record
- **THEN** the response is `400 INVALID_RESET_TOKEN`, indistinguishable from the expired/used case

### Requirement: Password-reset requests for the same user are rate-limited

The system SHALL reject — silently, from the caller's perspective (still returning `204` per the requirement above) — a `POST /auth/forgot-password` request for a `DashboardUser` who already had a reset token issued within the last 60 seconds. The system SHALL NOT create a new token or send a new email while a user is within this cooldown window.

#### Scenario: Repeated requests within the cooldown window do not issue additional tokens

- **WHEN** a second `POST /auth/forgot-password` is sent for the same email within 60 seconds of the first
- **THEN** the response is still `204`
- **AND** no additional token is created and no additional email is sent

#### Scenario: A request after the cooldown window issues a new token normally

- **WHEN** a `POST /auth/forgot-password` is sent for an email whose most recent token (if any) was issued more than 60 seconds ago
- **THEN** a new token is issued and an email is sent, following the normal flow
