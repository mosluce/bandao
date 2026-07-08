## 1. Data model & config (api)

- [x] 1.1 Add `AppUserAuthSource` enum (`Internal` | `External`) to `domain`; make `AppUser.password_hash: Option<String>` and add `auth_source` + `external_key: Option<String>` (also made `username`/`username_lower`/`created_by_dashboard_user_id` optional — absent for shadow users)
- [x] 1.2 Add `auth_source` (default `Internal`) and `external_auth` sub-document types to the `Org.settings` model; define `ExternalAuthConfig { driver, host, port, database, username, password_encrypted, query, key_col, display_col }` (+ `Org::auth_source()` / `Org::external_auth()` accessors)
- [x] 1.3 Introduce a symmetric AEAD helper in `api/` for the `external_auth` connection password — added `chacha20poly1305` (XChaCha20-Poly1305) `SecretBox` + `BANDAO_SECRET_KEY` env (base64 of 32 bytes) in `config.rs`; encrypt on write, decrypt only in-memory; never log/return plaintext. No key rotation (per decision). DEPLOY.md env matrix: task 8.1.
- [x] 1.4 Add the partial/sparse unique index `(org_id, external_key)` for external shadow users; keep the existing `(org_id, username_lower)` unique index for internal (migrated it to PARTIAL so shadow users' missing `username_lower` don't collide on null)
- [x] 1.5 Update `app_users` repository: `find_by_org_and_external_key`, `upsert_shadow(org_id, external_key, display_name)`, and adjust list to include external users; ensure DTO carries `auth_source` / `external_key`

## 2. Auth provider abstraction (api)

- [x] 2.1 Define `AppAuthProvider` trait (`authenticate(account, password) -> Result<AppUser, AuthProviderError>`; providers resolve to the local row that anchors the session — cleaner than a bare `ExternalIdentity`) + `AuthProviderError { InvalidCredentials, Unavailable(diagnostic) }`
- [x] 2.2 Implement `InternalProvider` wrapping the current Mongo + password-hash flow behind the trait
- [x] 2.3 Add a provider registry (`provider_for`) that selects from `Org.auth_source` (+ `external_auth.driver`); unsupported driver / missing config → `Unavailable`
- [x] 2.4 Add `ApiError` variants `EXTERNAL_AUTH_UNAVAILABLE` (503) and `EXTERNAL_AUTH_MODE` (409); wire into the outward error format

## 3. MSSQL provider (api)

- [x] 3.1 Add `tiberius` dependency (+ async TCP glue) to `api/Cargo.toml` (`tiberius 0.12` `["tds73","rustls"]` + `tokio-util` compat; resolves & compiles)
- [x] 3.2 Implement `MssqlProvider`: connect, bind `@account` / `@password` as tiberius params `@P1`/`@P2` (never string-interpolate), run `query`, read `key_col` / `display_col` from the single result row (multi-type column coercion) — **compiles; live verification pending group 6**
- [x] 3.3 Map provider outcomes: 0 rows → credential failure; connect/query/column errors → typed `Unavailable` with a specific diagnostic (column-not-found distinct from connect/query); password never placed in logs/errors
- [x] 3.4 Implement `external_auth` config validation (`providers::validate_query_settings`): driver supported, `query` contains both `@account`/`@password`, `key_col`/`display_col` non-empty

## 4. Login & shadow provisioning (api)

- [x] 4.1 Rewrite `POST /app/auth/login` to resolve the Org, select the provider, and delegate credential verification (internal path verified: all 8 login integration tests pass; external_db currently returns `EXTERNAL_AUTH_UNAVAILABLE` via the stubbed MSSQL provider)
- [x] 4.2 On external success, upsert the shadow AppUser `(org_id, external_key)` (create with `auth_source=external`, `password_hash=None`, `needs_password_change=false`; else refresh `display_name`/`last_login_at`), enforce `status==active`, then issue the session — `MssqlProvider::authenticate` calls `upsert_shadow`; login handler enforces active + issues session
- [x] 4.3 Collapse credential failures to `INVALID_CREDENTIALS`; surface provider unavailability as `EXTERNAL_AUTH_UNAVAILABLE`; keep the internal path behavior-identical

## 5. Org auth-source & test-login endpoints (api)

- [x] 5.1 Added dedicated `PUT /orgs/me/external-auth` (admin-only) to set `auth_source` + `external_auth` config; validates before persisting; rejects `external_db` without a valid config; write-only password (encrypt new, else keep stored)
- [x] 5.2 Gate `POST /app-users` and `POST /app-users/:id/password-reset` with `EXTERNAL_AUTH_MODE` while `auth_source == external_db` (`ensure_internal_auth`); `PATCH` disable still works in both modes
- [x] 5.3 Implement `POST /orgs/me/external-auth/test-login` (admin-only, org-scoped, dry-run via `resolve_identity`): returns `{connected, matched, external_key, display_name, error}`, creates no session / no shadow row, test password not persisted/logged — **live verification pending group 6**
- [x] 5.4 `OrgDto` exposes `auth_source` and a password-free `external_auth` view (`ExternalAuthSummaryDto` with `password_set`)

## 6. api integration tests

- [x] 6.1 Add a dockerized MSSQL fixture for integration tests (`testcontainers-modules` `mssql_server`; `tests/external_auth_login.rs` boots + seeds a real MSSQL — verified passing under arm64 emulation ~27s cold / ~9s warm)
- [x] 6.2 Test external login: success provisions shadow user + session; repeat login reuses the same `_id`; identity columns coerced (INT `emp_id`→"1001", NVARCHAR `name`→"王小明"); credentials bound as params (`@P1`/`@P2`, never interpolated)
- [x] 6.3 Test error semantics: no matching row → `INVALID_CREDENTIALS` (wrong password); column-not-found → distinct connectable diagnostic. (Unreachable-DB/disabled-shadow paths exercised by unit-level provider mapping; not re-booted per-case to save container time.)
- [x] 6.4 Test config/gating (`tests/external_auth_config.rs`, no container): save validation (missing placeholder / empty col), switch-without-config rejected, member forbidden, default org reports `internal`; `EXTERNAL_AUTH_MODE` on create + test-login dry-run (success + column-not-found) covered in the container test

## 7. admin-web

- [x] 7.1 Extend API types for `auth_source`, the password-free `external_auth` view, test-login request/response, and the new error codes
- [x] 7.2 Added dedicated settings sub-page `pages/settings/auth.vue` (admin-only, redirect non-admin): auth-source radio, MSSQL connection fields, write-only password field (`已設定` + 變更), query template, `key_col` / `display_col`
- [x] 7.3 Built the 試登入 panel: test account/password inputs calling the dry-run endpoint, rendering resolved `external_key` / `display_name` or the specific diagnostic
- [x] 7.4 Added the mode-switch confirmation modal warning that existing accounts will be unable to log in (data preserved, reversible); wire save through the settings composable
- [x] 7.5 Added the 驗證來源 entry link (dashboard admin tools) card on the dashboard (`pages/index.vue`) linking to the sub-page and showing current mode
- [x] 7.6 Made the App-users page (`pages/app-users/index.vue`) external-mode aware: hide 新增 / 重設密碼, show 唯一識別 / 名稱 / 最後登入 columns, keep 停用, and add the “needs first login” empty state

## 8. Docs & verification

- [x] 8.1 DEPLOY.md: added `BANDAO_SECRET_KEY` to the env matrix + an "External-database App-user auth" subsection covering the `tiberius` dependency, the prod network-reachability known limitation, and the setup flow
- [~] 8.2 `cargo test` (full suite incl. live MSSQL integration) ✓ all green; `cargo clippy --all-targets` ✓ clean; admin-web `nuxt typecheck` ✓ + production build ✓ (no eslint configured in admin-web). **Remaining: interactive browser click-through of the settings + 試登入 flow against a real MSSQL — a manual smoke needing a browser session (deferred to the user).**
