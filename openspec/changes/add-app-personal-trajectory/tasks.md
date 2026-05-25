## 1. API: GET /app/checkin/me/locations

- [x] 1.1 Add `list_my_locations` handler in `api/src/handlers/location_tracking.rs` — bearer auth via `RequireAppUser`, take `app_user_id` from token context, reuse `list_by_app_user_paginated`, reuse `validate_range`
- [x] 1.2 Register route in `api/src/handlers/mod.rs` next to the existing `/app/checkin/locations` (POST) route
- [x] 1.3 Confirm no Org-toggle check is applied to the new GET (only the existing POST keeps the `LOCATION_TRACKING_DISABLED` gate)
- [x] 1.4 Add integration tests in `api/tests/`: token-derived identity, date range filtering, oversized range rejection, 401 unauthenticated, cross-user isolation, toggle-off-still-readable
- [x] 1.5 `cargo test -p api` clean (one pre-existing flake on `app_me_password::voluntary_change_works_after_flag_already_cleared` from testcontainers contention; passes when re-run alone)

## 2. App: data layer for personal trajectory

- [ ] 2.1 Add `flutter_map` and `latlong2` to `app/pubspec.yaml`; `flutter pub get`; verify iOS/Android build still produces a binary
- [ ] 2.2 Create `app/lib/features/trajectory/data/my_locations_repository.dart` — wraps `dio` call to `GET /app/checkin/me/locations`, parses to `List<LocationPing>` (reuse existing `core/api/models/location_ping.dart`)
- [ ] 2.3 Create `app/lib/features/trajectory/data/trajectory_stats.dart` — pure function computing distance (geodesic sum via `latlong2`) and on-shift duration from a ping list; unit tests
- [ ] 2.4 Create `app/lib/features/trajectory/state/trajectory_controller.dart` — Riverpod async state holding `{ selectedDate, pings, loading, error }`; exposes `selectDate(DateTime)` and `refresh()`
- [ ] 2.5 Widget/unit tests for the controller: fetch on selectedDate change, no-network error path, empty-day path, permission-denied path

## 3. App: trajectory screen

- [ ] 3.1 Create `app/lib/features/trajectory/presentation/trajectory_screen.dart` — scaffold with app bar, date dropdown (today + previous 7 days, Org-tz aware), map area, stats area
- [ ] 3.2 Implement `flutter_map` setup: CARTO Positron tile URL, attribution overlay `© OpenStreetMap contributors © CARTO`, polyline layer, start/end markers, auto fit-bounds on data load
- [ ] 3.3 Empty-day path — render text `該日無軌跡資料`, skip map instantiation
- [ ] 3.4 Permission-denied path — render primer card with "前往系統設定" button hooked to `app_settings`; do not instantiate map
- [ ] 3.5 Stats panel rendering: 走動距離 (km, 1 decimal), 在班時長 (`H 小時 M 分`), 位置點 (integer count)
- [ ] 3.6 Widget tests: today-with-data, empty-day, permission-denied, picker-changes-trigger-refetch

## 4. App: home summary card

- [ ] 4.1 Create `app/lib/features/trajectory/presentation/today_summary_card.dart` — Riverpod widget that shows distance + duration for today, tap-through to `/trajectory`
- [ ] 4.2 Visibility rule: render when on-shift OR today's ping count > 0; otherwise return `SizedBox.shrink()`
- [ ] 4.3 Refresh trigger: subscribe to `LocationTrackingService.tickStream`, debounced to one network call per 60s; also refresh on `WidgetsBindingObserver.didChangeAppLifecycleState` resume
- [ ] 4.4 Insert card on `home_screen.dart` between the clock-in/out hero area and the queue chip (or wherever fits the existing layout — confirm via screenshot)
- [ ] 4.5 Widget test: card hidden on off-shift no-data day, visible on on-shift day, tap navigates to /trajectory

## 5. App: navigation shell refactor

- [ ] 5.1 Refactor `app/lib/app/router.dart` to use `StatefulShellRoute.indexedStack` for the three authenticated top-level routes (`/home`, `/history`, `/trajectory`); keep `/splash`, `/login`, `/force-change`, `/dev-server-config` outside the shell
- [ ] 5.2 Add `AppRoutes.trajectory = '/trajectory'` constant
- [ ] 5.3 Build the shell scaffold with a Material `NavigationBar` containing three destinations: 首頁 (`Icons.access_time`), 歷史 (`Icons.history`), 我的軌跡 (`Icons.map_outlined`)
- [ ] 5.4 Remove the `IconButton` on home's app bar that opens `/history` (the nav bar replaces it)
- [ ] 5.5 Widget tests: tab switch preserves home shift state (`isWorking` survives switch), three destinations visible from each top-level route, one-tap reach

## 6. App: consent dialog + permission description reword

- [ ] 6.1 Rewrite `NSLocationWhenInUseUsageDescription` in `app/ios/Runner/Info.plist` to lead with personal-log framing (see design.md D6 for text)
- [ ] 6.2 Update the Flutter clock-in consent dialog body text — find the existing dialog in the checkin flow and lead with the personal-log framing
- [ ] 6.3 Update `app/lib/l10n/app_localizations.dart` strings if the consent text lives there

## 7. Store metadata + screenshots

- [ ] 7.1 Rewrite `app/store_metadata/ios/description.txt` to lead the feature list with "我的工作日記" (per design.md D7 and mobile-release spec)
- [ ] 7.2 Rewrite `app/store_metadata/ios/promotional_text.txt` to reference 我的工作日記
- [ ] 7.3 Update Android equivalent (`app/store_metadata/android/short_description.txt`, `full_description.txt`) if those exist; same lead-with-personal-log treatment
- [ ] 7.4 Capture a new App Store screenshot of the `/trajectory` screen on an iPhone 17 Pro Max simulator with a real polyline; place file in `app/store_metadata/ios/screenshots/` so its sort order puts it in slot 2 or 3
- [ ] 7.5 Update Play Store screenshot set analogously

## 8. App Review reply artifact

- [ ] 8.1 Create `app/store_metadata/ios/app_review_replies/2.5.4-2026-05-15.md` with the verbatim reply body (see the English draft already prepared in the conversation thread) — cite guideline `2.5.4`, submission id `2f88a54d-2b9a-4069-b5fa-88e2ed770187`
- [ ] 8.2 Add demo-account credentials placeholder section the operator fills in before submitting

## 9. Version bump + changelog

- [ ] 9.1 Bump `app/pubspec.yaml` to `0.3.1+8`
- [ ] 9.2 Add `app/store_metadata/ios/release_notes/0.3.1.txt` with a zh-TW one-paragraph note about 我的工作日記
- [ ] 9.3 Update `CHANGELOG.md` at repo root: new entry under `[app] 0.3.1+8`

## 10. Deploy runbook updates

- [ ] 10.1 In `DEPLOY.md` under "App cut release > iOS cut", add a sub-step reminding the operator to verify App Privacy form's "Precise Location" use case includes "App Functionality" (not only "Other Purposes")
- [ ] 10.2 Add a sub-step reminding the operator to seed at least one demo day of pings on the review demo Org before pressing submit
- [ ] 10.3 Add a sub-step reminding the operator to paste the contents of `app_review_replies/2.5.4-2026-05-15.md` into App Store Connect's "App Review notes" / message thread on resubmit

## 11. Smoke + ship

- [ ] 11.1 Run `flutter analyze` clean on the app
- [ ] 11.2 Run `flutter test` clean on the app
- [ ] 11.3 Manual smoke on a real iPhone: clock in, walk 2 minutes, open 我的軌跡 tab, see polyline; clock out, see blue indicator disappear, home card persists with final stats
- [ ] 11.4 Manual smoke on Android: same path, verify foreground service notification still present, navigation bar still works
- [ ] 11.5 Cut iOS build 0.3.1 (8) and submit to App Store Connect with the 2.5.4 reply pasted into App Review notes
- [ ] 11.6 Cut Android build 0.3.1 (8) and submit to Play Console (location-tracking justification re-asserted; usually a re-review even though Google didn't reject)
