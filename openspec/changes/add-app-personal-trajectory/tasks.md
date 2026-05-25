## 1. API: GET /app/checkin/me/locations

- [x] 1.1 Add `list_my_locations` handler in `api/src/handlers/location_tracking.rs` — bearer auth via `RequireAppUser`, take `app_user_id` from token context, reuse `list_by_app_user_paginated`, reuse `validate_range`
- [x] 1.2 Register route in `api/src/handlers/mod.rs` next to the existing `/app/checkin/locations` (POST) route
- [x] 1.3 Confirm no Org-toggle check is applied to the new GET (only the existing POST keeps the `LOCATION_TRACKING_DISABLED` gate)
- [x] 1.4 Add integration tests in `api/tests/`: token-derived identity, date range filtering, oversized range rejection, 401 unauthenticated, cross-user isolation, toggle-off-still-readable
- [x] 1.5 `cargo test -p api` clean (one pre-existing flake on `app_me_password::voluntary_change_works_after_flag_already_cleared` from testcontainers contention; passes when re-run alone)

## 2. App: data layer for personal trajectory

- [x] 2.1 Add `flutter_map` and `latlong2` to `app/pubspec.yaml`; `flutter pub get` clean (full iOS/Android build deferred until §11 smoke to avoid the 10-minute cold-cache cost on every iteration)
- [x] 2.2 Create `app/lib/features/trajectory/data/my_locations_repository.dart` — wraps `dio` call to `GET /app/checkin/me/locations`, parses to `List<LocationPingDto>` (reuses existing `core/api/models/location_ping.dart`)
- [x] 2.3 Create `app/lib/features/trajectory/data/trajectory_stats.dart` — pure function computing distance (geodesic sum via `latlong2`) and on-shift duration from a ping list; 5 unit tests pass
- [x] 2.4 Create `app/lib/features/trajectory/state/trajectory_controller.dart` — Riverpod `AsyncNotifier<TrajectoryDayState>` holding `{ selectedDate, pings, stats }`; exposes `selectDate(DateTime)` and `refresh()`
- [x] 2.5 Controller tests: build-today, selectDate-refetch, repository error → AsyncError, refresh re-queries the same day (permission-denied path deferred to §3 screen tests where the permission widget lives)

## 3. App: trajectory screen

- [x] 3.1 Create `app/lib/features/trajectory/presentation/trajectory_screen.dart` — scaffold with app bar, date dropdown (today + previous 7 days; uses device local time, matching the existing `history_screen.dart` convention — Org-tz strict mode left for a later cleanup), map area, stats area
- [x] 3.2 `flutter_map` setup: CARTO Positron tile URL, RichAttributionWidget showing `© OpenStreetMap contributors © CARTO`, polyline layer, start/end markers, auto fit-bounds via `CameraFit.bounds`
- [x] 3.3 Empty-day path renders `該日無軌跡資料` and does not instantiate FlutterMap
- [x] 3.4 Permission-denied path renders primer card with `前往系統設定` button hooked to `AppSettings.openAppSettings()`; FlutterMap not instantiated
- [x] 3.5 Stats panel: 走動距離 (km, 1 decimal), 在班時長 (`H 小時 M 分`), 位置點 (integer count)
- [x] 3.6 Widget tests: empty-day, permission-denied, picker-change-triggers-refetch. Data-branch widget assertion (FlutterMap mounted) is omitted because TestWidgetsFlutterBinding stubs network → bubbles uncaught tile-fetch exceptions; the with-data branch is covered by the controller test (stats computation) and §11 manual smoke (visual)

## 4. App: home summary card

- [x] 4.1 `today_summary_card.dart` — Consumer widget shows distance + duration via `l10n.trajectoryDistanceKm` / `trajectoryDurationHm`, tap-through to `/trajectory`
- [x] 4.2 Visibility: render when on-shift (`onSite` or `inTransit`) OR today's ping count > 0; else `SizedBox.shrink()`
- [x] 4.3 Refresh trigger: subscribes to `LocationTrackingService.tickStream`, debounced to one refresh per 60s. App lifecycle `resumed` forces an immediate refresh (bypasses debounce)
- [x] 4.4 Card inserted in `home_screen.dart` between `HomeButtons` and the `Wrap` containing `QueueChip` / `TrackingChip`
- [x] 4.5 Widget tests (4): hidden on off-shift + zero pings, visible on-shift + zero pings, visible off-shift with pings, tap navigates to /trajectory

## 5. App: navigation shell refactor

- [x] 5.1 Refactored `app/lib/app/router.dart` to use `StatefulShellRoute.indexedStack` for the three authenticated top-level routes (`/`, `/history`, `/trajectory`); `/splash`, `/login`, `/force-change-password`, `/dev-server-config` stay outside the shell
- [x] 5.2 Added `AppRoutes.trajectory = '/trajectory'` constant (home stays `/` rather than renaming to `/home` — matches existing redirect targets and `initialLocation`)
- [x] 5.3 `_AppShell` builds a Material `NavigationBar` with three destinations: 首頁 (`Icons.access_time`) → `/`, 歷史 (`Icons.history`) → `/history`, 我的軌跡 (`Icons.map_outlined`) → `/trajectory`. Tap on the active tab re-pushes its initial location (`initialLocation:` flag in `goBranch`)
- [x] 5.4 Removed the in-page `TextButton.icon` on the home screen that pushed `/history` (the nav bar replaces it); dropped the now-unused `go_router` import
- [x] 5.5 Existing `test/app/router_test.dart` (5 redirect-rule cases) continues to pass after the shell refactor. `StatefulShellRoute` state-preservation between branches is documented go_router behavior; a dedicated widget test for the "shift state survives tab switch" scenario is deferred to §11 smoke (mounting the full HomeScreen with all its providers in a tester is brittle)

## 6. App: consent dialog + permission description reword

- [x] 6.1 Rewrote `NSLocationWhenInUseUsageDescription` to lead with the personal log (verbatim from design.md D6)
- [x] 6.2 Reworded `locationConsentBody` and `locationConsentBulletAudience` in the consent dialog l10n strings — body leads with "讓您可以回顧自己今天的工作路線" before mentioning admin visibility; audience bullet now says both you and admins can view
- [x] 6.3 Updated existing `location_consent_dialog_test` assertion to match the new audience bullet (`您本人可於` + `組織管理員亦可查閱`)

## 7. Store metadata + screenshots

- [x] 7.1 `app/store_metadata/ios/description.txt` reordered — first feature bullet is now "我的工作日記"; org-side tracking demoted; audience bullet says you AND admins can view
- [x] 7.2 `app/store_metadata/ios/promotional_text.txt` rewritten to lead with the personal log
- [x] 7.3 Android `short_description.txt` + `full_description.txt` mirror the iOS reframe
- [ ] 7.4 Capture new iPhone 17 Pro Max App Store screenshot of `/trajectory` with a real polyline — **deferred to §11 smoke** (needs a running simulator + a demo day of pings)
- [ ] 7.5 Play Store trajectory screenshot — **deferred to §11 smoke** for the same reason

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
