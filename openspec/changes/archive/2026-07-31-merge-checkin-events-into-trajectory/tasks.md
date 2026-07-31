## 1. Resolve the open question first

- [x] 1.1 Query KLCC production `checkin_events` for imported events with degenerate coordinates scoped to `source = legacy_backfill`. **Result: 0 rows.** `countDocuments({ source: "legacy_backfill", $and: [{ "location.coordinates.lat": 0 }, { "location.coordinates.lng": 0 }] })` → `0` (run 2026-07-30).
- [x] 1.2 No degenerate coordinates exist, so the merge ships unchanged with no coordinate validation. Recorded against the Open Questions entry in `design.md` and folded into the matching Risks entry — the absence of a `(0, 0)` filter is now evidence-backed, not an oversight.

## 2. API — range filters on the event history endpoints

- [x] 2.1 Extract `validate_range` from `api/src/handlers/location_tracking.rs` into a shared helper (alongside the RFC3339 → `INVALID_RANGE` parse pattern used at its four call sites) and repoint the existing ping list and export handlers at it. No behaviour change; existing ping tests must stay green.
- [x] 2.2 Add `from: Option<String>` and `to: Option<String>` to `EventsCursorQuery` in `api/src/handlers/checkin_dto.rs`.
- [x] 2.3 Add optional `from` / `to` range predicates to `CheckinEventRepository::list_by_app_user_paginated` in `api/src/db/checkin_events.rs`, composing with the existing `before` cursor via AND and preserving newest-first ordering. Confirm the existing `(app_user_id, occurred_at_client)` index covers the added predicate — no new index expected.
- [x] 2.4 Wire range parsing + validation into `list_events` in `api/src/handlers/app_checkin.rs` (`GET /app/checkin/events`), returning `INVALID_RANGE` on parse failure, `to < from`, or span > 90 days, and NOT rejecting on an old `from` alone.
- [x] 2.5 Wire the same into `list_user_events` in `api/src/handlers/checkin.rs` (`GET /checkin/users/:id/events`), keeping the `current_org` scoping check ahead of range validation so a cross-org id still returns `404`.
- [x] 2.6 Integration tests against real MongoDB for `GET /app/checkin/events`: range filter, old-`from`-allowed, span > 90 days rejected, `to < from` rejected, and omitting both params returning the pre-change page.
- [x] 2.7 Integration tests for `GET /checkin/users/:id/events`: range filter, a 200-day-old event retrievable behind more than 100 newer events, `INVALID_RANGE` cases, member parity with admin, and cross-org `404` with a range supplied.
- [x] 2.8 `cargo test` and `cargo clippy` clean. Verified: `cargo clippy --all-targets` clean, and the full `cargo test` run is **285 passed / 0 failed** against real MongoDB — including the `location_tracking_export` / `location_tracking_list` / `location_tracking_self_list` suites that the shared-range refactor touched, so 2.1 is confirmed behaviour-preserving.

## 3. Shared merge contract — admin-web

- [x] 3.1 Add a merge module next to `admin-web/utils` (or wherever `timeOfDayColorForMinute` lives, to keep the two shared contracts adjacent) exposing a `buildMergedSeries(pings, events)` returning `{ lat, lng, occurredAtClient, originRank, id }[]`: filters `source === 'admin_force'` out of events, parses `occurred_at_client` to instants, and sorts by instant → originRank (event `0`, ping `1`) → `id`.
- [x] 3.2 Unit-test the merge in isolation: interleaved ordering, `admin_force` excluded, `legacy_backfill` included, equal-timestamp tie resolving to the event, mixed `Z` / `+08:00` inputs ordering by instant, and near-coincident points both retained.
- [x] 3.3 Add `from` / `to` to `listUserEvents` in `admin-web/composables/useCheckin.ts`.

## 4. admin-web trajectory page

- [x] 4.1 In `admin-web/pages/checkin/[appUserId]/trajectory.vue`, pass the day range to `listUserEvents` and delete the client-side day filter (the `Date.parse` window over `eventsRes`).
- [x] 4.2 Replace the `pings.value` `localeCompare` sort with the merged series from `buildMergedSeries`, and drive the per-segment polylines from it.
- [x] 4.3 Replace `hasData` and the `clockInEvent` anchor computation with the merged point count thresholds: `0` → `該日無軌跡資料` and no Leaflet, `1` → map + markers only, `>= 2` → map + markers + line.
- [x] 4.4 Keep markers sourced from the full event list so an `admin_force` `clock_out` still renders its marker while contributing no vertex.
- [x] 4.5 Include merged points and marker coordinates in `fitBounds`.
- [x] 4.6 Extend `admin-web/test/pages/trajectory.test.ts`: line endpoints equal the clock-in/clock-out coordinates, the three render thresholds, `admin_force` marker-without-vertex, and the range-scoped event request being issued with the resolved day bounds.
- [x] 4.7 `pnpm typecheck` (or the repo's equivalent) and the admin-web test suite clean.

## 5. Flutter — merge contract and trajectory screen

- [x] 5.1 Add a Dart merge implementation beside `app/lib/features/trajectory/data/time_of_day_color.dart` mirroring `buildMergedSeries` exactly — same filter, same three-level order, same instant parsing — with a shared `MergedPoint` type.
- [x] 5.2 Unit-test the Dart merge against the same fixture cases as task 3.2 so both platforms are demonstrably in agreement.
- [x] 5.3 Add `from` / `to` to `CheckinRepository.events` in `app/lib/features/checkin/data/checkin_repository.dart`.
- [x] 5.4 In `app/lib/features/trajectory/state/trajectory_controller.dart`, fetch events with the day range instead of `limit: 100` + client-side filtering, and expose the merged series on the day state.
- [x] 5.5 Rewrite `computeTrajectoryStats` in `app/lib/features/trajectory/data/trajectory_stats.dart` to take merged points: geodesic distance over the merged series, first→last merged span for duration, merged count for the point count. Update the stale doc comment that calls the precise in-shift duration out of scope.
- [x] 5.6 In `app/lib/features/trajectory/presentation/trajectory_screen.dart`, build the polyline segments from the merged series and replace the empty-state branch with the three merged-count thresholds. Markers keep coming from the full event list.
- [x] 5.7 Verify the permission-denied primer card still takes precedence over all three merged-count states (it is a distinct branch, not a data state).
- [x] 5.8 Update `app/lib/features/trajectory/presentation/today_summary_card.dart` to consume the merged series. **No extra fetch was needed**: the card already reads `trajectoryProvider`, the same source the trajectory screen renders from, so consistency is structural rather than something to keep in sync. The design's assumption that the card would need its own events request — and the throttle note that followed from it — was wrong. Only the `pingCount` → `pointCount` rename applied.
- [x] 5.9 Tests: line endpoints match clock-in/clock-out, single merged point, `admin_force` marker-not-vertex, range-scoped event fetch, and head/tail legs in the stats. **Placed at the controller/unit level, not the widget level**: any widget test that mounts `FlutterMap` fails under `TestWidgetsFlutterBinding` because tile fetches always return 400 (the pre-existing note at the top of `trajectory_screen_test.dart` documents this). The controller's `mergedSeries` is the exact value the widget's branch reads, and admin-web's `trajectory.test.ts` asserts real polyline vertices against the same contract, so the geometry is covered on one platform plus the §6.4 device smoke. The home card shares `trajectoryProvider` with the screen, so agreement needs no separate fixture.
- [x] 5.10 `flutter analyze` and `flutter test` clean.

## 6. Verification

- [x] 6.1 Verified on 2026-07-31, a day with 12 app-created events plus pings, rendered in both the app's 我的軌跡 and admin-web's trajectory page: vertex order and line shape agree. Worth noting this comparison was only meaningful after #74 clamped the app's zoom — before that the two surfaces framed the same day differently, and the discrepancy would have looked like the merge contract drifting.
- [x] 6.2 Smoke a `legacy_backfill` day for a KLCC AppUser in admin-web. **Verified on two dates, deliberately chosen to separate the two mechanisms.** `2026-07-13` (17 days back, within the old `limit: 100` reach) proved the merge alone: both markers moved onto the line and the final leg took the correct mid-day colour. `2026-05-13` proved the range-scoped fetch: for this AppUser the 100th-newest event sits at `2026-05-19`, so the old client saw **zero** events on that date and rendered no markers at all — the new client renders both and runs the line through them. Ran against a full local copy of prod data (10239 events / 19034 pings), so no prod writes were involved.
- [x] 6.3 Verified on prod admin-web using a KLCC AppUser-day that has 2 events and **0 pings** (`wrkuo`, 2026-07-30): the two markers are joined by a line, which the old renderer left as two isolated dots. Confirms the accepted consequence in the proposal — the line is derived entirely from coordinates already stored on the events, with no new data collected. Reached the same behaviour without touching any Org's `location_tracking_enabled` setting, which is currently state-locked by two on-shift AppUsers anyway.
- [x] 6.4 Ran a real shift on the device (iPhone 17, iOS 26.5.2) against prod — clock in, wait, clock out — producing the first `source = app` events prod had ever held; every prior record was `legacy_backfill`. The polyline's endpoints are the clock-in and clock-out coordinates, and 在班時長 became the real clock-in→clock-out span rather than the first-ping→last-ping one. **Caveat**: the shift stayed indoors, so all points shared one coordinate and the line's *shape* was not exercised here — that is covered by 6.1's cross-platform comparison over a 12-event day, and by the admin-web legacy days in 6.2. This run did surface a genuine regression the merge introduced (a zero-area camera fit crashing flutter_map), fixed in #74.
- [x] 6.5 Confirmed by exporting from the trajectory page: the xlsx is still ping-only. Checkin coordinates do not appear, so a day's exported rows stay fewer than the plotted points — the deliberate limitation recorded in the proposal's Impact, not a defect. (Previously verified at the code level only; now confirmed from the running app.)
