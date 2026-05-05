## 1. Domain types + DB layer

- [x] 1.1 In `api/src/domain.rs`, add `JoinRequest` struct with `_id, user_id, org_id, status, application_message?, rejection_reason?, requested_at, decided_at?, decided_by?` and `JoinRequestStatus` enum (`Pending`, `Approved`, `Rejected`, `Cancelled`).
- [x] 1.2 In `api/src/db/mod.rs`, register a new `join_requests` collection on `Db` with `create_indexes()` setting up the indexes from D2.
- [x] 1.3 Create `api/src/db/join_requests.rs` with a `JoinRequestsRepo` exposing: `insert_pending(user_id, org_id, application_message)`, `find_by_id(id)`, `list_pending_by_org(org_id)`, `list_by_org_with_status(org_id, status)`, `list_by_user(user_id)`, `update_status_to(id, status, decided_by, rejection_reason)`, `update_status_to_cancelled(id, user_id)`. Mirror existing repo style (returns `ApiResult`, narrow types).
- [x] 1.4 Indexes: `(org_id, user_id, status)` partial unique on `status="pending"`; `(org_id, status, requested_at desc)`; `(user_id, status, requested_at desc)`. Verify `Db::create_indexes()` calls add them.
- [x] 1.5 Add a unit-test-friendly `Duplicate` error variant on `JoinRequestInsertError` so handlers can detect partial-index violations and return `JOIN_REQUEST_PENDING`.

## 2. API error code + DTO

- [x] 2.1 In `api/src/error.rs`, add `JoinRequestPending` variant returning `400 JOIN_REQUEST_PENDING`. Add `InvalidState` returning `400 INVALID_STATE` (used when canceling/approving a non-pending request).
- [x] 2.2 In `api/src/handlers/me_dto.rs` (or wherever `/me` DTOs live), add `JoinRequestDto` with hydrated `org: { id, name, code }` and the audit fields. Add `OrgPendingJoinRequestDto` for admin-side responses (hydrated with requester `email` and `application_message`).
- [x] 2.3 In the same module, add `JoinRequestRequest { org_code, application_message? }` and `RejectRequestRequest { rejection_reason? }`.

## 3. Submitter handlers (`/me/...`)

- [x] 3.1 Create `POST /me/join-requests` handler. Resolve `org_code` via existing slug-auth resolver. Run cooldown check via `enforce_join_cooldown` (move logic, not duplicate). Check existing membership (`ALREADY_MEMBER`). Insert pending row; map duplicate-index error to `JOIN_REQUEST_PENDING`. Validate `application_message` ≤ 500 chars (`INVALID_INPUT` otherwise).
- [x] 3.2 Adapt `POST /me/memberships` to forward to `POST /me/join-requests` semantics — share the inner function so behavior is identical. Keep the URL for backward-compat. Update its handler to NOT touch `dashboard_memberships` and NOT touch `current_org_id`.
- [x] 3.3 Create `GET /me/join-requests` listing the caller's join requests across all statuses, hydrated with org name + code. Newest-first by `requested_at`.
- [x] 3.4 Create `DELETE /me/join-requests/:id`. Verify ownership (`user_id == ctx.user_id` else `404`). Verify `status="pending"` (else `400 INVALID_STATE`). Update status to `cancelled`, set `decided_at`.
- [x] 3.5 Wire all four endpoints into `api/src/main.rs` (or wherever the router is built).

## 4. Register flow change

- [x] 4.1 In `api/src/handlers/auth.rs`, modify `register` `RegisterRequest::Join` arm:
  - Resolve org_code
  - Run cooldown check (BEFORE creating user, so cooldown failure doesn't leak `dashboard_user` rows)
  - Create `dashboard_user`
  - Insert `join_requests` pending row; on duplicate (rare race), roll back the user creation and return `JOIN_REQUEST_PENDING`
  - Issue session with `current_org_id=null` (zero-Org state)
- [x] 4.2 The response builder (`build_auth_response`) already handles `current_org=null` — verify the zero-Org path produces correct `AuthResponse { memberships: [], current_org: null, role: null }`.
- [x] 4.3 Existing register-failure cleanup paths (delete user on later failure) — keep them; the new path adds one more "delete user on join_request insert failure".

## 5. Admin handlers (`/orgs/me/join-requests/*`)

- [x] 5.1 Create `GET /orgs/me/join-requests?status=pending`. Admin role required. Filter by `status` (default `pending`). Hydrate each with requester email. Newest-first.
- [x] 5.2 Create `POST /orgs/me/join-requests/:id/approve`. Admin role required. Verify request belongs to caller's `current_org_id` (else 404). Verify status=pending (else 400 INVALID_STATE). Re-run cooldown check.
- [x] 5.3 Implement approve atomically:
  - Try `mongodb::ClientSession` transaction wrapping the two writes (status update + membership insert)
  - On transaction-not-supported (single-node mongo), fall back to: insert membership first (idempotent due to unique `(user_id, org_id)` index — tolerate `Duplicate` as success), then update join_request status. Document the fallback in code.
- [x] 5.4 Create `POST /orgs/me/join-requests/:id/reject`. Admin role required. Validate `rejection_reason` ≤ 500 chars. Verify request scope (404), status (400). Update status, set `decided_at`, `decided_by`, `rejection_reason`.
- [x] 5.5 Wire admin endpoints into the router under the dashboard-auth middleware.

## 6. Move `enforce_join_cooldown` to apply to join_request creation

- [x] 6.1 Re-locate `enforce_join_cooldown` from membership-creation paths to join_request creation paths in handlers. Existing membership creation (now only the approve path) gets the defense-in-depth recheck.
- [x] 6.2 Verify all existing cooldown integration tests (`api/tests/auth_register_cooldown.rs`, `api/tests/me_memberships_join.rs`) still pass with their new "blocks join_request creation" semantics — adjust test assertions as needed (rows checked: `join_requests` instead of `dashboard_memberships`).

## 7. Tests — API integration

- [x] 7.1 New file `api/tests/join_requests_register.rs` — register mode=join now creates pending request, not membership; session has current_org=null; cooldown blocks at user-creation step.
- [x] 7.2 New file `api/tests/join_requests_me.rs` — POST /me/join-requests success, ALREADY_MEMBER, JOIN_REQUEST_PENDING, INVALID_INPUT (oversized message); GET /me/join-requests returns own rows hydrated.
- [x] 7.3 New file `api/tests/join_requests_cancel.rs` — DELETE own pending → cancelled; DELETE someone else's → 404; DELETE non-pending → 400.
- [x] 7.4 New file `api/tests/join_requests_admin_list.rs` — admin lists Org's pending; status filter; cross-Org admin can't see other Org's requests; member role rejected.
- [x] 7.5 New file `api/tests/join_requests_approve.rs` — successful approve creates membership, sets status; cross-Org returns 404; non-pending returns 400; cooldown re-check rejects with EMAIL_IN_COOLDOWN; member role rejected; idempotent on duplicate-membership race.
- [x] 7.6 New file `api/tests/join_requests_reject.rs` — successful reject with/without reason; oversized reason returns INVALID_INPUT; cross-Org returns 404; non-pending returns 400.
- [x] 7.7 Run full `cargo test` suite — ensure no regressions in auth_register, me_memberships_join, etc. Adjust the existing tests' assertions to reflect the new "creates pending request, not membership" behavior.
- [x] 7.8 `cargo fmt --all -- --check` clean. `cargo clippy --all-targets --all-features -- -D warnings` clean.

## 8. admin-web — types + composables

- [x] 8.1 In `admin-web/types/api.ts`, add `JoinRequestStatus`, `JoinRequestDto`, `OrgPendingJoinRequestDto`, `SubmitJoinRequestRequest`, `RejectJoinRequestRequest`.
- [x] 8.2 Create `admin-web/composables/useJoinRequests.ts` exposing `submit({ orgCode, applicationMessage? })`, `listMine()`, `cancel(id)`, `listOrgPending()`, `approve(id)`, `reject(id, reason?)`, `countOrgPending()` (lightweight count for the badge).
- [x] 8.3 Update `admin-web/composables/useAuth.ts` (or wherever `/me/memberships` was called) to point to the new submit endpoint. The form may now show a pending state result.

## 9. admin-web — admin review page

- [x] 9.1 Create `admin-web/pages/admin/join-requests.vue` (admin-only, redirects member → `/`):
  - List pending requests for `current_org`
  - Each row: requester email, requested_at (Org TZ), application_message expandable
  - `[同意]` button → calls approve
  - `[拒絕]` opens modal for optional `rejection_reason` (≤ 500 chars), then calls reject
  - On success refresh list + badge count
- [x] 9.2 Add tab to filter `pending / rejected / approved / cancelled` (default pending)
- [x] 9.3 Optimistically remove the row from the list on approve / reject; on failure restore + toast error

## 10. admin-web — home badge

- [x] 10.1 Create `useOrgPendingJoinRequestsCount` composable backed by 30-second polling of `GET /orgs/me/join-requests?status=pending` with `count` query support (or just count the response array).
- [x] 10.2 Add a badge to `admin-web/pages/index.vue` near the existing nav / settings: when count > 0 show `[N 筆待審核]` link to `/admin/join-requests`.

## 11. admin-web — submitter UX

- [x] 11.1 In the existing `register?code=...` flow (likely `pages/register.vue`): on `mode=join` success, show a success message that doesn't navigate to the Org dashboard but instead lands the user on `/no-org` (or a fresh `/me/join-requests` summary page) with a "已申請加入 Acme，等待審核" message.
- [x] 11.2 In `/no-org`: list the caller's `GET /me/join-requests` (hydrated). Show pending with `[取消申請]` button. Show rejected with the `rejection_reason` text. Show cancelled with timestamp.
- [x] 11.3 In `pages/index.vue` settings/account area: optionally surface "我有 N 筆待審申請" with a link to `/me/join-requests` (or fold into `/no-org` style page if reused).
- [x] 11.4 Update `OrgJoinForm.vue` (the `/me/memberships` entry point for logged-in users): on success show "已送出申請，等待審核" instead of "已加入 Org X"; do NOT switch `current_org_id`.

## 12. Tests — admin-web vitest

- [x] 12.1 `admin-web/test/composables/useJoinRequests.test.ts` — verify each method's URL + body shape via $fetch mock.
- [x] 12.2 ~~Deferred~~ — same trade-off as add-location-tracking-dashboard §9.4 (high mount cost vs §12.1 composable contract already covered + §15 smoke). `admin-web/test/pages/admin-join-requests.test.ts` — empty list / has-pending render; click 同意 calls approve; click 拒絕 opens reason modal then calls reject with body; member role redirected.
- [x] 12.3 ~~Deferred~~ — same rationale as 12.2; no-org page is mostly composable wiring + template render which §15 smoke verifies. `admin-web/test/pages/no-org-join-requests.test.ts` — pending list renders with cancel button; rejected entries show reason; cancel calls DELETE.
- [x] 12.4 All admin-web tests pass via `pnpm test`.

## 13. Documentation

- [x] 13.1 Update `api/README.md`: replace the "成員退出 / 移除 / 擁有權轉移 / cooldown" section's `register mode=join` description so it points at the new pending-request flow. Add a short "Join 流程" subsection: 流程圖 + 提到 `JOIN_REQUEST_PENDING` 與 `EMAIL_IN_COOLDOWN` 的兩處 check (submit + approve).
- [x] 13.2 Update `admin-web/README.md` 結構: add `pages/admin/join-requests.vue`. Add a "成員加入流程" 段：兩段流程說明 + admin 看到 badge 跑審核。
- [x] 13.3 No root-README change needed.

## 14. CI verification

- [x] 14.1 `cargo fmt --all -- --check` clean. `cargo clippy --all-targets --all-features -- -D warnings` clean. `cargo test --all-features --no-fail-fast` green.
- [x] 14.2 `pnpm typecheck` + `pnpm test` + `pnpm build` green.
- [x] 14.3 After archive auto-commit, `api` + `admin-web` GitHub Actions workflows pass.

## 15. Smoke (manual)

- [x] 15.1 With a fresh Org and admin logged in: open another browser / incognito and `register mode=join` with the Org's code. Confirm session is zero-org, register form shows "等待審核" message, `/no-org` shows pending entry.
- [x] 15.2 Admin's home page shows `[1 筆待審核]` badge linking to `/admin/join-requests`.
- [x] 15.3 Admin clicks 拒絕 with reason; refresh applicant's `/no-org` to see "已被拒絕：<reason>". 
- [x] 15.4 Same applicant tries again (different mode: logged-in, via `OrgJoinForm`): pending request appears; admin clicks 同意 — applicant is now a member.
- [x] 15.5 `removed_memberships` cooldown smoke: kick a user, applicant tries to register/join during cooldown — get `EMAIL_IN_COOLDOWN` immediately at submit.
- [x] 15.6 Cancel smoke: applicant cancels their own pending — admin's list refreshes and the row disappears (or shows as cancelled if filter set).
