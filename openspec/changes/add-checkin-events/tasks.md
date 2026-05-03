## 1. Domain & DB schema

- [x] 1.1 Add `CheckinEventType` enum (`clock_in | clock_out | transfer_out | transfer_in`) and `AppUserCheckinStatus` enum (`off_duty | on_site | in_transit`) to `api/src/domain.rs` with snake_case serde.
- [x] 1.2 Add `EventSource` enum (`app | admin_force`) and `EventInitiatorKind` enum (`app_user | dashboard_user`) to `api/src/domain.rs`.
- [x] 1.3 Add `EventLocation` struct: `coordinates: GeoPoint { lat: f64, lng: f64 }`, `accuracy_meters: Option<f64>`, `region_name: Option<String>`, `manual_label: Option<String>`.
- [x] 1.4 Add `CheckinEvent` struct: `id, org_id, app_user_id, event_type, occurred_at_client, occurred_at_server, source, initiated_by_kind, initiated_by_id, location, reason: Option<String>` (reason only set on admin_force).
- [x] 1.5 Add `CheckinUserStatus` struct: `app_user_id (PK), org_id, status, current_shift_started_at: Option<DateTime>, last_event_id: Option<ObjectId>, updated_at`.
- [x] 1.6 Extend `Org` in `api/src/domain.rs`: add `timezone: String` (default `"Asia/Taipei"`) and `OrgCheckinSettings { transfer_enabled: bool }` reachable as `Org.settings.checkin` via `bson::Document` access (or migrate to a typed struct on `Org`). Pick the simpler.
- [x] 1.7 Create `api/src/db/checkin_events.rs` with `CheckinEventRepository`: `create`, `find_by_id`, `latest_for_app_user`, `list_by_app_user_paginated(cursor, limit)`, `list_by_org_paginated`, `list_by_app_user_after(after_client_time)`.
- [x] 1.8 Create `api/src/db/checkin_user_status.rs` with `CheckinUserStatusRepository`: `init_off_duty(app_user_id, org_id)`, `find(app_user_id)`, `list_by_org(org_id)`, `update_to(app_user_id, expected_prior_status, new_status, current_shift_started_at, last_event_id)` — `update_to` uses conditional `find_one_and_update` matching the prior status; returns `None` (race) on mismatch.
- [x] 1.9 In `api/src/db/mod.rs` create the two collections and indexes:
  - `checkin_events`: `(app_user_id, occurred_at_client desc)`, `(org_id, occurred_at_client desc)`
  - `checkin_user_status`: unique on `app_user_id`, secondary on `(org_id, status)`
- [x] 1.10 In `api/src/db/orgs.rs` add `update_settings(org_id, settings_patch: { transfer_enabled?, timezone? })` returning the updated Org. Handle absent `Org.settings.checkin` by treating it as defaults at read time.

## 2. Reverse geocoder

- [x] 2.1 Create `api/src/services/mod.rs` and `api/src/services/reverse_geocoder.rs` defining `#[async_trait] pub trait ReverseGeocoder { async fn lookup(&self, lat: f64, lng: f64) -> Option<String>; }`.
- [x] 2.2 Create `api/src/services/reverse_geocoder/nominatim.rs` (or `nominatim_geocoder.rs`) implementing the trait against `https://nominatim.openstreetmap.org/reverse`. Use `reqwest::Client` with a User-Agent string `"argus-api/<version> (<contact>)"`, 2-second timeout, accept-language preference (configurable, default `"zh-TW,en"`). All errors collapse to `None`.
- [x] 2.3 Wire a default `ReverseGeocoder` instance into `AppState` (probably as `Arc<dyn ReverseGeocoder>`). Keep the type erased so tests can substitute a stub.
- [x] 2.4 Add a stub `StaticReverseGeocoder { fixed: Option<String> }` for tests.

## 3. AppUser-facing handlers (`/app/checkin/*`)

- [x] 3.1 `POST /app/checkin/events` in `api/src/handlers/app_checkin.rs`:
  - Validate request body: `event_type`, `lat`, `lng`, `accuracy?`, `manual_label?` (length 1–120 if present), `occurred_at_client` (RFC3339).
  - Look up caller's `CheckinUserStatus` (init off_duty if missing).
  - Run state-machine validation; reject with `INVALID_TRANSITION` on illegal pair.
  - Run `OUT_OF_ORDER` check against latest stored event for this AppUser (strict `<=` rejection).
  - Run `TRANSFER_DISABLED` check against `Org.settings.checkin.transfer_enabled` for transfer events.
  - Call `ReverseGeocoder::lookup`; on `None`, store `region_name = null`.
  - Insert event row; then call `update_to(app_user_id, prior_status, new_status, ...)`. If race (returns `None`), best-effort delete the event row and respond `INVALID_TRANSITION`.
  - Return `201 Created` with `{ event, status }`.
- [x] 3.2 `GET /app/checkin/status` returns `{ status, current_shift_started_at, last_event }`.
- [x] 3.3 `GET /app/checkin/events` returns cursor-paginated own events (newest first by `occurred_at_client`, default 50, accepts `cursor` query param as event id of the last item from previous page).
- [x] 3.4 Wire `/app/checkin/*` routes; AppUser middleware (`RequireAppUser`) covers them, applying the `needs_password_change` 423 gate transparently.

## 4. Admin-facing handlers (`/checkin/*` and `/orgs/me/settings`)

- [x] 4.1 `GET /checkin/users` in `api/src/handlers/checkin.rs`: admin-only, scoped to `current_org`. Returns array of `{ user, status, current_shift_started_at, last_event, has_skew_warning }`. Compute `has_skew_warning = |last_event.occurred_at_client - last_event.occurred_at_server| > 1 hour` per row when `last_event` exists.
- [x] 4.2 `GET /checkin/users/:id/events`: admin-only, scoped to `current_org`. Cross-Org id → `NOT_FOUND`. Cursor pagination by `occurred_at_client` desc.
- [x] 4.3 `POST /checkin/users/:id/force-checkout`: admin-only, scoped to `current_org`. Body `{ reason?: String (≤240 chars) }`. Reject if target's status is `off_duty` with `NOT_ON_DUTY`. Insert `clock_out` event with `source=admin_force`, `initiated_by_kind=dashboard_user`, `initiated_by_id=ctx.user_id`, `occurred_at_client=now`, `occurred_at_server=now`, `location` copied from target's last event, `manual_label="管理員強制收班"`, `reason` from body. Update status atomically.
- [x] 4.4 `PATCH /orgs/me/settings`: admin-only, scoped to `current_org`. Accepts `{ transfer_enabled?: bool, timezone?: String }`. If `transfer_enabled` is present, run state-lock check (count `checkin_user_status` where `org_id = current_org_id AND status != off_duty`); reject with `STATE_LOCKED { on_duty_count }` if count > 0. Validate `timezone` against IANA db; reject with `INVALID_TIMEZONE` on bad value. Apply the patch and return updated `OrgSettingsDto`.

## 5. New error variants

- [x] 5.1 In `api/src/error.rs` add: `InvalidTransition { from, attempted } (422, INVALID_TRANSITION)`, `TransferDisabled (403, TRANSFER_DISABLED)`, `OutOfOrder (409, OUT_OF_ORDER)`, `StateLocked { on_duty_count: u32 } (409, STATE_LOCKED)`, `NotOnDuty (409, NOT_ON_DUTY)`, `InvalidTimezone (400, INVALID_TIMEZONE)`. Update `IntoResponse` to render the structured fields.

## 6. DTO shapes

- [x] 6.1 `CheckinEventDto { id, app_user_id, event_type, occurred_at_client, occurred_at_server, source, initiated_by_kind, initiated_by_id, location, reason?, has_skew_warning }`.
- [x] 6.2 `CheckinUserStatusDto { app_user_id, status, current_shift_started_at, last_event?, has_skew_warning }`.
- [x] 6.3 `SubmitCheckinEventRequest { event_type, lat, lng, accuracy?, manual_label?, occurred_at_client }`.
- [x] 6.4 `ForceCheckoutRequest { reason?: String }`.
- [x] 6.5 `UpdateOrgSettingsRequest { transfer_enabled?, timezone? }` and `OrgSettingsDto { timezone, checkin: { transfer_enabled } }`.

## 7. AppUser status init when an AppUser is created

- [x] 7.1 Update `POST /app-users` (in `api/src/handlers/app_users.rs`) to also insert a `checkin_user_status` row with `status = off_duty` immediately after the AppUser row succeeds. If the status insert fails, roll back the AppUser create (best effort).
- [x] 7.2 Update tests `tests/app_users_create.rs` to assert the matching `checkin_user_status` row exists.

## 8. Startup repair (status drift safety net)

- [x] 8.1 Add a small startup task (or admin-only `POST /admin/checkin/repair-status`) that scans `checkin_user_status` against the latest event per AppUser and fixes any drift (e.g. status row says off_duty but latest event was clock_in, or vice versa). Document why this exists in `api/README.md`. MVP: only the startup form, run once per process start; admin endpoint is optional follow-up.

## 9. API integration tests

- [x] 9.1 `tests/checkin_state_machine.rs`: every legal transition returns success and updates `checkin_user_status` correctly; every illegal pair returns `INVALID_TRANSITION` and leaves state unchanged.
- [x] 9.2 `tests/checkin_multi_site.rs`: full cycle `clock_in → transfer_out → transfer_in → transfer_out → transfer_in → clock_out` succeeds with three `on_site` segments visible in the event list.
- [x] 9.3 `tests/checkin_out_of_order.rs`: second event with `occurred_at_client <= last_event.occurred_at_client` for the same AppUser is rejected `OUT_OF_ORDER`; per-AppUser scoping (Bob's old event doesn't conflict with Alice's newer event).
- [x] 9.4 `tests/checkin_dual_timestamps.rs`: client time far in the past or future is accepted; `has_skew_warning` flips at the 1-hour boundary; ordering and display use client time.
- [x] 9.5 `tests/checkin_transfer_toggle.rs`: with `transfer_enabled=false`, `transfer_out`/`transfer_in` rejected `TRANSFER_DISABLED`; `clock_in`/`clock_out` unaffected; toggling back to true unblocks.
- [x] 9.6 `tests/checkin_state_lock.rs`: PATCH `transfer_enabled` succeeds when all AppUsers `off_duty`; rejects `STATE_LOCKED` with `on_duty_count` when anyone on shift; PATCH `timezone` succeeds regardless of state.
- [x] 9.7 `tests/checkin_force_checkout.rs`: admin force-checkout succeeds for `on_site` and `in_transit` AppUsers; rejected `NOT_ON_DUTY` for `off_duty`; cross-Org rejected `NOT_FOUND`; `member` rejected `FORBIDDEN`; resulting event has `source=admin_force`, `initiated_by_kind=dashboard_user`, copied location, manual_label `"管理員強制收班"`, optional reason stored.
- [x] 9.8 `tests/checkin_geocoding_failsoft.rs`: with a stub geocoder returning `None`, events still record with `region_name = null`; with a stub returning `Some("Taipei City")`, events populate the field.
- [x] 9.9 `tests/checkin_admin_views.rs`: `GET /checkin/users` lists current_org AppUsers + status, excludes other Orgs, member rejected; `GET /checkin/users/:id/events` cursor pagination, cross-Org `NOT_FOUND`.
- [x] 9.10 `tests/checkin_appuser_scope.rs`: `GET /app/checkin/status` and `/events` only return caller's own data; cannot peek at another AppUser's history through the AppUser surface.
- [x] 9.11 `tests/checkin_app_user_status_init.rs`: creating an AppUser via `POST /app-users` immediately yields a `checkin_user_status` row with `status=off_duty`, ready for first `clock_in`.
- [x] 9.12 `tests/orgs_timezone.rs`: default new Org has `timezone="Asia/Taipei"`; PATCH to valid IANA value succeeds; invalid value rejected `INVALID_TIMEZONE`; member rejected `FORBIDDEN`; no DB timestamp changes after a TZ update.

## 10. admin-web

- [ ] 10.1 Add types to `admin-web/types/api.ts`: `CheckinEventType`, `AppUserCheckinStatus`, `EventSource`, `EventInitiatorKind`, `EventLocation`, `CheckinEventDto`, `CheckinUserStatusDto`, `SubmitCheckinEventRequest`, `ForceCheckoutRequest`, `UpdateOrgSettingsRequest`, `OrgSettingsDto`.
- [ ] 10.2 Add `composables/useCheckin.ts`: `listUsers()`, `listUserEvents(id, cursor?)`, `forceCheckout(id, reason?)`.
- [ ] 10.3 Add `composables/useOrgSettings.ts`: `update({ transfer_enabled?, timezone? })`. (Or fold into existing `useOrgSlug` / a new umbrella; pick the simplest.)
- [ ] 10.4 New page `pages/checkin/index.vue` (admin-only): live status board listing AppUsers grouped by status (on_site / in_transit / off_duty). Each row shows display_name, status badge, current_shift_started_at (rendered in Org timezone), last event location summary, and a skew-warning icon when applicable. Force-checkout button per non-`off_duty` row → confirm dialog with optional reason field. Polling every 30s. Watch `auth.currentOrg.value?.id` to refetch.
- [ ] 10.5 New page `pages/checkin/[appUserId].vue` (admin-only): single AppUser's event history with cursor pagination, each event showing event_type, time (Org TZ), location (region_name + manual_label + lat/lng tooltip), source badge (`app` / `管理員強制收班`), reason (when present), skew-warning icon when applicable.
- [ ] 10.6 Update `pages/index.vue` organisation-info section: add `transfer_enabled` toggle (state-locked semantics surfaced — when `STATE_LOCKED`, show "目前在班 N 人，需先全部下班才能調整"). Add `timezone` selector populated from a small curated list (or text input with validation). Both reuse `useOrgSettings.update`.
- [ ] 10.7 Add `pages/checkin/index.vue` to admin nav row in `pages/index.vue` (alongside 成員管理 / App 使用者 / 冷卻管理).
- [ ] 10.8 Friendly error messages for `INVALID_TRANSITION`, `TRANSFER_DISABLED`, `OUT_OF_ORDER`, `STATE_LOCKED` (with `on_duty_count`), `NOT_ON_DUTY`, `INVALID_TIMEZONE`.
- [ ] 10.9 All Org-TZ rendering uses a shared helper (e.g. `formatInOrgTz(iso, org.timezone)`); fall back to browser TZ if Org TZ is missing.
- [ ] 10.10 OrgSwitcher present on `pages/checkin/*` headers (consistent with members / cooldowns / app-users).

## 11. Docs

- [ ] 11.1 Update `api/README.md` with a "打卡 / Checkin" section: state-machine table, endpoint list (mobile + admin), error codes (`INVALID_TRANSITION`, `TRANSFER_DISABLED`, `OUT_OF_ORDER`, `STATE_LOCKED`, `NOT_ON_DUTY`, `INVALID_TIMEZONE`), reverse-geocoder swap path (Nominatim → others), Nominatim Usage Policy notes (User-Agent, 1 req/s avg), and the dual-timestamp + offline-queue contract for `add-app-shell`.
- [ ] 11.2 Update `admin-web/README.md` structure section to include `pages/checkin/`, `composables/useCheckin`, `composables/useOrgSettings`, and a paragraph on the live board + skew warning UX.
- [ ] 11.3 Cross-reference `add-app-shell` in `api/README.md` as the upcoming change that consumes `/app/checkin/*` and implements the persistent queue.

## 12. Smoke

- [ ] 12.1 `cargo test` per-binary serial loop on macOS clean.
- [ ] 12.2 `pnpm typecheck` + `pnpm build` clean.
- [ ] 12.3 Live curl smoke (no Flutter): create an AppUser via admin-web, log in via `/app/auth/login`, change initial password, then walk the full state-machine via `/app/checkin/events` (clock_in → transfer_out → transfer_in → transfer_out → transfer_in → clock_out). Confirm admin-web live board reflects each transition. Test transfer-disabled toggle (clock everyone out, flip toggle, attempt transfer → 403, attempt clock_in/out → ok). Test force-checkout from admin-web. Test timezone change visible in display.
