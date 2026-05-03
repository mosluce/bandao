## Context

The dashboard surface (operator/admin tooling) is mature: `dashboard_users` identity, m:n `dashboard_memberships`, cookie-based sessions, `current_org_id` switchable per session, role enforced by middleware. The mobile-end-user surface is unimplemented — `app/` holds only `.gitkeep`, and the API has no handlers under any `/app/*` prefix.

This change introduces the AppUser axis: a separate identity model, a separate auth mechanism, and admin tooling on the dashboard side to manage them. We deliberately keep AppUser **1:1** with Org (each AppUser belongs to exactly one Org, immutable) — different from DashboardUser's m:n. Reasoning: an employee's relationship to a workplace is a singular professional identity, not a community membership. Multi-Org for AppUsers can be revisited later if a real "派遣 / 兼職" use case appears, but the data model would be a different shape than DashboardUser memberships (probably per-pair employment record with start/end dates), so we explicitly don't generalize the membership pattern across the two axes.

We are pre-launch with no AppUser data anywhere. Migration story is "create the new collections and indexes; existing data unaffected". Flutter scaffolding is intentionally out of scope so the change stays manageable; the API surface here is the contract `add-app-shell` will consume.

## Goals / Non-Goals

**Goals:**

- Define `app_users` and `app_sessions` clearly enough that `add-app-shell` can build against the API contract without further design work.
- Make AppUser auth feel parallel to dashboard auth conceptually (opaque random token, sliding refresh, server-side session row) but use Bearer transport for native-mobile ergonomics.
- Force first-login password change so admin-issued initial passwords cannot persist as the working credential.
- Give the admin a clear, finite ceremony: create AppUser → see initial password once → tell user OOB → never see it again. Same ceremony for "reset password".
- Keep the identifier story org-scoped: same username at OrgA and OrgB are different people.
- Leave room for future external auth (LDAP/SSO) by keeping `status` as the gating field, decoupled from the local password.

**Non-Goals:**

- Flutter app scaffold or any UI in `app/` (separate `add-app-shell` change).
- Email-based password reset / forgot-password self-serve (no email infra; future change).
- External auth integration (`auth_method` field, LDAP/SSO connectors). The `status` field is the foundational seed — full external-auth handling is a future change.
- AppUser cooldown on remove/disable (not needed since AppUser onboarding is admin-driven, no self-serve to abuse).
- Multi-Org AppUser. Revisit only if a concrete dispatch / freelance use case lands.
- AppUser-side device management ("see all my logged-in devices and revoke") beyond the basic `app_sessions` row. The schema leaves room (`device_id` placeholder discussed but not added in MVP — keep minimal).
- Rate limiting on `/app/auth/login`. Should be added later for both surfaces together.

## Decisions

### Why a separate `app_users` collection (not a flag on `dashboard_users`)

DashboardUser and AppUser have different shapes (DashboardUser has m:n memberships and a `current_org_id` per session; AppUser is 1:1 and has no notion of "current org"). They have different auth surfaces (cookie vs Bearer). They have different lifecycle gates (DashboardUser self-registers; AppUser is admin-created). Forcing one collection with a discriminator would require half the columns to be `Optional`, scatter conditionals across handlers, and entangle two unrelated mental models. Two collections keep the abstractions clean.

Same email being a DashboardUser AND an AppUser is fine — they're independent records and that's a feature, not a bug.

### Auth transport: Bearer header, not cookie

Cookies are awkward in native mobile environments (cookie store across app restarts, no shared cookie jar with browser, CSRF semantics that don't apply to non-browser clients). Native apps universally use `Authorization: Bearer <token>`. We mirror that.

The token itself is the same primitive as dashboard sessions: a random opaque base64 string (`~32 bytes`, ≥256 bits entropy). Server stores `app_sessions(_id=token, app_user_id, expires_at, created_at)`. Sliding refresh extends `expires_at` on every authenticated request; expired rows return UNAUTHORIZED. Logout deletes the row by token. **No JWT** — JWT's stateless verification is irrelevant here (we always have a DB) and complicates revocation.

### Login takes three fields: `org_code` + `username` + `password`

Mobile users belong to one Org. The first-time login form needs to namespace by Org so the server can `find_one(app_users, { org_id, username })`. We accept the same `org_code` shapes as `register mode=join` (random 10-char code, active slug, grace-period slug) using the existing `slug_auth::resolve_org_for_join` helper.

UX optimization (handled later by `add-app-shell`): Flutter remembers `org_id` after first successful login so subsequent sessions only require username + password. The wire contract is always 3 fields.

### Soft-disable, not hard-delete

Future: an AppUser is the FK target for checkin records, location traces, etc. Hard-deleting would orphan all that history. Soft-disable preserves the row + audit history while:
- denying new logins (`/app/auth/login` returns `INVALID_CREDENTIALS` — generic on purpose, no info leak),
- killing all existing tokens (`app_sessions` rows for that user are deleted),
- still letting the admin see them in the AppUser list with a "disabled" badge.

Re-enable just flips `status=active`; the user's old password is intact (since we never touched `password_hash`), and `needs_password_change` is whatever it was before. They log in normally.

### Initial password ceremony

Initial password is generated by the server, shown to the admin once in the response, and never retrievable again. We use the same alphabet as `org_code` (`23456789ABCDEFGHJKLMNPQRSTUVWXYZ`) for two reasons:
- no confusable characters (`0/O`, `1/I/L`),
- consistency with the project's existing identifier style.

Length 12 → ~60 bits of entropy. Strong enough to resist offline attack on the bcrypt hash for the brief window before the user changes it; readable enough for admins to type or dictate over the phone if needed.

The admin-web UI shows the password in a one-time modal with a copy button. Once the admin closes the modal, it's gone. (Server has only the bcrypt hash from that point onward — the cleartext is dropped after the response is sent.)

### Forced password change uses HTTP 423 + dedicated error code

When `needs_password_change=true`, every `/app/*` endpoint EXCEPT `GET /app/me`, `POST /app/me/password`, `POST /app/auth/logout` returns `423 LOCKED` with `error.code = NEEDS_PASSWORD_CHANGE`. The Flutter app reads this and routes the user to the change-password screen.

We use 423 (not 403) because the request is well-authenticated and well-formed; the resource is just "locked" pending a state change by the user. Standard HTTP-vocabulary fit. `403 Forbidden` would imply "you don't have permission" which is wrong.

### Username format and uniqueness

Allowed: `^[a-zA-Z0-9_.-]{2,32}$`. Lowercase comparison for uniqueness so `Alice` and `alice` collide. Original casing is preserved for display.

Why looser than `org_code` (`^[2-9A-HJ-NP-Z]{10}$`) or `slug` (`^[a-z0-9]{2,24}$`): username is human-controlled and human-displayed. Letting people use dots, dashes, underscores covers `firstname.lastname`, `firstname-lastname`, `f_lastname` patterns naturally. Not allowing `@` keeps it visually distinct from email.

Unique index: `(org_id, lowercase_username)` where `lowercase_username` is a denormalized field (or computed at query time via case-insensitive collation). MVP: store both `username` (raw) and `username_lower` (lowercased), index on `(org_id, username_lower)`.

### `display_name` is required

If we let it be optional and fall back to username, the admin list reads as a wall of usernames and admins lose context fast. Required at create, editable later. 1–60 chars, no character-class restriction (Unicode — Chinese names, accented Latin, etc. all fine).

### Session TTL: same as dashboard (14 days, sliding refresh)

No reason to diverge. If app-side feedback shows mobile users want longer-lived sessions to reduce re-login friction, bump it together for both surfaces in a future change.

### `last_login_at`

Updated on successful `/app/auth/login`. Not on every authenticated request (would spam writes). Useful signal for admins to identify dormant accounts. Optional UX in admin list.

### Routing convention

```
/app/auth/login          POST   public
/app/auth/logout         POST   Bearer
/app/me                  GET    Bearer
/app/me/password         POST   Bearer (allowed when needs_password_change=true)

/app-users               GET    dashboard cookie + admin
/app-users               POST   dashboard cookie + admin
/app-users/:id           PATCH  dashboard cookie + admin
/app-users/:id/password-reset   POST   dashboard cookie + admin
```

The `/app/*` prefix unambiguously marks the mobile-facing surface (different auth, different tenancy assumptions, different token handling). The `/app-users` admin endpoints stay outside `/app/*` because they live in dashboard's auth + tenancy world (act on behalf of `current_org`).

### Where the AppUser context lives in middleware

A new extractor `RequireAppUser` lives at `api/src/auth/app_extractor.rs`:

```
extract Authorization: Bearer <token>
  → look up app_sessions(_id = token)
  → if missing or expired → UNAUTHORIZED
  → look up app_users(_id = session.app_user_id)
  → if missing or status != active → UNAUTHORIZED (defensive)
  → if needs_password_change && route NOT in {GET /app/me, POST /app/me/password, POST /app/auth/logout}
    → 423 NEEDS_PASSWORD_CHANGE
  → fill AppAuthContext { app_user, org, session_token }
  → sliding refresh expires_at
```

Three convenience extractors:
- `AppAuthContext` — base, populated by middleware
- `RequireAppUser` — same as base but enforces 423 gate (used by all /app/* except the trio above)
- (no role distinction — AppUser doesn't have admin/member axis; everyone has the same one-Org membership)

### Reusing dashboard primitives

- `auth::password::hash` / `verify` — reused as-is (bcrypt cost identical).
- `auth::slug::resolve_org_for_join` — reused for the `org_code` field of `/app/auth/login`.
- Random token generation — same primitive as `auth::session_token::generate` (or factor out if signature differs).
- Initial password generator — new helper `auth::app_password::generate_initial()` using the org-code alphabet, length 12.

## Risks / Trade-offs

- **Username collision UX** → Mitigation: admin-web `POST /app-users` returns `USERNAME_TAKEN` and the admin retries with a different one. No magic numeric suffixes.
- **Admin sees plaintext initial password** → Inherent to the "admin gives credential OOB" model; acceptable. Mitigation: cleartext is in the response only, never persisted. The admin-web modal shows it once and emphasizes "won't be shown again".
- **Forgotten password loop without email** → Admin must do password-reset; AppUser must reach admin OOB. Acceptable for the workplace context. Future `add-app-self-serve-reset` can layer email on top.
- **Race: disable a user while they have an in-flight authenticated request** → The current request may complete; the next one fails when middleware can't find their session row (we deleted them). Acceptable for MVP.
- **Bearer token in client-side storage** → Flutter will store the token in secure storage (Keychain/Keystore) as part of `add-app-shell`. Out of scope here, but design.md flags it.
- **Same person being a DashboardUser AND an AppUser** → Allowed. Two independent records, different collections, different auth. The use case is real (e.g. an Org owner who also wants to check in as an employee).
- **`/app/*` endpoint sprawl** → As we add checkin / tracking / etc., the prefix will grow. Keeping admin endpoints under `/app-users/*` (dashboard tenancy) and only mobile-facing under `/app/*` (AppUser tenancy) maintains the auth-surface boundary.
- **Future external auth** → `auth_method` field would be added to `app_users` (default `local`). For LDAP-backed users, `password_hash` would be `null` and login would defer to the external service. The `status` field already gates "can this person log in at all", independent of where the credential lives. Schema change is forward-compatible.

## Migration Plan

1. Create `app_users` collection with unique index `(org_id, username_lower)` and secondary index on `org_id`.
2. Create `app_sessions` collection with unique index on `_id` (the token; trivially the primary key) and TTL or expiry-driven cleanup. Mongo TTL on `expires_at` is the simplest.
3. Wire repositories, handlers, extractor, and routes into `api/src/`.
4. No existing data to migrate.

No rollback plan needed beyond `git revert` — the new collections are additive and untouched by existing flows.

## Open Questions

None blocking. Deferred but worth noting:

- Whether `/app/auth/login` should ever issue a refresh token + short-lived access token instead of a single long-lived sliding token. If we observe security incidents or compliance requirements asking for that, a separate change can introduce it without breaking the URL contract.
- Whether the admin-web list should expose `last_login_at` and let admins sort/filter by it. UX detail — `add-app-shell`-era can confirm what's actually useful.
