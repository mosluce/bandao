## 1. Domain & DB schema

- [x] 1.1 Add `slug: Option<String>` and `slug_changed_at: Option<DateTime>` to `domain::Org`
- [x] 1.2 Define `domain::OrgSlugReservation { id, slug, org_id, expires_at: Option<DateTime>, created_at }` struct with serde + bson derives (single collection covers both active reservation `expires_at=None` and grace `expires_at=Some`)
- [x] 1.3 Update `db::ensure_indexes()` to add: `orgs.slug` sparse unique, `slug_reservations.slug` unique, `slug_reservations.expires_at` TTL (`expireAfterSeconds: 0`, only docs with non-null expires_at are reaped), `slug_reservations.org_id`

## 2. Slug validation primitives

- [x] 2.1 Create `auth::slug` module with `RESERVED_SLUGS: &[&str]` constant covering API path roots, system identifiers, and `argus`
- [x] 2.2 Implement `slug::normalize(input: &str) -> String` (lowercase + trim)
- [x] 2.3 Implement `slug::validate(normalized: &str) -> Result<(), SlugValidationError>` returning `InvalidFormat` for `^[a-z0-9]{2,24}$` failure or `Reserved` for reserved-list hit
- [x] 2.4 Add unit tests covering normalize + validate happy paths, format failures, reserved hits, and the invariant "every first-level path of the existing axum router belongs to RESERVED_SLUGS"

## 3. Repository layer

- [x] 3.1 Add `OrgRepository::find_by_slug(slug: &str) -> Result<Option<Org>>` querying active slug field
- [x] 3.2 Add `OrgSlugReservationRepository` with: `find_by_slug(slug)`, `try_insert_active(slug, org_id) -> Result<(), SlugTaken>` (relies on unique index → duplicate key = SLUG_TAKEN), `move_to_grace(slug, org_id, expires_at)` (update active row to grace), `delete_by_id(id)` (rollback helper)
- [x] 3.3 Add `OrgRepository::set_slug(org_id, slug, slug_changed_at)` and `OrgRepository::clear_slug(org_id, slug_changed_at)`
- [x] 3.4 Implement `auth::slug::set_slug_atomic(org_id, new_slug_or_none, now, grace_ttl)` orchestration: (a) for SET: try_insert_active new reservation → on duplicate-key return SlugTaken; (b) move old active reservation to grace with expires_at = now + grace_ttl (if exists); (c) update orgs doc with new slug + slug_changed_at; (d) on failure of (b) or (c), best-effort rollback by deleting the just-inserted reservation. CLEAR variant: skip step (a), only move old → grace and null orgs.slug.

## 4. Error model

- [x] 4.1 Add `ApiError` variants: `InvalidSlugFormat`, `SlugReserved`, `SlugTaken`, `SlugChangeTooSoon { retry_after: DateTime }`
- [x] 4.2 Map each to `IntoResponse`: 400 `INVALID_SLUG_FORMAT`, 400 `SLUG_RESERVED`, 409 `SLUG_TAKEN`, 429 `SLUG_CHANGE_TOO_SOON` (body includes `retry_after` ISO-8601)
- [x] 4.3 Confirm existing `INVALID_ORG_CODE` covers all "input does not resolve" cases (no new error code needed for unknown slug at register)

## 5. Slug change endpoints

- [x] 5.1 Add `handlers::orgs::set_slug` (`POST /orgs/me/slug`, `RequireAdmin`): normalize input → validate → enforce 30-day rate limit (skip if `slug_changed_at` is None) → call `set_slug_atomic` → return `{ slug }`
- [x] 5.2 Add `handlers::orgs::clear_slug` (`DELETE /orgs/me/slug`, `RequireAdmin`): enforce rate limit → call `set_slug_atomic` with new=None → 204 No Content
- [x] 5.3 Wire both into the axum router in `main.rs`
- [x] 5.4 Compute `retry_after = slug_changed_at + 30 days` for `SlugChangeTooSoon` responses

## 6. Join lookup change

- [x] 6.1 Implement `auth::slug::resolve_org_for_join(input: &str) -> Result<Org, ApiError>` (lives in `auth::slug`, not `auth::join`):
  - rejects with `INVALID_ORG_CODE` if input matches neither slug nor code regex
  - if slug-shaped: query slug_reservations by slug; if found and (expires_at None OR expires_at>now), resolve org_id via `find_by_id`; otherwise fall through to INVALID_ORG_CODE
  - if code-shaped: try `find_by_code`
- [x] 6.2 Update `handlers::auth::register` join branch to call `resolve_org_for_join` instead of inline `find_by_code`
- [x] 6.3 Confirm rotate code path is untouched (verified: handlers::orgs::rotate_code and db::orgs::rotate_code unchanged)

## 7. Integration tests

- [x] 7.1 `auth_register_by_slug.rs`: register-create then admin sets slug → second user joins with `mode=join` + slug → assert member belongs to same Org
- [x] 7.2 `auth_register_by_grace_slug.rs`: admin sets slug `acme` → fast-forward 35 days then change to `acmecorp` (use Mongo manipulation to backdate `slug_changed_at`) → join with `acme` still works → 31 days later (or backdate grace expires_at) join with `acme` returns INVALID_ORG_CODE
- [x] 7.3 `orgs_slug_set.rs`: happy path; INVALID_SLUG_FORMAT (`a`, `acme-corp`, 25-char); SLUG_RESERVED (`admin`, `argus`, `auth`); SLUG_TAKEN (active and grace); rate-limit (`SLUG_CHANGE_TOO_SOON` 10 days after change, success after 30+ days); `FORBIDDEN` for member; first-set bypasses rate limit
- [x] 7.4 `orgs_slug_clear.rs`: clear puts old slug into grace; another Org cannot claim during grace; second clear within 30 days returns SLUG_CHANGE_TOO_SOON
- [x] 7.5 `orgs_slug_lookup_format.rs`: register `mode=join` with garbage input (e.g. `"!!!"`, empty, mixed case `"AcMe"` that fails neither regex due to case) → INVALID_ORG_CODE without DB lookup
- [x] 7.6 Verify reserved-list invariant test from 2.4 still passes

## 8. Admin-web — types & client

- [x] 8.1 Update `types/api.ts`: add `OrgDto.slug?: string`, `OrgDto.slug_changed_at?: string`, `SetSlugRequest`, `SetSlugResponse`
- [x] 8.2 Map new error codes (`INVALID_SLUG_FORMAT`, `SLUG_RESERVED`, `SLUG_TAKEN`, `SLUG_CHANGE_TOO_SOON`) to `ApiError` (already generic in useApi onResponseError — pass-through covers all codes)
- [x] 8.3 Add `setOrgSlug(slug)` and `clearOrgSlug()` via new `useOrgSlug` composable (kept separate from `useApi` to avoid changing existing call shape — pages call `const { setOrgSlug, clearOrgSlug } = useOrgSlug()`)

## 9. Admin-web — UI

- [x] 9.1 In `pages/index.vue` add a vanity slug section beside the existing org code: shows current slug (or "未設定"), Edit button opens inline form with validation hint (format / reserved / taken / cool-down)
- [x] 9.2 Add Clear button that opens a confirm dialog warning about grace period
- [x] 9.3 Compute invite link: prefer slug when present, otherwise code; expose Copy button
- [x] 9.4 When `SLUG_CHANGE_TOO_SOON` returned, show countdown with `retry_after` parsed from response (extended `ApiError` to carry `retryAfter`; UI shows the localized timestamp instead of a live countdown — the countdown ROADMAP item left for later)
- [x] 9.5 Disable both buttons for non-admins (Edit/Clear buttons sit inside `v-if="auth.isAdmin.value"` template — non-admins see slug value but no controls)

## 10. Docs & polish

- [x] 10.1 Update `api/README.md` with the two new endpoints + reserved-list note
- [x] 10.2 Update `admin-web/README.md` with vanity slug UI behavior
- [x] 10.3 No env vars added — confirmed (grace TTL is a 30-day code constant per Decision 5; no `.env.example` change)
- [x] 10.4 Run full `cargo test` + `pnpm typecheck` + `pnpm build` and confirm green (cargo: 12 unit + 28 integration tests pass; admin-web typecheck + build pass)

## 11. Smoke validation

- [x] 11.1 Boot api + mongodb + admin-web; admin sets slug `acme`; copy invite link → registers in private window → joins
- [x] 11.2 Change slug to `acmecorp`; old invite link with `acme` still works; another Org cannot claim `acme`
- [x] 11.3 Try set within 30 days → see countdown; try reserved word → see SLUG_RESERVED error; try `Acme` → server normalizes and accepts as `acme`
