## 1. api: config storage + validation

- [x] 1.1 `domain.rs`: added `list_query: Option<String>` to `ExternalAuthConfig`. Also added it to `handlers/auth.rs::ExternalAuthSummaryDto` (+ `from_config`) — **not in the original task list**, but without it the admin-web settings page has no way to read back the saved `list_query` on load (every other field is surfaced through this DTO for the same "pre-fill the form" reason).
- [x] 1.2 `handlers/external_auth.rs`: added `list_query: Option<String>` to `ExternalAuthInput`, threaded through `build_config`
- [x] 1.3 `auth/providers/mod.rs`: new `validate_list_query_settings(driver, list_query)` — rejects `@account`/`@password` placeholders, otherwise mirrors `validate_query_settings`'s driver check. Called from `build_config` only when `list_query` is `Some`.
- [x] 1.4 `error.rs`: added `ApiError::ExternalAuthNotEnabled` → `409 EXTERNAL_AUTH_NOT_ENABLED` (distinct from `ExternalAuthMode`, which means the opposite direction — see design.md D5)

## 2. api: sync execution

- [x] 2.1 `auth/providers/mssql.rs`: extracted the shared connection-setup logic out of `resolve_identity` into a private `connect()` method (returns `Client<Compat<TcpStream>>`), reused by both `resolve_identity` and the new `list_identities(&self, list_query: &str)`. `list_identities` runs with zero bound parameters (`client.query(list_query, &[])`), uses `stream.into_first_result()` (tiberius 0.12's multi-row collector — confirmed via crate source, no existing precedent for this in the codebase) to get every row, and returns `Vec<ListedIdentity { external_key: Option<String>, display_name: Option<String> }>`. Column-not-found (via `column_string()`) propagates as `Err(Unavailable)`, same as `resolve_identity`; a NULL/absent `key_col` on an individual row is NOT an error here — it's `external_key: None`, and the caller (the `sync` handler) decides how to treat that.
- [x] 2.2 `db/app_users.rs`: new `sync_upsert_shadow(org_id, external_key, display_name) -> ApiResult<SyncUpsertOutcome>` per design.md D2 — does NOT touch `last_login_at`, distinct from `upsert_shadow`. New `SyncUpsertOutcome { Created, Updated }` enum, re-exported from `db/mod.rs`.
- [x] 2.3 `handlers/external_auth.rs`: `sync` handler — `RequireAdmin`, checks `org.auth_source() == OrgAuthSource::ExternalDb` (else `ExternalAuthNotEnabled`), checks `list_query` is configured and non-blank (else `Validation`), runs the list query, loops rows applying the skip/create/update logic from design.md D4, returns `SyncResponse { total_rows, created, updated, skipped: Vec<SkippedRow> }`. Connection/query/column-mapping failures map to a new `ApiError::ExternalAuthSyncFailed(String)` (task 1.4 correction — the plan only anticipated `ExternalAuthNotEnabled`, but a bare error with no diagnostic would be a real UX regression for this admin-troubleshooting surface, mirroring why `test_login` already surfaces `error: Option<String>` instead of a generic failure).
- [x] 2.4 `handlers/mod.rs`: registered `POST /orgs/me/external-auth/sync`

## 3. api integration tests

All ten covered by a new `api/tests/external_auth_sync.rs`, sharing a `setup()` helper (boots MSSQL, seeds two `staff` rows, configures the Org for `external_db` with both `query` and `list_query`).

- [x] 3.1 Saving a `list_query` containing `@account` or `@password` is rejected (validation error, not persisted) — `saving_list_query_with_placeholders_is_rejected`
- [x] 3.2 Sync creates new `AppUser` rows with `last_login_at = null`, `status = active` for `external_key`s not previously known — `sync_creates_new_shadow_users`
- [x] 3.3 Sync updates `display_name` for an already-existing external `AppUser` without touching its `last_login_at` — `sync_updates_display_name_without_touching_last_login_at`
- [x] 3.4 A local `AppUser` whose `external_key` is absent from the sync result is completely unchanged after sync — `sync_never_touches_users_absent_from_the_result`
- [x] 3.5 A row with empty/NULL `key_col` is skipped and reported in `skipped`, other rows still process; response is still `200` — `sync_skips_rows_with_null_key_col`
- [x] 3.6 `key_col`/`display_col` column-not-found in the result set fails the whole sync with no writes — `sync_fails_whole_batch_on_missing_column`
- [x] 3.7 Sync rejected with `EXTERNAL_AUTH_NOT_ENABLED` when `current_org.auth_source == internal`, even if `external_auth` (incl. `list_query`) is configured — `sync_rejected_when_auth_source_is_internal`
- [x] 3.8 Sync rejected with a validation error when `auth_source == external_db` but no `list_query` is set — `sync_rejected_when_list_query_not_configured`
- [x] 3.9 Member (non-admin) gets `FORBIDDEN` calling sync — `sync_forbidden_for_non_admin`
- [x] 3.10 Regression: existing `external-db-auth` login/test-login/configure tests still pass unchanged — `login_and_test_login_regression_with_list_query_configured`, plus the pre-existing `external_auth_login.rs`/`external_auth_config.rs` suites re-run clean

**Corrections made while writing these:**
- Found and fixed a real bug in the test helper itself (not app code): the first draft of `setup()` didn't return the `ContainerAsync<MssqlServer>` handle, so it dropped (stopping the container) the instant `setup()` returned — every `sync()` call then failed with `EXTERNAL_AUTH_SYNC_FAILED` / "Connection refused". Fixed by returning the handle as a 6th tuple element; every caller keeps it bound (`_mssql` where unused) for the test's lifetime.
- Running this file's 9 MSSQL-backed tests with default `cargo test` parallelism reliably crashes MSSQL containers ("Could not allocate initial 5000 lock owner blocks during startup") from concurrent-container memory contention — not present before since only one such test existed (`external_auth_login.rs`). Added `serial_test` as a dev-dependency and `#[serial(mssql)]` on every MSSQL-booting test in both this file and `external_auth_login.rs`, so they never run concurrently regardless of `cargo test`'s thread count. Verified: full suite passes with default parallelism afterward (two unrelated pre-existing Mongo-container flakes seen once under heavy parallel load, confirmed to pass in isolation — not caused by this change).

## 4. admin-web

- [x] 4.1 `types/api.ts`: `ExternalAuthSummaryDto`/`ExternalAuthInput` got `list_query?: string`; new `SyncSkippedRow`/`SyncExternalUsersResponse` types
- [x] 4.2 `composables/useExternalAuth.ts`: `sync(): Promise<SyncExternalUsersResponse>` calling `POST /orgs/me/external-auth/sync`
- [x] 4.3 `pages/settings/auth.vue`: "同步查詢" textarea under 驗證查詢 (empty → omitted from the `configure` payload so it doesn't clobber a stored value on an unrelated edit)
- [x] 4.4 `pages/settings/auth.vue`: "同步使用者名單" section — gated on `currentSource === 'external_db'` (the *saved* org state, not the form's in-progress `source` toggle, since the server-side gate is on the persisted auth_source); click → `sync()` → loading state → inline created/updated/skipped summary. No confirmation dialog.

## 5. Docs & verification

- [x] 5.1 `cargo test --all-features --no-fail-fast` clean (default parallelism, matching CI); `cargo clippy --all-targets --all-features -- -D warnings` clean; `cargo fmt --all` applied
- [x] 5.2 admin-web `pnpm typecheck` (0 errors) + `pnpm test` (7 files / 38 tests passed) + `pnpm build` clean
- [x] 5.3 Manual smoke via real dev servers + a standalone `docker run mcr.microsoft.com/mssql/server:2022-latest` container (seeded a 2-row `staff` table) + Playwright driving an actual browser against `localhost:3000`: configured `list_query` through the settings/auth UI's new textarea, clicked "同步使用者名單" → got "共 2 筆，新增 2 筆、更新 0 筆", confirmed both rows appear in App 使用者 with `上次登入 = —`; changed one row's `name` directly in MSSQL and re-synced → "共 2 筆，新增 0 筆、更新 2 筆" (upsert always counts existing rows as `Updated` regardless of whether any field actually changed — matches the "purely additive/update" design, no diffing required), confirmed the changed display_name landed locally and `last_login_at` stayed untouched.
