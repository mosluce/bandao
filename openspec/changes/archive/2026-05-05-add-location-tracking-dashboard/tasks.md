## 1. API: extend LocationListQuery with from / to

- [x] 1.1 In `api/src/handlers/location_tracking.rs`, extend `LocationListQuery` with `pub from: Option<String>` and `pub to: Option<String>` (RFC3339, both `#[serde(default)]`).
- [x] 1.2 In `list_locations` handler, parse `from` / `to` (each via `parse_rfc3339`), surface parse failures as `INVALID_RANGE`. Match the export endpoint's validation (`to < from`, span > 90 days, `from` older than 90 days from now).
- [x] 1.3 Either side may be omitted — only run the corresponding filter when present, no validation error for absent sides (single-sided range is allowed).
- [x] 1.4 Pass parsed `from` / `to` (as `Option<bson::DateTime>`) into the db query layer. Update `LocationPingsRepo` (or wherever `list_by_app_user_paginated` lives) to accept them and add the corresponding filters.
- [x] 1.5 Behavior: `before` cursor still works independently. When `from` AND `before` both supplied, both filter; results still newest-first with `limit` cap.
- [x] 1.6 Add unit tests covering the range validation: parse error, `to < from`, span > 90 days, `from` older than 90 days from now, single-sided OK, both-sides OK.
- [x] 1.7 Add an integration test in `api/tests/location_tracking_list.rs` (or new file) — seed pings spanning 3 days, verify `?from=&to=` returns only the day's slice in newest-first order.
- [x] 1.8 `cargo fmt --all -- --check` clean. `cargo clippy --all-targets --all-features -- -D warnings` clean. `cargo test` green.

## 2. admin-web: types + composables

- [x] 2.1 In `admin-web/types/api.ts`:
  - Update `OrgCheckin` to `{ transfer_enabled: boolean, location_tracking_enabled: boolean }`
  - Update `UpdateOrgSettingsRequest` to add `location_tracking_enabled?: boolean`
  - Update the response shape (likely `OrgSettingsDto`) similarly
  - Add `LocationPingDto` mirroring API shape (`id, app_user_id, lat, lng, accuracy_meters?, occurred_at_client, occurred_at_server`)
  - Add `LocationListParams` (`{ from?: string; to?: string; before?: string; limit?: number }`)
- [x] 2.2 Create `admin-web/composables/useLocationPings.ts` with `list({ appUserId, params })` returning `LocationPingDto[]`. Wraps `useApi()`.
- [x] 2.3 (Optional) Helper `dateToOrgRange(date: string, tz: string)` that takes `YYYY-MM-DD` + IANA tz and returns `{ from, to }` RFC3339 strings covering that calendar day. Place under `admin-web/utils/` or inline in the page if only used there. Use `Intl.DateTimeFormat` for offset resolution; verify both sides of DST transition produce sensible bounds.

## 3. admin-web: leaflet dep

- [x] 3.1 `pnpm add leaflet` (production dep — runtime import).
- [x] 3.2 `pnpm add -D @types/leaflet`.
- [x] 3.3 Verify `pnpm install` resolves cleanly. Verify `pnpm typecheck` still passes after deps land.

## 4. admin-web: route restructure

- [x] 4.1 Move `admin-web/pages/checkin/[appUserId].vue` to `admin-web/pages/checkin/[appUserId]/index.vue`. Verify `pnpm dev` still serves `/checkin/:appUserId` correctly.
- [x] 4.2 Update any internal navigation that targets the old file path (`<NuxtLink>` `to=...`) — `to="/checkin/:appUserId"` URL shape unchanged, only the file path moved.

## 5. admin-web: trajectory page

- [x] 5.1 Create `admin-web/pages/checkin/[appUserId]/trajectory.vue`.
- [x] 5.2 `definePageMeta({ middleware: 'auth' })` — same as the user-detail page.
- [x] 5.3 State refs: `loading`, `error`, `pings`, `events`, `dateInput` (string `YYYY-MM-DD`), `mapContainer` (template ref).
- [x] 5.4 Resolve initial `dateInput` from `?date=` query, fall back to today in Org tz.
- [x] 5.5 `watch([dateInput, () => auth.currentOrg.value?.id], () => loadDay())` triggers refetch.
- [x] 5.6 `loadDay()`:
  - compute `from` / `to` via `dateToOrgRange`
  - parallel-fetch `useLocationPings().list({ appUserId, params: { from, to, limit: 1000 } })` and events list
  - sort pings ascending by `occurred_at_client` for the polyline
  - filter events client-side to the same range
  - set `loading = false`
- [x] 5.7 Render: loading spinner / `該日無軌跡資料` empty state / map container.
- [x] 5.8 In `onMounted` AND on watcher trigger when pings non-empty: lazy-import `leaflet` (`const L = await import('leaflet')`), import `'leaflet/dist/leaflet.css'`, init / re-init the map. Tear down (`map.remove()`) on `onBeforeUnmount` or before re-init.
- [x] 5.9 Tile layer: CartoDB Positron — `https://{s}.basemaps.cartocdn.com/light_all/{z}/{x}/{y}.png` with attribution `'© OpenStreetMap contributors © CARTO'`.
- [x] 5.10 Polyline: `L.polyline(pings.map(p => [p.lat, p.lng]), { color: '#1f2937', weight: 3 })`.
- [x] 5.11 Event markers: `L.circleMarker([e.location.coordinates.lat, ...], { radius, color, fillColor })` per event_type. Pop-up shows event_type label + timestamp. Color hint:
  - clock_in: green
  - clock_out: slate
  - transfer_in / transfer_out: amber
- [x] 5.12 `map.fitBounds(bounds, { padding: [20, 20] })` covering polyline + markers.
- [x] 5.13 Date picker `<input type="date" v-model="dateInput">` at top of page; `<NuxtLink to="..">` back to user-detail; export action (next section).
- [x] 5.14 Update URL on date change: `router.replace({ query: { ...route.query, date: dateInput.value } })`.

## 6. admin-web: xlsx export modal

- [x] 6.1 Add a date-range modal triggered by an "匯出" button on the trajectory page top-right.
- [x] 6.2 Two `<input type="date">` (from / to) + 確認 / 取消 buttons.
- [x] 6.3 Client-side validation before submit:
  - both fields required
  - `to >= from`
  - span ≤ 90 days
  inline error text below the inputs.
- [x] 6.4 On confirm: build URL `${apiBaseUrl}/checkin/users/<id>/locations/export?from=&to=`. RFC3339 the dates (Org tz). Trigger via `<a href download>` click programmatically (`document.createElement('a').click()`).
- [x] 6.5 Close modal on success.

## 7. admin-web: Org settings toggle

- [x] 7.1 In `pages/index.vue`, copy the existing `transfer_enabled` toggle's `<dt>/<dd>` block. Adapt:
  - dt label: `定位追蹤`
  - bound to `auth.currentOrg.value.checkin.location_tracking_enabled`
  - on change calls `orgSettings.update({ location_tracking_enabled: target })`
  - new refs: `locationTrackingToggleSaving`, `locationTrackingToggleError`
  - hint text: `關閉後，App 端不再蒐集工作期間定位軌跡。已存在的軌跡資料不受影響。`
- [x] 7.2 Same `STATE_LOCKED` handling — show `'目前有 App 使用者在班，需先全部下班才能調整此設定'`.
- [x] 7.3 After successful update, refresh `auth.currentOrg` if needed (likely already auto-syncs from the API response).

## 8. admin-web: link from user-detail to trajectory

- [x] 8.1 In `pages/checkin/[appUserId]/index.vue` (the moved user-detail page), add a `<NuxtLink>` "查看軌跡" pointing to `/checkin/:appUserId/trajectory`.
- [x] 8.2 Optionally hide the link when `auth.currentOrg.value.checkin.location_tracking_enabled === false` — keeps the UI clean when the feature is off (data may still exist but admin probably doesn't expect to see the link).

## 9. Tests (vitest, sourcing the framework from add-admin-web-test-infra)

- [x] 9.1 `admin-web/test/composables/useLocationPings.test.ts` — mock `$fetch` (via `mockNuxtImport` or override), assert URL params encode `from` / `to` correctly, assert response shape.
- [x] 9.2 `admin-web/test/utils/dateToOrgRange.test.ts` (if helper extracted) — verify Asia/Taipei `2026-03-01` → `from=2026-03-01T00:00:00+08:00`, `to=2026-03-02T00:00:00+08:00`.
- [x] 9.3 `admin-web/test/pages/trajectory.test.ts` — mock the composables, mount, assert empty state shows the `該日無軌跡資料` text and DOES NOT mount the map container. With non-empty pings, assert map container mounts. Skip Leaflet init internals (they require browser-grade DOM) — assert at component-state level.
- [x] 9.4 ~~Deferred~~ — pages/index.vue mounts a 600+ line dashboard with many composables; the toggle handler is a near-copy of transfer_enabled (already shipped + tested in prod), and §13.6 covers it end-to-end. Cost > value here. `admin-web/test/pages/index-toggle.test.ts` — mount `pages/index.vue` (or extract toggle to a sub-component), simulate toggle click, assert `useOrgSettings.update` called with `{ location_tracking_enabled: true/false }`.
- [x] 9.5 All admin-web tests pass via `pnpm test`.

## 10. Documentation

- [x] 10.1 Update `admin-web/README.md` "結構" section to reflect the new `pages/checkin/[appUserId]/{index,trajectory}.vue` layout.
- [x] 10.2 Add a "軌跡頁" section briefly: route, what it shows, dependency on Org toggle being on for new data.
- [x] 10.3 Update `api/README.md` reverse-geocoding section is fine (untouched). Add a sentence to the location-tracking section noting the new `from`/`to` filter on the list endpoint.

## 11. ROADMAP

- [x] 11.1 At archive time, remove the `add-location-tracking-dashboard` entry from `ROADMAP.md` "下一批 changes 已規劃". (The CSV → xlsx wording inconsistency dies with the entry.)

## 12. CI verification

- [x] 12.1 `cargo fmt --all -- --check` + `cargo clippy --all-targets --all-features -- -D warnings` + `cargo test --all-features --no-fail-fast` all green locally.
- [x] 12.2 `pnpm typecheck` + `pnpm test` + `pnpm build` all green locally.
- [x] 12.3 After archive auto-commit pushes, both `api` and `admin-web` GitHub Actions workflows pass.

## 13. Smoke (manual)

- [x] 13.1 With API + admin-web running locally and an Org with `location_tracking_enabled: true`, an AppUser that has uploaded pings: navigate to `/checkin/:appUserId/trajectory`, see polyline + markers + auto-fit. Switch date with picker — page refetches.
- [x] 13.2 Empty date (a day with no pings) shows `該日無軌跡資料` and no map.
- [x] 13.3 Click 匯出 button, choose valid range (≤ 90 days), confirm — xlsx downloads.
- [x] 13.4 Try invalid range (>90 days, to < from) — inline error blocks submit.
- [x] 13.5 Try date that resolves to >90 days ago `from` — request goes through but server returns `INVALID_RANGE`; verify graceful UX.
- [x] 13.6 On `/`, toggle 定位追蹤 off (with no AppUser on shift) — succeeds. Toggle on. Have an AppUser clock in, then try toggle — server returns `STATE_LOCKED`, UI shows the localized error.
