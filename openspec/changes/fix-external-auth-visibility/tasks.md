## 1. api

- [x] 1.1 Split `OrgDto::from_org` into `OrgDto::from_org_as_admin` / `OrgDto::from_org_as_non_admin`, both delegating to a private `OrgDto::build(org, include_external_auth: bool)` helper. Old `from_org` name removed entirely (per design D2, no compat shim).
- [x] 1.2 `build_auth_response`: each membership's `OrgDto` picks the variant based on that pair's own `m.role`; `current_org`'s `OrgDto` picks based on the already-computed `role` (`Some(Role::Admin)` → admin variant, else non-admin)
- [x] 1.3 `handlers/app_auth.rs`'s `login` and `me` now call `OrgDto::from_org_as_non_admin` unconditionally
- [x] 1.4 `handlers/orgs.rs::transfer_owner` and `handlers/external_auth.rs::configure`: renamed to `OrgDto::from_org_as_admin` — **task description was wrong about "no code change expected"**, since the old unqualified `from_org` name was removed entirely (D2), every call site needed a rename regardless of whether its access level changed. Both are still `RequireAdmin`-gated end to end, so behavior is unchanged.
- [x] 1.5 `cargo build` compiles clean; `grep -rn "OrgDto::from_org(" src/` (the old unqualified name) returns zero matches — confirms no call site was missed

## 2. api integration tests

New file `api/tests/org_dto_external_auth_visibility.rs` (5 tests, all passing). Seeds `external_auth` directly via `db.orgs.set_auth_config(..., OrgAuthSource::Internal, Some(&fixture))`, keeping `auth_source = internal` so AppUser password login works without needing a real MSSQL connection — `Org::external_auth()` reads the config doc independent of `auth_source`.

- [x] 2.1 `GET /me`: admin's response includes `external_auth` for an Org with a configured external-auth doc; member's response (both `current_org` and `memberships[].org`) omits the key entirely (assert key absence, not `null`)
- [x] 2.2 `POST /auth/login`: same admin-vs-member assertion. `POST /auth/register` covered as a light regression only (a brand-new Org can't have a pre-existing `external_auth` doc, so it can't exercise the role-gating logic — just confirms the response shape is unaffected)
- [x] 2.3 `POST /app/auth/login` and `GET /app/me`: AppUser session response never includes `external_auth`, even for an Org with `external_auth` configured
- [x] 2.4 Regression: `POST /orgs/me/owner` (transfer_owner, via a promoted second admin) and `POST /orgs/me/external-auth` (configure — router wires it as POST despite the handler doc-comment saying PUT) responses still include `external_auth` for the admin caller

## 3. Docs & verification

- [x] 3.1 `cargo test` (full suite) clean; `cargo clippy --all-targets` clean; `cargo fmt` applied
- [x] 3.2 Manual spot-check against a local dev Org with `auth_source == external_db` configured: log in as admin vs as a member, diff the raw `/me` JSON to confirm the field is truly absent for the member. Confirmed via curl against the running dev server (`cargo run`) — configured external-auth as admin, registered+approved a member, diffed `/me` JSON: `external_auth` present in both `current_org` and `memberships[].org` for admin, entirely absent for member; every other diff line was the expected identity/role difference.
