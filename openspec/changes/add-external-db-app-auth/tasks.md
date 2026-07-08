## 1. Data model & config (api)

- [x] 1.1 Add `AppUserAuthSource` enum (`Internal` | `External`) to `domain`; make `AppUser.password_hash: Option<String>` and add `auth_source` + `external_key: Option<String>` (also made `username`/`username_lower`/`created_by_dashboard_user_id` optional — absent for shadow users)
- [x] 1.2 Add `auth_source` (default `Internal`) and `external_auth` sub-document types to the `Org.settings` model; define `ExternalAuthConfig { driver, host, port, database, username, password_encrypted, query, key_col, display_col }` (+ `Org::auth_source()` / `Org::external_auth()` accessors)
- [ ] 1.3 Introduce a symmetric AEAD helper in `api/` for the `external_auth` connection password — `api/` currently has NO reversible crypto (only argon2/rand), so add a dep (e.g. `chacha20poly1305`/`aes-gcm`) + a key source env (e.g. `BANDAO_SECRET_KEY`, added to `config.rs` and the DEPLOY.md env matrix); encrypt on write, decrypt only in-memory at auth/test time; never log or return plaintext — **BLOCKED: needs key-source naming + rotation decision**
- [x] 1.4 Add the partial/sparse unique index `(org_id, external_key)` for external shadow users; keep the existing `(org_id, username_lower)` unique index for internal (migrated it to PARTIAL so shadow users' missing `username_lower` don't collide on null)
- [x] 1.5 Update `app_users` repository: `find_by_org_and_external_key`, `upsert_shadow(org_id, external_key, display_name)`, and adjust list to include external users; ensure DTO carries `auth_source` / `external_key`

## 2. Auth provider abstraction (api)

- [x] 2.1 Define `AppAuthProvider` trait (`authenticate(account, password) -> Result<AppUser, AuthProviderError>`; providers resolve to the local row that anchors the session — cleaner than a bare `ExternalIdentity`) + `AuthProviderError { InvalidCredentials, Unavailable(diagnostic) }`
- [x] 2.2 Implement `InternalProvider` wrapping the current Mongo + password-hash flow behind the trait
- [x] 2.3 Add a provider registry (`provider_for`) that selects from `Org.auth_source` (+ `external_auth.driver`); unsupported driver / missing config → `Unavailable`
- [x] 2.4 Add `ApiError` variants `EXTERNAL_AUTH_UNAVAILABLE` (503) and `EXTERNAL_AUTH_MODE` (409); wire into the outward error format

## 3. MSSQL provider (api)

- [ ] 3.1 Add `tiberius` dependency (+ async TCP glue) to `api/Cargo.toml`
- [ ] 3.2 Implement `MssqlProvider`: connect with timeout, bind `@account` / `@password` as parameters (never string-interpolate), run `query`, read `key_col` / `display_col` from the single result row
- [ ] 3.3 Map provider outcomes: 0 rows → credential failure; connect/query/column errors → typed unavailability with a specific diagnostic for test-login; ensure password never reaches logs/errors
- [ ] 3.4 Implement `external_auth` config validation: `query` contains both `@account` and `@password`; `key_col` and `display_col` non-empty

## 4. Login & shadow provisioning (api)

- [x] 4.1 Rewrite `POST /app/auth/login` to resolve the Org, select the provider, and delegate credential verification (internal path verified: all 8 login integration tests pass; external_db currently returns `EXTERNAL_AUTH_UNAVAILABLE` via the stubbed MSSQL provider)
- [ ] 4.2 On external success, upsert the shadow AppUser `(org_id, external_key)` (create with `auth_source=external`, `password_hash=None`, `needs_password_change=false`; else refresh `display_name`/`last_login_at`), enforce `status==active`, then issue the session — repo `upsert_shadow` ready (1.5); wiring lands with the real MSSQL provider (group 3)
- [x] 4.3 Collapse credential failures to `INVALID_CREDENTIALS`; surface provider unavailability as `EXTERNAL_AUTH_UNAVAILABLE`; keep the internal path behavior-identical

## 5. Org auth-source & test-login endpoints (api)

- [ ] 5.1 Extend `PATCH /orgs/me/settings` (or add a dedicated route) so admins can set `auth_source` and the `external_auth` config; validate config before persisting; reject `external_db` without a valid config; write `password_set` semantics (never echo the password)
- [ ] 5.2 Gate `POST /app-users` and `POST /app-users/:id/password-reset` with `EXTERNAL_AUTH_MODE` while `auth_source == external_db`; keep `PATCH` disable working in both modes
- [ ] 5.3 Implement `POST /orgs/me/external-auth/test-login` (admin-only, org-scoped, dry-run): run the full provider flow, return resolved identity or a specific diagnostic, create no session / no shadow row, never log the test password
- [ ] 5.4 Update `OrgDto` / settings DTOs to expose `auth_source` and a password-free `external_auth` view (`password_set`)

## 6. api integration tests

- [ ] 6.1 Add a dockerized MSSQL fixture for integration tests (per AGENTS.md: hit a real DB, no mocks)
- [ ] 6.2 Test external login: success provisions shadow user + session; repeat login reuses the same `_id` and refreshes `display_name`; parameter binding neutralizes a SQL-injection-style account
- [ ] 6.3 Test error semantics: no matching row → `INVALID_CREDENTIALS`; unreachable DB / bad query → `EXTERNAL_AUTH_UNAVAILABLE`; disabled shadow user → `INVALID_CREDENTIALS`
- [ ] 6.4 Test config/gating: save validation (missing placeholder / empty cols), `EXTERNAL_AUTH_MODE` on create & reset, member forbidden, and the test-login dry-run (success + column-not-found diagnostic, no writes)

## 7. admin-web

- [ ] 7.1 Regenerate/extend API types for `auth_source`, the password-free `external_auth` view, test-login request/response, and the new error codes
- [ ] 7.2 Add a dedicated settings sub-page `pages/settings/auth.vue` (admin-only, redirect non-admin): auth-source radio, MSSQL connection fields, write-only password field (`已設定` + 變更), query template, `key_col` / `display_col`
- [ ] 7.3 Build the 試登入 panel: test account/password inputs calling the dry-run endpoint, rendering resolved `external_key` / `display_name` or the specific diagnostic
- [ ] 7.4 Add the mode-switch confirmation modal warning that existing accounts will be unable to log in (data preserved, reversible); wire save through the settings composable
- [ ] 7.5 Add the lightweight 驗證來源 entry card on the dashboard (`pages/index.vue`) linking to the sub-page and showing current mode
- [ ] 7.6 Make the App-users page (`pages/app-users/index.vue`) external-mode aware: hide 新增 / 重設密碼, show 唯一識別 / 名稱 / 最後登入 columns, keep 停用, and add the “needs first login” empty state

## 8. Docs & verification

- [ ] 8.1 Note the `tiberius` dependency, the prod-reachability known limitation, and the external-auth setup flow in the relevant module README(s) / DEPLOY.md
- [ ] 8.2 Run `cargo test` (incl. new MSSQL integration tests), `cargo clippy`, `pnpm lint`/typecheck; smoke the admin-web settings + test-login flow against a real MSSQL before archiving
