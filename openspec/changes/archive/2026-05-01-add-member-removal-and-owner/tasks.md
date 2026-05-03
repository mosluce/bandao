## 1. Domain & DB layer

- [x] 1.1 Add `owner_id: ObjectId` to `Org` in `api/src/domain.rs`
- [x] 1.2 Add `RemovedMembership` struct (`org_id`, `email`, `removed_at`, `cooldown_until`, `removal_kind`) to `api/src/domain.rs`; add `RemovalKind` enum with serde rename `kicked` / `left`
- [x] 1.3 Update `OrgRepository` (`api/src/db/orgs.rs`) `create` signature to accept `owner_id`; ensure existing slug code still passes through
- [x] 1.4 Create `api/src/db/removed_memberships.rs` with `RemovedMembershipRepository`: `insert(org_id, email, kind)`, `find(org_id, email)`, `delete(org_id, email)`, `list_for_org(org_id)`
- [x] 1.5 Wire the new repository into `Database` aggregate in `api/src/db/mod.rs`
- [x] 1.6 Index setup: unique `{ org_id: 1, email: 1 }` and TTL `{ cooldown_until: 1 } expireAfterSeconds: 0` for `removed_memberships` (in the same place where existing collection indexes are configured)

## 2. Register / create flow — set owner

- [x] 2.1 In `handlers/auth.rs::register::Create`, after creating the `dashboard_user`, update the freshly-created Org to set `owner_id = user.id`
- [x] 2.2 Decide whether to create-then-update vs change `OrgRepository::create` to accept a pre-allocated owner id; either is fine, the latter avoids the second write — pick one and apply consistently
- [x] 2.3 Confirm rollback path on user-create failure still deletes the Org (existing `delete_by_id` call)
- [x] 2.4 Update `OrgDto` in `handlers/auth.rs` (and `handlers/me.rs` if it returns Org) to include `owner_id` as a hex string

## 3. Removal flow — `DELETE /dashboard-users/:id`

- [x] 3.1 Add `users::remove` handler with `RequireAdmin` extractor
- [x] 3.2 Reject self via this endpoint (`id == ctx.user_id`) → `FORBIDDEN`
- [x] 3.3 Load target user; cross-Org or missing → `NOT_FOUND`
- [x] 3.4 Load Org; reject if `target.id == org.owner_id` → `OWNER_PROTECTED`
- [x] 3.5 Delete all `dashboard_sessions` for `user_id == target.id` (new repo method `delete_all_by_user_id`)
- [x] 3.6 Delete the `dashboard_user` row
- [x] 3.7 Insert `removed_memberships` marker with `removal_kind = "kicked"` and `email = lowercase(target.email)`
- [x] 3.8 Wire route in `handlers/mod.rs`

## 4. Self-leave flow — `POST /me/leave`

- [x] 4.1 Add `me::leave` handler in `handlers/me.rs`
- [x] 4.2 Reject if `ctx.user_id == org.owner_id` → `OWNER_PROTECTED`
- [x] 4.3 Same delete cascade as removal: sessions → user → marker (`removal_kind = "left"`)
- [x] 4.4 Clear the session cookie via `build_clearing_cookie()` and return `204`
- [x] 4.5 Wire route in `handlers/mod.rs`

## 5. Cooldown enforcement on register-join

- [x] 5.1 In `handlers/auth.rs::register::Join`, after `slug_auth::resolve_org_for_join` and before `dashboard_users.create`, call `removed_memberships.find(org.id, lowercase(input_email))`
- [x] 5.2 If a marker exists with `cooldown_until > now`, reject with new error `EMAIL_IN_COOLDOWN`
- [x] 5.3 Confirm lookup is case-insensitive (input lowercased before query)

## 6. Role-update owner guard + LAST_ADMIN cleanup

- [x] 6.1 In `handlers/users.rs::update_role`, load the Org; if `target.id == org.owner_id` and request is `role=member` → `OWNER_PROTECTED`
- [x] 6.2 Promotion of owner via `role=admin` is a no-op (already admin); ensure the existing "no-op when role unchanged" branch covers this without falling through to the count-admin check
- [x] 6.3 Remove the `count_admins_in_org` call and the `LastAdmin` branch from `update_role`
- [x] 6.4 Remove `count_admins_in_org` method from `DashboardUserRepository` if no longer used elsewhere
- [x] 6.5 Remove `ApiError::LastAdmin` variant and its mapping in `error.rs`

## 7. Cooldown management endpoints

- [x] 7.1 Add `users::list_cooldowns` handler with `RequireAdmin` returning markers for `ctx.org_id`
- [x] 7.2 Add `CooldownDto` (email, removed_at, cooldown_until, removal_kind) for response
- [x] 7.3 Add `users::clear_cooldown` handler with `RequireAdmin`, path param `:email`, deletes `(ctx.org_id, lowercase(email))` marker, returns `204` (idempotent)
- [x] 7.4 Wire routes in `handlers/mod.rs` (`GET /dashboard-users/cooldowns`, `DELETE /dashboard-users/cooldowns/:email`)

## 8. Error code surface

- [x] 8.1 Add `ApiError::OwnerProtected` (HTTP 403, code `OWNER_PROTECTED`)
- [x] 8.2 Add `ApiError::EmailInCooldown` (HTTP 409, code `EMAIL_IN_COOLDOWN`)
- [x] 8.3 Confirm `LAST_ADMIN` mapping is gone (consequence of 6.5)

## 9. API integration tests

- [x] 9.1 Test: `register mode=create` sets `Org.owner_id` to the new user's id
- [x] 9.2 Test: admin removes a member → user gone, sessions gone, marker present, response `204`
- [x] 9.3 Test: admin removes another non-owner admin succeeds
- [x] 9.4 Test: admin trying to remove the owner → `OWNER_PROTECTED`
- [x] 9.5 Test: admin trying to remove themselves via `:id` → `FORBIDDEN`
- [x] 9.6 Test: member trying to remove anyone → `FORBIDDEN`
- [x] 9.7 Test: admin trying to remove user from another Org → `NOT_FOUND`
- [x] 9.8 Test: non-owner self-leave → user gone, sessions gone, marker present, cookie cleared
- [x] 9.9 Test: owner self-leave → `OWNER_PROTECTED`
- [x] 9.10 Test: register-join during cooldown for same Org → `EMAIL_IN_COOLDOWN`
- [x] 9.11 Test: register-join during cooldown for different Org succeeds
- [x] 9.12 Test: register-join with mixed-case email matches lowercased marker
- [x] 9.13 Test: register-join after admin clears cooldown succeeds
- [x] 9.14 Test: list cooldowns returns only caller's Org markers
- [x] 9.15 Test: clear cooldown for non-existent marker returns `204`
- [x] 9.16 Test: member calling cooldown endpoints → `FORBIDDEN`
- [x] 9.17 Test: demoting the owner → `OWNER_PROTECTED`
- [x] 9.18 Test: demoting a non-owner admin succeeds (no LAST_ADMIN check)

## 10. admin-web — user list page

- [x] 10.1 Surface `owner_id` from `/me` (or `/orgs/me`) so the page can compare row.id against owner
- [x] 10.2 Add a "Remove" button on each user row except: the owner row, the current user's own row
- [x] 10.3 Add a confirmation dialog before issuing `DELETE /dashboard-users/:id`
- [x] 10.4 On success, refresh the list (or remove the row optimistically + revalidate)
- [x] 10.5 Display `OWNER_PROTECTED` and `FORBIDDEN` errors as inline notices

## 11. admin-web — profile / settings danger zone

- [x] 11.1 Add a "Danger zone" section to the profile or settings page
- [x] 11.2 Add a "Leave Org" button; disable + show explanatory text when the current user is the owner
- [x] 11.3 Confirmation dialog before issuing `POST /me/leave`
- [x] 11.4 On success, clear local auth state and redirect to login

## 12. admin-web — cooldown management page

- [x] 12.1 New route (e.g. `/dashboard-users/cooldowns`) wired to `GET /dashboard-users/cooldowns`
- [x] 12.2 Render a table with columns: email, removed_at, cooldown_until (relative + absolute), removal_kind
- [x] 12.3 Per-row "Release" button → `DELETE /dashboard-users/cooldowns/:email`, then refresh
- [x] 12.4 Empty state copy when no cooldowns
- [x] 12.5 Hide the menu entry from members (admin-only nav)

## 13. ROADMAP & docs

- [x] 13.1 Add `transfer-org-ownership` entry to `ROADMAP.md` (motivation: owner is currently permanent; needed for owner offboarding)
- [x] 13.2 Add `delete-org` entry to `ROADMAP.md` (motivation: terminal escape hatch when ownership transfer isn't possible)
- [x] 13.3 Remove the now-implemented `成員退出 / 移除` entry from `ROADMAP.md`
- [x] 13.4 Update `api/README.md` if it documents endpoints / error codes (add new endpoints, drop `LAST_ADMIN`, add `OWNER_PROTECTED` / `EMAIL_IN_COOLDOWN`)

## 14. Validation & smoke

- [x] 14.1 `cargo test` passes (unit + integration)
- [x] 14.2 Manual smoke: create Org → invite member → admin remove member → confirm session 401, marker visible in cooldowns page, immediate rejoin blocked, admin clear cooldown, rejoin succeeds
- [x] 14.3 Manual smoke: non-owner admin self-leaves → cookie cleared, redirected to login
- [x] 14.4 Manual smoke: owner sees Leave Org disabled and Remove button hidden on own row
- [x] 14.5 `openspec validate add-member-removal-and-owner` passes
