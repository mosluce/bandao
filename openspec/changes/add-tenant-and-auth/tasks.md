## 1. Repo bootstrap

- [x] 1.1 Add root `README.md` with project overview, module layout, dev prerequisites (Rust toolchain, Node, MongoDB)
- [x] 1.2 Add root `.gitignore` covering Rust (`target/`), Node (`node_modules/`, `.nuxt/`, `.output/`), env files, OS noise
- [x] 1.3 Add root `.editorconfig` for consistent indent / line endings across modules
- [x] 1.4 Create empty `app/.gitkeep` placeholder to reserve Flutter slot

## 2. MongoDB local dev setup

- [x] 2.1 Add `docker-compose.yml` at repo root running MongoDB 7.x with named volume and exposed port
- [x] 2.2 Document `docker compose up -d mongodb` flow and connection string in `README.md`

## 3. API project scaffold (`api/`)

- [ ] 3.1 `cargo init api/` and pin Rust toolchain via `rust-toolchain.toml`
- [ ] 3.2 Add core dependencies: `tokio`, `axum`, `tower`, `serde`, `serde_json`, `thiserror`, `tracing`, `tracing-subscriber`, `dotenvy`
- [ ] 3.3 Add domain dependencies: `mongodb`, `bson`, `argon2`, `nanoid`, `time`, `uuid`
- [ ] 3.4 Add dev dependencies: `tokio-test`, `testcontainers`, `reqwest` (for integration tests)
- [ ] 3.5 Wire up `main.rs` with config load (env), tracing init, axum router stub, graceful shutdown
- [ ] 3.6 Add `Config` struct (Mongo URI, listen addr, session TTL, cookie domain, secure flag) loaded from env with sane defaults

## 4. Database layer

- [ ] 4.1 Implement `Db` struct wrapping `mongodb::Database`, with constructor that connects and pings
- [ ] 4.2 Implement `ensure_indexes()` idempotently creating: `orgs.code` unique, `dashboard_users.email` unique, `dashboard_sessions.expires_at` TTL
- [ ] 4.3 Define `Org`, `DashboardUser`, `DashboardSession` structs with serde + bson derives
- [ ] 4.4 Implement `OrgRepository` (create, find_by_id, find_by_code, rotate_code)
- [ ] 4.5 Implement `DashboardUserRepository` (create, find_by_email, find_by_id, list_admins_in_org, update_role)
- [ ] 4.6 Implement `DashboardSessionRepository` (create, find_by_token, delete_by_token, touch_expires)

## 5. Auth primitives

- [ ] 5.1 Implement `password::hash(plain) -> String` and `password::verify(plain, hash) -> bool` using argon2id with OWASP-recommended params
- [ ] 5.2 Implement `org_code::generate() -> String` producing 10-char nanoid from custom alphabet
- [ ] 5.3 Implement `session_token::generate() -> String` producing 32-byte base64url random token

## 6. Auth middleware & request context

- [ ] 6.1 Define `AuthContext { user_id, org_id, role }` and an axum extractor that pulls it from request extensions
- [ ] 6.2 Implement middleware that reads session cookie → loads session → loads user → injects `AuthContext` (or rejects 401 if missing/invalid/expired)
- [ ] 6.3 Implement `RequireAdmin` extractor wrapping `AuthContext` and rejecting non-admins with 403

## 7. Error handling

- [ ] 7.1 Define `ApiError` enum with named variants (`EmailTaken`, `InvalidOrgCode`, `LastAdmin`, `Unauthorized`, `Forbidden`, `NotFound`, `InvalidCredentials`, `Internal`)
- [ ] 7.2 Implement `IntoResponse` for `ApiError` producing `{ error: { code, message } }` with appropriate HTTP status
- [ ] 7.3 Add `Result<T> = Result<T, ApiError>` alias used throughout handlers

## 8. Auth endpoints

- [ ] 8.1 `POST /auth/register` (mode=create): validate input → create Org → create admin user → create session → set cookie → 200
- [ ] 8.2 `POST /auth/register` (mode=join): validate input → find Org by code → create member user → create session → set cookie → 200; rejects unknown code
- [ ] 8.3 `POST /auth/login`: lookup user by email → verify password → create session → set cookie → 200; rejects wrong creds with generic error
- [ ] 8.4 `POST /auth/logout`: delete session row → clear cookie → 204
- [ ] 8.5 `GET /me`: return `{ user, org, role }` from `AuthContext`

## 9. Admin endpoints

- [ ] 9.1 `POST /orgs/me/code/rotate`: admin-only, generate new Org code (retry on collision), persist, return new code
- [ ] 9.2 `PATCH /dashboard-users/:id/role`: admin-only, validate target is in same Org, enforce last-admin invariant via `update_role` repository method, return updated user

## 10. API integration tests

- [ ] 10.1 Set up `tests/common.rs` with a `TestApp` helper that boots a fresh MongoDB container via testcontainers and a fresh axum router on a random port
- [ ] 10.2 Test: register create-mode happy path → assert Org and admin user exist, cookie is set
- [ ] 10.3 Test: register join-mode happy path → assert member user with correct org_id, role
- [ ] 10.4 Test: register rejects `EMAIL_TAKEN` and `INVALID_ORG_CODE`
- [ ] 10.5 Test: login happy path + wrong-password / unknown-email both return generic `INVALID_CREDENTIALS`
- [ ] 10.6 Test: logout invalidates session (next `/me` returns 401)
- [ ] 10.7 Test: expired session is rejected and cookie cleared
- [ ] 10.8 Test: rotate code as admin succeeds; as member returns 403; previous code becomes invalid for join
- [ ] 10.9 Test: role change happy path; cross-Org target returns `NOT_FOUND`; demoting last admin returns `LAST_ADMIN`

## 11. Admin-web project scaffold (`admin-web/`)

- [ ] 11.1 `npx nuxi init admin-web` (Nuxt 3 + TypeScript), prune defaults to a clean baseline
- [ ] 11.2 Enable `tsconfig` strict mode and matching ESLint config; commit lockfile
- [ ] 11.3 Add `runtimeConfig` for `apiBaseUrl`; add `.env.example` with default `http://localhost:8080`
- [ ] 11.4 Configure `$fetch` wrapper composable `useApi()` that includes credentials and points at `apiBaseUrl`

## 12. Auth state & API client

- [ ] 12.1 Define typed API request/response models in `admin-web/types/api.ts` (mirror Rust DTOs by hand for MVP; OpenAPI codegen → ROADMAP)
- [ ] 12.2 Implement composable `useAuth()` exposing `me`, `login`, `logout`, `register`, with reactive state and `refresh()` calling `/me`
- [ ] 12.3 Add route middleware `auth` that redirects unauthenticated visitors to `/login` and `guest` that redirects authenticated visitors away from `/login` and `/register`

## 13. Admin-web pages

- [ ] 13.1 `/register` page with two tabs: "Create Org" (org_name + email + password) and "Join Org" (org_code + email + password). Pre-fill `org_code` from `?code=` query
- [ ] 13.2 `/login` page (email + password), generic error message on failure
- [ ] 13.3 `/` (index) page protected by `auth` middleware, shows current Org name, Org code (with copy button + invite-link hint), user email, role, logout button
- [ ] 13.4 Admin-only section on `/`: "Rotate Org code" button (with confirm dialog) — visible only when `role=admin`
- [ ] 13.5 Admin-only `/members` page listing other dashboard users in the Org with role and a promote/demote action; disabled when target would violate last-admin invariant (or rely on server error)

## 14. Smoke validation

- [ ] 14.1 Boot api + mongodb + admin-web locally and walk the flow: register create-Org → see /me → copy invite link → register join-Org in private window → see /members listing
- [ ] 14.2 Verify cookie is HttpOnly + Secure (in dev with HTTPS off, document the exception) and `SameSite=Lax`
- [ ] 14.3 Verify rotating the Org code invalidates the previous code for join

## 15. CI baseline

- [ ] 15.1 Add `.github/workflows/api.yml` running `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` (with MongoDB service container)
- [ ] 15.2 Add `.github/workflows/admin-web.yml` running `pnpm install`, `pnpm typecheck`, `pnpm lint`, `pnpm build`

## 16. Docs

- [ ] 16.1 Update `README.md` with end-to-end quick-start commands (compose up, api run, admin-web dev)
- [ ] 16.2 Add `api/README.md` covering env vars and test prerequisites
- [ ] 16.3 Add `admin-web/README.md` covering env vars and dev/build commands
