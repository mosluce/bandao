## 1. Data model & schema

- [x] 1.1 Add `Membership` struct to `api/src/domain.rs` with fields `id, user_id, org_id, role, joined_at, updated_at`; serde uses `_id` for `id`
- [x] 1.2 Strip `org_id` and `role` fields from `DashboardUser` in `api/src/domain.rs` (identity only)
- [x] 1.3 Rename `DashboardSession.org_id` → `current_org_id` in `api/src/domain.rs` and make it `Option<ObjectId>`
- [x] 1.4 Create `api/src/db/dashboard_memberships.rs` with `MembershipRepository`: `create`, `find`, `find_by_user_and_org`, `list_by_user`, `list_by_org`, `update_role`, `delete`, `delete_by_org`, `count_by_user`
- [x] 1.5 In Mongo init code (`api/src/db/mod.rs`), create `dashboard_memberships` collection with unique index on `(user_id, org_id)` and secondary index on `org_id`
- [x] 1.6 In Mongo init code, drop the old `dashboard_users.org_id` index if present

## 2. Repos: rewire existing collections

- [x] 2.1 Update `api/src/db/dashboard_users.rs`: remove `org_id` and `role` from `create` parameters and the struct; remove `list_in_org` (moved to memberships); remove `update_role` (moved to memberships)
- [x] 2.2 Update `api/src/db/dashboard_sessions.rs`: rename `org_id` → `current_org_id`, make it `Option<ObjectId>`; add `update_current_org(token, new_current_org_id)`; add `delete_by_user_and_org(user_id, org_id)` for force-kick scope
- [x] 2.3 Audit `dashboard_sessions.delete_by_user(user_id)` callers — only logout-style flows should use it now; per-org leave/remove uses the new scoped delete

## 3. Auth middleware & extractor

- [x] 3.1 Update `api/src/auth/extractor.rs::AuthContext` to carry `Option<ObjectId> current_org_id` and `Option<Role> role` (both nullable for zero-Org state)
- [x] 3.2 Update `api/src/auth/middleware.rs`: after resolving session, if `current_org_id` is set, look up `dashboard_memberships(user_id, current_org_id)` to fill `role`; if lookup fails treat as `UNAUTHORIZED` and clear cookie
- [x] 3.3 Add `RequireActiveOrg` extractor that requires `current_org_id.is_some()`, returning `NO_ACTIVE_ORG` (403) otherwise; use it at every org-scoped handler entry point
- [x] 3.4 Update `RequireAdmin` extractor to use `RequireActiveOrg` first, then check `role == admin`
- [x] 3.5 Add `ApiError::NoActiveOrg` and `ApiError::NotAMember` and `ApiError::AlreadyMember` and `ApiError::InvalidPassword` and `ApiError::InvalidTarget` and `ApiError::SameOwner` variants in `api/src/error.rs`, with appropriate HTTP statuses

## 4. Auth handlers (register, login, logout)

- [x] 4.1 In `api/src/handlers/auth.rs::register`, `mode=create`: insert `dashboard_user` (identity only), insert `dashboard_membership(role=admin)`, insert session with `current_org_id = new_org.id`; rollback membership and Org if anything fails
- [x] 4.2 In `register`, `mode=join`: identity-not-exists check (existing email → `EMAIL_TAKEN`); cooldown check before insert; insert identity + `dashboard_membership(role=member)` + session
- [x] 4.3 In `api/src/handlers/auth.rs::login`, after credential verification: load all of user's memberships; pick default `current_org_id` per spec rule (oldest owned > oldest membership > null); insert session with that `current_org_id`; return `{ user, memberships, current_org }`
- [x] 4.4 Update `AuthResponse` DTO in `auth.rs` to include `memberships: Vec<MembershipDto>` and `current_org: Option<OrgDto>` and `role: Option<Role>`; remove the top-level `org` and `role` fields' previous strict shape
- [x] 4.5 Logout handler (`POST /auth/logout`) is unchanged in semantics; verify it still passes integration tests after middleware changes

## 5. /me handlers

- [x] 5.1 `GET /me`: return `{ user, memberships: [{org, role}], current_org: Org | null, role: Role | null }`; works with `current_org_id == null`
- [x] 5.2 `POST /me/orgs` (org-agnostic): create new Org with caller as owner, insert membership(role=admin), update current session `current_org_id` to new Org; return updated `/me`-shaped payload
- [x] 5.3 `POST /me/memberships` (org-agnostic): resolve target Org via the existing `resolve_org_for_join` (org_code / active slug / grace slug); reject `ALREADY_MEMBER` if `(user, org)` membership exists; cooldown check; insert membership(role=member); update current session `current_org_id` to joined Org
- [x] 5.4 `POST /me/current-org` (org-agnostic): verify caller has membership in target org; update current session's `current_org_id`; return updated `/me`-shaped payload
- [x] 5.5 Rewrite `POST /me/leave`: now scoped to `current_org`. Delete only the membership for `(ctx.user_id, current_org_id)`. Delete only sessions where `(user_id == ctx.user_id AND current_org_id == current_org_id)`. Insert cooldown marker. Reject if caller is `current_org.owner_id` with `OWNER_PROTECTED`.

## 6. /dashboard-users handlers (membership-scoped)

- [x] 6.1 `PATCH /dashboard-users/:id/role`: operate on `dashboard_memberships(target_id, current_org_id)`. Cross-Org membership lookups return `NOT_FOUND`. Owner protection: reject demoting `current_org.owner_id`.
- [x] 6.2 `DELETE /dashboard-users/:id`: delete membership row only; delete only sessions where `(user_id == target.id AND current_org_id == current_org_id)`; insert cooldown marker. Owner / self / cross-org protections preserved.
- [x] 6.3 `GET /dashboard-users` (list members of current_org): use `MembershipRepository::list_by_org(current_org_id)` joined with user identities to return the same payload shape callers expect.
- [x] 6.4 `GET /dashboard-users/cooldowns` and `DELETE /dashboard-users/cooldowns/:email`: behavior unchanged at the handler level beyond using `current_org_id`; verify tests after the middleware refactor.

## 7. Owner transfer

- [x] 7.1 Add `POST /orgs/me/owner` handler in `api/src/handlers/orgs.rs`. Body: `{ new_owner_user_id, current_password }`. Caller must be `current_org.owner_id`.
- [x] 7.2 Verify `current_password` against caller's stored hash via `auth::password::verify`; return `INVALID_PASSWORD` on mismatch.
- [x] 7.3 Verify target has `dashboard_memberships(new_owner_user_id, current_org_id)` with `role=admin`; return `INVALID_TARGET` otherwise.
- [x] 7.4 Reject `new_owner_user_id == ctx.user_id` with `SAME_OWNER`.
- [x] 7.5 Update `org.owner_id = new_owner_user_id` and `org.updated_at = now`. Do not touch sessions, memberships, or any other Org field.

## 8. API integration tests — rewrite existing 16 to multi-org model

- [x] 8.1 Update `tests/common/mod.rs` test helpers: builder for `(identity, org, membership, session)` with explicit role and current_org; helper for "user with N orgs"; helper to assert membership counts.
- [x] 8.2 Rewrite `tests/auth_register.rs`: identity + Org + membership all created; existing-email rejected.
- [x] 8.3 Rewrite `tests/auth_register_by_slug.rs` and `tests/auth_register_by_grace_slug.rs` for the new identity+membership shape.
- [x] 8.4 Rewrite `tests/auth_register_cooldown.rs` for the new cooldown gate.
- [x] 8.5 Rewrite `tests/auth_login.rs` to assert default `current_org_id` rule (owned-first, oldest-fallback, null when no memberships).
- [x] 8.6 Rewrite `tests/auth_logout.rs`: verify session is deleted; identity and memberships preserved.
- [x] 8.7 Rewrite `tests/session_expiry.rs` for the new session shape (`current_org_id` may be null).
- [x] 8.8 Rewrite `tests/users_role.rs` operating on memberships; cross-org lookups return NOT_FOUND.
- [x] 8.9 Rewrite `tests/users_remove.rs`: only the membership + scoped sessions are removed; identity survives.
- [x] 8.10 Rewrite `tests/users_cooldowns.rs` after handler changes.
- [x] 8.11 Rewrite `tests/me_leave.rs`: only membership + scoped sessions deleted; identity preserved; sessions for other orgs intact; owner cannot leave.
- [x] 8.12 Rewrite `tests/orgs_rotate.rs`, `tests/orgs_slug_set.rs`, `tests/orgs_slug_clear.rs`, `tests/orgs_slug_lookup_format.rs` to construct ctx via memberships.

## 9. API integration tests — new

- [ ] 9.1 `tests/me_orgs_create.rs`: logged-in user creates a new Org, becomes owner, current_org_id updates; works in zero-org state.
- [ ] 9.2 `tests/me_memberships_join.rs`: join via org_code / active slug / grace slug; ALREADY_MEMBER rejection; cooldown applies; current_org_id updates.
- [ ] 9.3 `tests/me_current_org_switch.rs`: switch to another membership; NOT_A_MEMBER for non-member orgs; current session only is mutated.
- [ ] 9.4 `tests/zero_org_state.rs`: org-scoped endpoints return NO_ACTIVE_ORG; org-agnostic ones succeed; user can recover via /me/orgs or /me/memberships.
- [ ] 9.5 `tests/multi_org_isolation.rs`: user in two orgs sees only `current_org`'s members / cooldowns / etc.; switching org changes the visible scope.
- [ ] 9.6 `tests/orgs_owner_transfer.rs`: happy path; non-owner forbidden; member forbidden; wrong password; invalid target (not admin / not in org); self-transfer rejected; previous owner can self-leave after transfer; new owner is protected.
- [ ] 9.7 `tests/membership_force_kick.rs`: leaving / being removed from Org X kills only sessions whose `current_org_id == X`; sessions pointing at other orgs survive.
- [ ] 9.8 `tests/auth_role_lookup_per_request.rs`: role demotion takes effect on the next authenticated request without re-login; stale-membership session returns UNAUTHORIZED + clears cookie.

## 10. admin-web: composables and middleware

- [ ] 10.1 Update `composables/useAuth.ts` to expose `user`, `memberships`, `currentOrg` (reactive), `role` (derived), and actions `createOrg`, `joinOrg`, `switchOrg`, `leaveOrg`, `transferOwnership`.
- [ ] 10.2 Implement localStorage-backed `lastSelectedOrgId`: on app boot, if a value exists and matches a current membership, call `switchOrg` to align server state.
- [ ] 10.3 Update `middleware/auth.ts` to allow zero-Org state for org-agnostic routes and redirect to the empty-state page if landing on org-scoped routes with no current_org.
- [ ] 10.4 Add a global "current_org changed" reactive signal so org-scoped pages refetch their data on switch.
- [ ] 10.5 Update `types/api.ts` to mirror the new DTOs (`MembershipDto`, updated `AuthResponse`, etc.).

## 11. admin-web: pages and components

- [ ] 11.1 Build header `OrgSwitcher` component: dropdown grouped by "我擁有的" / "我加入的", role badges (擁有者 / 管理員 / 成員), `+ 建立新組織`, `+ 用 org code 加入`.
- [ ] 11.2 Build empty-state page (e.g. `pages/no-org.vue`): copy "你目前不屬於任何組織"; CTAs `[ 建立新組織 ]` and `[ 加入組織 ]`.
- [ ] 11.3 Build `OrgCreateModal` and `OrgJoinModal` components (or full pages); wire to `useAuth().createOrg` / `joinOrg`.
- [ ] 11.4 Update `pages/index.vue` to read `currentOrg` (not `org`) and re-fetch on switch.
- [ ] 11.5 Update `pages/members.vue`, `pages/cooldowns.vue` similarly; ensure they handle a current_org change without stale data.
- [ ] 11.6 Add an "Owner transfer" flow on `members.vue` (visible only to owner): pick admin, enter password, confirm; surface INVALID_PASSWORD / INVALID_TARGET / SAME_OWNER.
- [ ] 11.7 Update `pages/login.vue` and `pages/register.vue` for the new response shapes; make sure landing-after-auth respects `current_org` (or routes to empty-state if null).

## 12. Cleanup & docs

- [ ] 12.1 Remove the `transfer-org-ownership` line from `ROADMAP.md` Side ideas (delivered in this change).
- [ ] 12.2 Soften the wording on the `delete-org` ROADMAP entry to note it now cascades memberships only (identities survive).
- [ ] 12.3 Update `api/README.md` with the new model description (identity vs membership, zero-Org state).
- [ ] 12.4 Update `admin-web/README.md` with the new flows (org switcher, zero-Org page, transfer ownership).
- [ ] 12.5 Note the local-DB-wipe expectation in the PR description (no migration, drop-and-recreate).

## 13. Smoke

- [ ] 13.1 Bring up local stack (mongo + api + admin-web), wipe DB, register two identities, each creating their own Org; have one join the other's; verify switching, leaving, owner transfer, zero-org recovery end-to-end in the browser.
- [ ] 13.2 Run `cargo test` and `pnpm typecheck` + `pnpm build` clean.
