## Why

The existing dashboard auth covers operators / admins, but the actual end-users of argus — the employees who will check in and out from the mobile app — have no representation yet. `app/` is just a `.gitkeep`, and the API has no concept of an AppUser. Before we can land checkin events, location tracking, or any of the downstream ROADMAP items, we need the model + admin tooling for managing AppUsers.

Doing it now (right after the m:n dashboard change) is the right moment: the membership model is fresh in everyone's head, the contrast between the two user kinds (DashboardUser m:n vs. AppUser 1:1) is easy to articulate, and we can pin down the auth surface for the app side without entangling Flutter bootstrap. Flutter scaffolding is intentionally a separate change (`add-app-shell`) so this change stays a clean backend + admin-web slice.

## What Changes

- New `app_users` collection: identity per `(org_id, username)`, holding `password_hash`, `display_name`, `status: active|disabled`, `needs_password_change: bool`, audit timestamps, and `created_by_dashboard_user_id`. AppUser is **1:1** with Org (no multi-Org membership for AppUsers in MVP — separate axis from DashboardUser m:n).
- New `app_sessions` collection: opaque random token, `app_user_id`, `expires_at`, `created_at`. Bearer transport via `Authorization: Bearer <token>`.
- New `/app/auth/*` and `/app/me/*` endpoints (mobile-facing):
  - `POST /app/auth/login` — body `{ org_code, username, password }`. `org_code` accepts random org code, active slug, or grace-period slug (same resolver as `register mode=join`). Issues a session and returns identity + Org + `needs_password_change` flag.
  - `POST /app/auth/logout` — kills only the current token.
  - `GET /app/me` — identity context.
  - `POST /app/me/password` — change password (works whether forced or voluntary).
- New `/app-users/*` endpoints under existing dashboard auth (admin-only, scoped to `current_org`):
  - `GET /app-users` — list AppUsers in current Org.
  - `POST /app-users` body `{ username, display_name }` — create with a server-generated initial password; response includes the cleartext password **once** for admin to share out-of-band.
  - `PATCH /app-users/:id` body `{ display_name?, status? }` — update profile / enable / disable.
  - `POST /app-users/:id/password-reset` — regenerate the initial password (one-time view), force `needs_password_change=true`, kill all of that AppUser's app_sessions.
- **First-login flow**: when `needs_password_change=true`, all `/app/*` endpoints except `GET /app/me`, `POST /app/me/password`, `POST /app/auth/logout` SHALL respond with HTTP 423 + `NEEDS_PASSWORD_CHANGE`. Successful `POST /app/me/password` clears the flag.
- **Soft-disable semantics**: setting `status=disabled` invalidates every `app_sessions` row for that AppUser and blocks future logins. `INVALID_CREDENTIALS` is returned generically (no leak between "wrong password" and "disabled"). Re-enable does not reset `needs_password_change` and does not change the password — the user logs back in with whatever they had.
- No cooldown on AppUser removal/disable. AppUser identifiers are admin-controlled, so the dashboard's anti-self-rejoin cooldown doesn't apply.
- Username uniqueness is scoped per Org: index `(org_id, lowercase(username))` unique. Same username may exist in different Orgs as different people.
- Password hashing reuses `auth::password::hash/verify` (bcrypt). Token format reuses the same opaque-random pattern as dashboard sessions.

Out of scope (separate ROADMAP items):

- Flutter app scaffold + actual UI (`add-app-shell`).
- Email-based password reset / forgot-password self-serve.
- External auth integration (LDAP / SSO / external DB) — soft-disable status field is the seed for it; full integration is a future change.
- AppUser self-registration via invite link.

## Capabilities

### New Capabilities

- `app-user-mgmt`: AppUser identity, admin CRUD, AppUser auth (login / logout / me / password change), forced-password-change first-login flow, soft-disable.

### Modified Capabilities

(none — this change introduces a new top-level capability and does not alter `dashboard-auth` or `org-tenancy`)

## Impact

- **Schema**: two new collections (`app_users`, `app_sessions`) with indexes; no changes to existing collections.
- **API code**: new module `api/src/handlers/app_auth.rs` (mobile-side) and `api/src/handlers/app_users.rs` (admin-side). New `api/src/db/app_users.rs` and `api/src/db/app_sessions.rs` repositories. New `api/src/auth/app_extractor.rs` extractor for Bearer-token auth on the mobile surface. `Db` and `AppState` extended.
- **API tests**: new `tests/app_*` integration tests covering login (3-field), logout, /me, password-change forced and voluntary, admin CRUD, soft-disable kicking sessions, /app-users RBAC (admin-only).
- **admin-web**: new page `pages/app-users/index.vue` (list + create + enable/disable + reset) reachable from existing admin-only nav (e.g. members or homepage card). Components for "view one-time generated password" modal. New `composables/useAppUsers.ts`. New types in `types/api.ts`.
- **Docs**: `api/README.md` and `admin-web/README.md` updated to mention the AppUser surface alongside the existing DashboardUser one. ROADMAP entry for `add-app-user-mgmt` is the prerequisite for the next checkin/tracking changes — once landed, that line moves to "delivered".
- **No Flutter changes** in this change. The contract here is what `add-app-shell` will consume next.
