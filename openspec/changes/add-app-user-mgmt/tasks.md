## 1. Domain & DB schema

- [x] 1.1 Add `AppUser` and `AppUserStatus` to `api/src/domain.rs` with fields `id, org_id, username, username_lower, display_name, password_hash, status, needs_password_change, last_login_at, created_by_dashboard_user_id, created_at, updated_at`.
- [x] 1.2 Add `AppSession` to `api/src/domain.rs` with fields `token (_id), app_user_id, expires_at, created_at`.
- [x] 1.3 Create `api/src/db/app_users.rs` with `AppUserRepository`: `create`, `find_by_id`, `find_by_org_and_username_lower`, `list_by_org`, `update_profile`, `update_status`, `update_password`, `mark_password_changed`, `touch_last_login`. `create` returns the equivalent of `Duplicate` (mirroring the `MembershipInsertError` pattern) when the unique index rejects.
- [x] 1.4 Create `api/src/db/app_sessions.rs` with `AppSessionRepository`: `create`, `find_by_token`, `delete_by_token`, `delete_by_app_user`, `touch_expires`.
- [x] 1.5 In `api/src/db/mod.rs` create the two collections and indexes: unique `(org_id, username_lower)` + secondary `org_id` on `app_users`; TTL on `expires_at` for `app_sessions`. Wire both repos into the `Db` struct alongside the existing ones.

## 2. Helpers (auth + password generation)

- [x] 2.1 Add `api/src/auth/app_password.rs` with `generate_initial() -> String` returning a 12-char password from the alphabet `23456789ABCDEFGHJKLMNPQRSTUVWXYZ`.
- [x] 2.2 Reuse `api/src/auth/password::hash` / `verify` for AppUser bcrypt; do not introduce a parallel scheme.
- [x] 2.3 Reuse `api/src/auth/session_token::generate` for AppSession token bytes (refactor into a shared helper if its signature is too dashboard-specific; otherwise call as-is).
- [x] 2.4 Reuse `api/src/auth/slug::resolve_org_for_join` from the dashboard register flow when resolving `org_code` in `/app/auth/login`.

## 3. AppUser auth extractor & error variants

- [x] 3.1 Add `api/src/auth/app_extractor.rs` with `AppAuthContext { app_user_id, org_id, session_token, needs_password_change }` and a `RequireAppUser` extractor that:
  - reads `Authorization: Bearer <token>`,
  - looks up the session, expiry, and the AppUser,
  - rejects with `UNAUTHORIZED` when missing / expired / status != active,
  - returns `423 NEEDS_PASSWORD_CHANGE` when `needs_password_change == true` AND the route is not in the allow-list (`GET /app/me`, `POST /app/me/password`, `POST /app/auth/logout`),
  - extends `expires_at` (sliding refresh).
- [x] 3.2 Add `ApiError::UsernameTaken (409)`, `ApiError::InvalidUsernameFormat (400)`, `ApiError::NeedsPasswordChange (423)` variants in `api/src/error.rs` with appropriate codes (`USERNAME_TAKEN` / `INVALID_USERNAME_FORMAT` / `NEEDS_PASSWORD_CHANGE`).
- [x] 3.3 Decide on the gating mechanism for the `needs_password_change` allow-list: either a marker trait per route, an axum layer over the protected sub-router, or simply have each gated handler take `RequireAppUser` (which enforces) and the three exempt handlers take `AppAuthContext` (which does not). Pick the simplest that keeps the spec scenarios true.

## 4. Mobile-facing handlers (`/app/*`)

- [x] 4.1 `POST /app/auth/login` in `api/src/handlers/app_auth.rs`: validate body shape, resolve `org_code`, look up `app_users(org_id, username_lower)`, verify password, verify status, issue session, update `last_login_at`, return `{ token, expires_at, user, org, needs_password_change }`. All failure modes collapse to `INVALID_CREDENTIALS`.
- [x] 4.2 `POST /app/auth/logout`: delete session row by token; reachable even when `needs_password_change == true`.
- [x] 4.3 `GET /app/me`: return `{ user, org, needs_password_change }`; reachable even when `needs_password_change == true`.
- [x] 4.4 `POST /app/me/password`: verify `current_password`, validate `new_password >= 8`, update hash, set `needs_password_change = false`, leave sessions intact; reachable even when `needs_password_change == true`.
- [x] 4.5 Wire `/app/*` routes into `api/src/lib.rs` (or wherever the router is composed). Ensure they sit under a sub-router that runs the `RequireAppUser` middleware (or equivalent) — except `/app/auth/login`, which is public.

## 5. Admin-facing handlers (`/app-users`)

- [x] 5.1 `GET /app-users`: admin-only, scoped to `current_org`. Return list of `AppUserDto`. Excludes `password_hash` and any session info.
- [x] 5.2 `POST /app-users`: admin-only. Validate `username` against `^[a-zA-Z0-9_.-]{2,32}$` and `display_name` length 1–60. Generate initial password, create row with `needs_password_change = true`, return `{ user, initial_password }`. Map duplicate-key to `USERNAME_TAKEN`.
- [x] 5.3 `PATCH /app-users/:id`: admin-only, scoped to `current_org`. Accept partial update of `display_name?`, `status?`. Reject any other fields. When `status` transitions to `disabled`, delete all `app_sessions` for that AppUser. Cross-Org id → `NOT_FOUND`.
- [x] 5.4 `POST /app-users/:id/password-reset`: admin-only, scoped to `current_org`. Generate new initial password, update hash, set `needs_password_change = true`, delete all `app_sessions` for that AppUser, return `{ user, initial_password }`.
- [x] 5.5 Wire `/app-users/*` routes under existing dashboard auth + tenancy (cookie + `RequireAdmin`). Make sure NO_ACTIVE_ORG comes through naturally for sessions without `current_org_id`.

## 6. DTO shapes

- [x] 6.1 Define `AppUserDto { id, username, display_name, status, needs_password_change, last_login_at, created_at }` once and reuse from both `app_auth` and `app_users` handler modules.
- [x] 6.2 Define `AppLoginRequest { org_code, username, password }`, `AppLoginResponse { token, expires_at, user, org, needs_password_change }`, `AppMeResponse { user, org, needs_password_change }`, `AppPasswordChangeRequest { current_password, new_password }`.
- [x] 6.3 Define `CreateAppUserRequest { username, display_name }`, `UpdateAppUserRequest { display_name?, status? }`, `CreateAppUserResponse { user, initial_password }`.

## 7. API integration tests

- [x] 7.1 `tests/common/mod.rs` helper: `create_app_user(org_id, username, display_name, admin_session)` and `app_login(org_code, username, password)` builders. Reuse the existing TestApp infra and `testcontainers`.
- [x] 7.2 `tests/app_auth_login.rs`: happy path; unknown org_code → INVALID_CREDENTIALS; unknown username → INVALID_CREDENTIALS; wrong password → INVALID_CREDENTIALS; disabled → INVALID_CREDENTIALS; case-insensitive username match; slug + grace slug both work; `last_login_at` updated.
- [x] 7.3 `tests/app_auth_logout.rs`: logout deletes only the current token; sibling sessions on other devices survive; works with `needs_password_change=true`.
- [x] 7.4 `tests/app_me.rs`: returns `{ user, org, needs_password_change }`; works with `needs_password_change=true`; unknown token → 401.
- [x] 7.5 `tests/app_me_password.rs`: forced flow (initial password → change → flag cleared, token still valid); voluntary flow (already-active user changes password); wrong current_password → INVALID_PASSWORD; too-short new password → VALIDATION; works while `needs_password_change=true`.
- [x] 7.6 `tests/app_needs_password_change_gate.rs`: hit a gated `/app/*` endpoint with `needs_password_change=true` → 423 NEEDS_PASSWORD_CHANGE; hit GET /app/me / POST /app/me/password / POST /app/auth/logout → succeeds; after change, gated endpoints stop blocking. Use a placeholder gated route (or document that there's no second `/app/*` endpoint yet — assert via the extractor's behavior).
- [x] 7.7 `tests/app_users_list.rs`: admin lists current_org's AppUsers; member → FORBIDDEN; no current_org → NO_ACTIVE_ORG; cross-org rows excluded.
- [x] 7.8 `tests/app_users_create.rs`: happy path returns `initial_password` once; format `^[2-9A-HJ-NP-Z]{12}$`; member → FORBIDDEN; INVALID_USERNAME_FORMAT for bad shapes; USERNAME_TAKEN for case-insensitive duplicate; same username allowed in another Org.
- [x] 7.9 `tests/app_users_update.rs`: update display_name only; disable kills sessions; re-enable preserves password and needs_password_change; member → FORBIDDEN; cross-org → NOT_FOUND.
- [x] 7.10 `tests/app_users_password_reset.rs`: reset returns new `initial_password`, sets `needs_password_change=true`, kills all sessions, hash differs from prior; member → FORBIDDEN; cross-org → NOT_FOUND.

## 8. admin-web

- [x] 8.1 Add types to `admin-web/types/api.ts`: `AppUserDto`, `AppUserStatus`, `CreateAppUserRequest`, `UpdateAppUserRequest`, `CreateAppUserResponse`.
- [x] 8.2 Add `composables/useAppUsers.ts` exposing `list`, `create`, `update`, `disable`, `enable`, `resetPassword`, all of which 401-resilient via existing `useApi`.
- [x] 8.3 Add `pages/app-users/index.vue`: list with `username`, `display_name`, `status`, `last_login_at`, `created_at`. Watch `auth.currentOrg.value?.id` and refetch on switch. Admin-only — non-admin lands on `/`.
- [x] 8.4 Add "新增 App 使用者" button → modal with `username` + `display_name`. On success, show a one-time password modal with copy-to-clipboard + "知道了" dismiss. After dismiss, the cleartext is gone client-side.
- [x] 8.5 Add inline disable/enable toggle per row (with confirm for disable). Disabled rows render with reduced opacity + "已停用" badge.
- [x] 8.6 Add "重設密碼" action per row → confirmation → on success, reuse the one-time password modal.
- [x] 8.7 Add a header link from `pages/index.vue` (admin section) to `/app-users`. Add OrgSwitcher to `pages/app-users/index.vue` like other org-scoped pages.
- [x] 8.8 Friendly error messages for `USERNAME_TAKEN`, `INVALID_USERNAME_FORMAT`, `NO_ACTIVE_ORG`, `FORBIDDEN`.

## 9. Docs

- [x] 9.1 Update `api/README.md` with a new section describing the AppUser surface (`/app/auth/*`, `/app/me`, `/app-users/*`), including the new error codes (`USERNAME_TAKEN`, `INVALID_USERNAME_FORMAT`, `NEEDS_PASSWORD_CHANGE`).
- [x] 9.2 Update `admin-web/README.md` structure section to include `pages/app-users/`, `composables/useAppUsers`, and a brief note on the one-time-password modal pattern.
- [x] 9.3 Note in proposal-adjacent docs (or a short paragraph in `api/README.md`) that `add-app-shell` is the next ROADMAP item that consumes this surface.

## 10. Smoke

- [x] 10.1 Run cargo build / cargo test (per-binary serial loop on macOS to dodge TIME_WAIT noise documented in `api/README.md`).
- [x] 10.2 Run admin-web `pnpm typecheck` + `pnpm build`.
- [ ] 10.3 Live smoke: bring up local stack, log into admin-web, create an AppUser, confirm one-time password modal, disable + re-enable, reset password. Use `curl` to hit `/app/auth/login` with `(org_code, username, initial_password)` and confirm 200 + `needs_password_change=true`; hit `/app/me/password` with the change; confirm flag clears. (No Flutter required — the live smoke is API-level.)
