## 1. Data model (api)

- [x] 1.1 Add `ApiTokenScope` enum to `src/domain.rs` (首發只有 `CheckinRead`, serde rename to `checkin:read`) and `ApiTokenStatus` enum (`Active` | `Disabled`)
- [x] 1.2 Add `OrgApiToken` struct to `src/domain.rs`: `{ id, org_id, name, token_hash, token_prefix, scopes: Vec<ApiTokenScope>, status, created_at, created_by (dashboard_user_id), last_used_at: Option<DateTime>, rotated_at: Option<DateTime> }`
- [x] 1.3 Add `src/db/org_api_tokens.rs` repository: `insert`, `list_by_org`, `find_by_id_and_org`, `find_active_by_hash` (for auth resolution), `update_status`, `rotate` (new hash/prefix/rotated_at), `delete`
- [x] 1.4 Add unique index on `token_hash`; index on `(org_id)` for list queries

## 2. Token generation & hashing (api)

- [x] 2.1 Add `src/auth/api_token.rs`: `generate() -> (String plaintext, String hash, String prefix)` — plaintext is `bandao_at_` + `session_token::generate()`-style 32-byte OsRng base64url; hash is SHA-256 of the full plaintext (**base64-encoded**, not hex — no functional difference, just the encoding chosen); prefix is `bandao_at_` + first 8 chars of the random part for UI display
- [x] 2.2 Unit tests: generated tokens are unique, carry the `bandao_at_` prefix, hash is deterministic for the same input, prefix never contains enough of the token to reconstruct it

## 3. Auth extractor (api)

- [x] 3.1 Add `ApiTokenAuthContext { org_id, scopes }` extractor + `require_scope(scope)` method in `src/auth/api_token.rs` (scope check is a method callers invoke explicitly, not a static per-scope extractor type — scope requirements vary per endpoint)
- [x] 3.2 Parse `Authorization: Bearer <token>`; if prefix is `bandao_at_`, hash the presented token and look up `find_active_by_hash`; on miss or `status == Disabled` return `401 Unauthorized` (generic, no distinction leaked)
- [x] 3.3 On successful match, update `last_used_at` (best-effort, don't fail the request if this write errors)
- [x] 3.4 No router wiring yet — this extractor has no consumer until `add-zhengdan-checkin-export`. Core logic factored into `resolve_from_bearer(db, bearer_value)`, tested directly against a real (testcontainers) `Db` in `tests/org_api_tokens_auth.rs` without touching the real router — see group 5.

## 4. CRUD endpoints (api)

- [x] 4.1 `src/handlers/org_api_tokens.rs`: `GET /orgs/me/api-tokens` (list, admin-only, no plaintext/hash in response — only `id`, `name`, `scopes`, `status`, `token_prefix`, `created_at`, `last_used_at`)
- [x] 4.2 `POST /orgs/me/api-tokens` — body `{ name, scopes: [string] }`; validates `name` non-empty and `scopes` non-empty + all-known (via typed `Vec<ApiTokenScope>` deserialization — an unknown scope value fails to deserialize as `400`); returns the plaintext token **once** in the response body alongside the row
- [x] 4.3 `POST /orgs/me/api-tokens/{id}/rotate` — admin-only, org-scoped; generates new hash/prefix, keeps name/scopes; returns new plaintext token once
- [x] 4.4 `PATCH /orgs/me/api-tokens/{id}` — body `{ status: "active" | "disabled" }`
- [x] 4.5 `DELETE /orgs/me/api-tokens/{id}` — hard delete
- [x] 4.6 Wire all four routes into the existing `protected` router group in `src/handlers/mod.rs` (cookie-session + admin-only, same pattern as `/app-users/*`)
- [x] 4.7 Reused the existing generic `NotFound`/`FORBIDDEN`/`Validation` error codes rather than adding a new `API_TOKEN_NOT_FOUND` — matches the codebase's existing convention of collapsing cross-Org/missing-target lookups to plain `NOT_FOUND` (e.g. `/app-users/:id`), no other endpoint in this codebase has a resource-specific 404 code

## 5. api integration tests

- [x] 5.1 CRUD lifecycle (`tests/org_api_tokens_crud.rs`): create → appears in list with correct `token_prefix` (not the plaintext) → rotate → old secret no longer resolves, new one does → disable → resolution fails while disabled → re-enable (same secret) → resolves again → delete → 404 on subsequent mutation
- [x] 5.2 Scope enforcement: `scopes: []` rejected at creation (400, `ApiError::Validation`) and never persisted; an unknown scope string rejected at the JSON-deserialization layer (422, axum's default `Json` rejection) and never persisted
- [x] 5.3 Non-admin (member role) gets `FORBIDDEN` on all five endpoints
- [x] 5.4 Cross-org isolation: a token from Org A cannot be rotated/disabled/deleted via Org B's admin session (404, not leaking existence); Org B's own list stays empty
- [x] 5.5 `resolve_from_bearer` coverage (`tests/org_api_tokens_auth.rs`, calls it directly against a real testcontainers `Db`, no HTTP/router involved): valid active token resolves `(org_id, scopes)` and `require_scope` succeeds; disabled token fails to resolve; unknown token fails to resolve; a non-`bandao_at_`-prefixed bearer value fails to resolve without ever querying `org_api_tokens` (regression guard against interfering with AppUser bearer auth); rotation invalidates the previous secret immediately

## 6. admin-web

- [x] 6.1 Add API types for `ApiTokenDto` (list view — no secret) and `ApiTokenSecretResponse` (create/rotate — includes plaintext once), plus `ApiTokenScope`/`ApiTokenStatus` and a `API_TOKEN_SCOPES` label list for the checkbox UI (`types/api.ts`); `composables/useApiTokens.ts` wraps the five endpoints
- [x] 6.2 `pages/settings/api-tokens.vue` (admin-only, redirect non-admin): table of tokens (name / scopes / status / created_at / last_used_at) + row actions (rotate / disable-enable / delete, destructive ones — rotate, disable, delete — behind a confirm panel matching the `app-users` page's pattern)
- [x] 6.3 「建立 API Token」inline form: name input + scope checkboxes (fed by `API_TOKEN_SCOPES`) → on success, show the plaintext token once in a copy-to-clipboard modal, matching the AppUser initial-password modal pattern
- [x] 6.4 Rotate re-uses the same one-time-reveal modal pattern
- [x] 6.5 Dashboard entry link (`pages/index.vue`, in the 管理員工具 pill row) linking to the new settings page, next to the existing 驗證來源 link

## 7. Docs & verification

- [x] 7.1 `cargo test` (full suite) clean; `cargo clippy --all-targets` clean; `cargo fmt` applied
- [x] 7.2 admin-web `pnpm typecheck` (exit 0) + `pnpm test` (38 passed) + `pnpm build` (production build succeeds) clean
- [x] 7.3 Manual browser smoke done against the live local dev server (not testcontainers): user logged into admin-web, walked through create → copy plaintext → rotate → disable/enable → delete on `/settings/api-tokens`. Backend side additionally curl-verified end-to-end (register → create → list → rotate → disable → delete, plus a real `/auth/login` round-trip) against the running `api` dev server.
