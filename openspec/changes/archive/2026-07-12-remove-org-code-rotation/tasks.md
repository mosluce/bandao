## 1. api

- [x] 1.1 Delete `handlers/orgs.rs::rotate_code`
- [x] 1.2 Delete `db/orgs.rs::rotate_code`
- [x] 1.3 Remove the `POST /orgs/me/code/rotate` route registration in `handlers/mod.rs`
- [x] 1.4 Delete the `RotateCodeResponse` DTO (in `handlers/orgs.rs`); also removed the now-unused `ORG_CODE_RETRIES` const and `org_code` import from `handlers/orgs.rs` (a separate copy of `ORG_CODE_RETRIES` already exists in `handlers/auth.rs` for Org creation, untouched)
- [x] 1.5 Delete `tests/orgs_rotate.rs`
- [x] 1.6 Confirmed `auth::org_code::generate()` (used in `handlers/auth.rs` Org creation) and `is_well_formed()` (used in `auth/slug.rs` join-input classification) are still referenced and untouched

## 2. admin-web

- [x] 2.1 `pages/index.vue`: removed the `rotating` / `showRotateConfirm` / `rotateError` refs, the `rotateCode()` function, the now-unused `RotateCodeResponse` import, and the rotate button + confirmation panel markup
- [x] 2.2 `types/api.ts`: removed `RotateCodeResponse`
- [x] 2.3 Confirmed via `grep -rn "rotate" pages/ composables/ types/` — only remaining matches are API-token rotate (`useApiTokens.ts`, `settings/api-tokens.vue`, `ApiTokenSecretResponse`'s `rotated_at`), a distinct feature, untouched

## 3. Docs & verification

- [x] 3.1 `cargo test` (full suite) clean; `cargo clippy --all-targets` clean; `cargo fmt` applied. Found and fixed two pre-existing tests that used `/orgs/me/code/rotate` only as a convenient admin-only probe endpoint (unrelated to the rotate feature itself, not listed in this change's original scope): `tests/auth_role_lookup_per_request.rs::role_demotion_takes_effect_on_next_request_without_relogin` and `tests/zero_org_state.rs::org_scoped_endpoints_reject_with_no_active_org` both swapped to `GET /orgs/me/join-requests` as the probe. Also fixed a stale route-path reference in a doc-comment in `auth/slug.rs`'s `router_first_level_paths_are_reserved` test (`/orgs/me/code/rotate` → `/orgs/me/owner`).
- [x] 3.2 admin-web `pnpm typecheck` + `pnpm test` (38/38 passed) + `pnpm build` all clean
- [x] 3.3 Confirmed via the freshly built client bundle (`grep -rl "輪替\|rotateCode\|showRotateConfirm\|RotateCodeResponse" .output/public/_nuxt/*.js` → no matches) that no rotate button, confirm panel, or related text ships anywhere — this caught a second stray reference the earlier grep missed (a "管理員工具" section subtitle describing rotation, now reworded). Confirmed via curl against the running dev api that org creation still yields a working `org.code`: registered a fresh Org, a new member successfully submitted a join request with that code, and a freshly created AppUser successfully logged in via `/app/auth/login` with that code — while `POST /orgs/me/code/rotate` no longer resolves to the old handler.
