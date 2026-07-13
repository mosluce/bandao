## 1. Config

- [x] 1.1 Add `login_lockout_threshold: u32` (env `LOGIN_LOCKOUT_THRESHOLD`, default `3`) and `login_lockout_duration: Duration` (env `LOGIN_LOCKOUT_DURATION_SECONDS`, default `3600`) to `Config` in `api/src/config.rs`, following the `session_ttl_secs` parsing pattern.

## 2. Domain and DB — dashboard users

- [x] 2.1 Add `failed_login_attempts: u32` (`#[serde(default)]`) and `locked_until: Option<bson::DateTime>` (`#[serde(default)]`) to `DashboardUser` in `api/src/domain.rs`.
- [x] 2.2 Add `DashboardUserRepository` methods: `record_failed_attempt(id) -> u32` (atomic `$inc` on `failed_login_attempts`, returns the new count) and `set_locked_until(id, until)`, `reset_lockout(id)` (sets `failed_login_attempts = 0, locked_until = null`), in `api/src/db/dashboard_users.rs`.

## 3. Domain and DB — AppUsers

- [x] 3.1 Add `failed_login_attempts: u32` (`#[serde(default)]`) and `locked_until: Option<bson::DateTime>` (`#[serde(default)]`) to `AppUser` in `api/src/domain.rs`.
- [x] 3.2 Add the same `record_failed_attempt`, `set_locked_until`, `reset_lockout` methods to `AppUserRepository` in `api/src/db/app_users.rs`.

## 4. Dashboard login flow

- [x] 4.1 In `login` (`api/src/handlers/auth.rs`), after `find_by_email` succeeds and before `password::verify`: if `locked_until` is in the future, return `ApiError::InvalidCredentials` immediately without verifying the password.
- [x] 4.2 On password mismatch when not locked: call `record_failed_attempt`; if the returned count `>= config.login_lockout_threshold`, call `set_locked_until(now + config.login_lockout_duration)`. Return `ApiError::InvalidCredentials` either way (same response for below-threshold and at-threshold failures).
- [x] 4.3 On successful password verification: call `reset_lockout` before issuing the session.

## 5. AppUser login flow

- [x] 5.1 In `login` (`api/src/handlers/app_auth.rs`), determine whether the resolved Org's `auth_source == internal` before applying any lockout check; skip lockout entirely for `external_db`.
- [x] 5.2 For `internal`-auth AppUsers, apply the same three steps as 4.1–4.3 (locked-check before verify, record-failure-and-maybe-lock on mismatch, reset-on-success) using the `AppUserRepository` methods from 3.2.
- [x] 5.3 Confirm the existing `disabled`/`unknown-account`/`unknown-org` paths remain untouched and still collapse to `INVALID_CREDENTIALS` / `EXTERNAL_AUTH_UNAVAILABLE` as before.

## 6. Admin unlock endpoints

- [x] 6.1 Add `POST /dashboard-users/{id}/unlock` handler in `api/src/handlers/users.rs` (or `auth.rs`, matching where `clear_cooldown` lives), `RequireAdmin`, scoped to `current_org` via the target's membership (mirror `update_role`/`remove`'s cross-Org `NOT_FOUND` check), calling `reset_lockout`, returning `204`.
- [x] 6.2 Add `POST /app-users/{id}/unlock` handler alongside the existing `app-users` admin handlers, `RequireAdmin`, scoped to `current_org` via `org_id`, calling `reset_lockout`, returning `204`.
- [x] 6.3 Register both routes in `api/src/handlers/mod.rs`.

## 7. API-visible lock status

- [x] 7.1 Add `is_locked: bool` (computed as `locked_until` in the future) to `DashboardUserDto` and populate it in `list_in_org` (`api/src/handlers/users.rs`).
- [x] 7.2 Add `is_locked: bool` to the AppUser DTO returned by `GET /app-users`, `false` unconditionally for external shadow users.
- [x] 7.3 Confirm `failed_login_attempts` and raw `locked_until` are never serialized into any DTO.

## 8. admin-web

- [x] 8.1 In `admin-web/pages/members.vue`: show an "unlock" action only when `is_locked`, calling `POST /dashboard-users/{id}/unlock` via the existing `api` helper, refreshing the list on success.
- [x] 8.2 In `admin-web/pages/app-users/index.vue` and `admin-web/composables/useAppUsers.ts`: add an `unlock` method/action alongside `disable`/`enable`/`resetPassword`, shown only when `is_locked`.

## 9. Tests

- [x] 9.1 Dashboard login: failed attempts increment; threshold locks; locked account returns `INVALID_CREDENTIALS` without checking password and without extending `locked_until`; successful login resets counters; admin unlock clears lockout; cross-Org unlock rejected `NOT_FOUND`; member unlock rejected `FORBIDDEN`.
- [x] 9.2 AppUser login: same matrix as 9.1 for `internal`-auth AppUsers, plus a case proving `external_db`-auth AppUsers accumulate no lockout state across repeated failures.
- [x] 9.3 `GET /dashboard-users` and `GET /app-users` include correct `is_locked` and omit raw counters.

## 10. Docs

- [x] 10.1 Remove the "登入失敗鎖定" entry from `ROADMAP.md` (superseded by this change).
