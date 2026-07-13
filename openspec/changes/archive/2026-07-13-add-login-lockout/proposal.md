## Why

Neither `/auth/login` (dashboard user) nor `/app/auth/login` (AppUser) has any limit on failed attempts today. Anyone who knows (or guesses) an email/username and an Org's identifier can brute-force a password with unlimited tries. This is the specific residual risk that `remove-org-code-rotation` accepted when it dropped slug rotation — that change was applied on the premise that a login-lockout mechanism would follow to close the gap (see `ROADMAP.md`).

## What Changes

- Track `failed_login_attempts` (u32) and `locked_until` (optional timestamp) on both `DashboardUser` and `AppUser` documents.
- After 3 consecutive failed attempts on one account, lock it for 1 hour. Both the threshold and duration are configurable via env vars (`LOGIN_LOCKOUT_THRESHOLD`, `LOGIN_LOCKOUT_DURATION_SECONDS`), defaulting to 3 / 3600.
- A successful login resets `failed_login_attempts` to 0 and clears `locked_until`.
- While locked, a login attempt SHALL NOT be checked against the password hash (avoids doing bcrypt work and avoids extending the lock via repeated attempts) and SHALL return the same response as a wrong-password attempt — `INVALID_CREDENTIALS` for dashboard users, the existing generic AppUser login failure — so the lockout mechanism does not introduce a new way to tell whether an account exists or is locked. No new error type or HTTP status is introduced.
- AppUsers whose Org has `auth_source == external_db` are exempt from lockout tracking: credential verification for those accounts happens against the customer's own database, not ours, so a local lockout would just add another way for `external-db-auth` failures to be misdiagnosed.
- Add an admin-only unlock action for each user type: `POST /dashboard-users/{id}/unlock`, `POST /app-users/{id}/unlock`. Both clear `failed_login_attempts` and `locked_until` and require the existing admin authorization used elsewhere in each capability.
- Admin-facing user list responses (`GET` for dashboard users' equivalent listing and `GET /app-users`) gain a computed `is_locked: bool` (`locked_until` in the future) so admin-web can conditionally render an "unlock" action. Raw attempt counts and the raw `locked_until` timestamp are not exposed to clients.
- Explicitly out of scope: per-IP throttling (noted as a future roadmap item, not this change).

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `dashboard-auth`: the "Dashboard user logs in with email and password" requirement gains lockout-after-N-failures behavior while preserving its existing non-disclosure guarantee; add an admin unlock endpoint.
- `app-user-mgmt`: the "AppUser logs in with org identifier, username, and password" requirement gains lockout-after-N-failures behavior for internal-auth accounts only; the AppUser list/DTO requirements gain an `is_locked` field; add an admin unlock endpoint.

## Impact

- `api/src/domain.rs` — add `failed_login_attempts` / `locked_until` to `DashboardUser` and `AppUser`.
- `api/src/db/dashboard_users.rs`, `api/src/db/app_users.rs` — atomic increment-and-maybe-lock on failure, reset-on-success, and unlock methods.
- `api/src/handlers/auth.rs` (dashboard login + new unlock endpoint), `api/src/handlers/app_auth.rs` (AppUser login), `api/src/handlers/users.rs` or equivalent (dashboard unlock endpoint, mirroring the existing `clear_cooldown` pattern).
- `api/src/config.rs` — two new configurable env vars with defaults.
- `admin-web/pages/members.vue`, `admin-web/pages/app-users/index.vue`, `admin-web/composables/useAppUsers.ts` (and a dashboard-users equivalent) — conditional unlock button driven by `is_locked`.
- No new MongoDB collection, no new `ApiError` variant, no in-memory/cache layer required.
