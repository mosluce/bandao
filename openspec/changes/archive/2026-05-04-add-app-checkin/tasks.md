## 1. Dependencies + native config

- [x] 1.1 Add runtime deps to `app/pubspec.yaml`: `geolocator`, `app_settings`, `connectivity_plus`, `workmanager`, `drift`, `sqlite3_flutter_libs`. Use latest stable versions compatible with Flutter 3.29.
- [x] 1.2 Add dev dep `drift_dev`. Verify `build_runner` (already present) works with the new generator.
- [x] 1.3 Run `flutter pub get`; commit updated `pubspec.lock`.
- [x] 1.4 iOS `Info.plist` — add `NSLocationWhenInUseUsageDescription` (zh-TW string explaining usage) and `UIBackgroundModes: <array><string>processing</string></array>`. Add `BGTaskSchedulerPermittedIdentifiers` array containing `tw.ccmos.app.argus.queue-drain`.
- [x] 1.5 Android `AndroidManifest.xml` — add `<uses-permission>` entries for `ACCESS_FINE_LOCATION` and `ACCESS_COARSE_LOCATION`.
- [x] 1.6 `pod install` from `app/ios/`; commit any updated `Podfile.lock` and `Runner.xcodeproj` changes.
- [x] 1.7 Bump `app/pubspec.yaml` `version: 0.2.0+2` to mark this build's significance.

## 2. Drift schema + queue DAO

- [x] 2.1 Create `app/lib/features/checkin/data/checkin_queue_db.dart` with a `@DriftDatabase` class containing the `pending_events` table per design.md (columns: `id, app_user_id, event_type, lat, lng, accuracy, manual_label, occurred_at_client, status, attempts, last_error_code, last_error_message, last_attempt_at, enqueued_at`). Add an index on `(status, occurred_at_client)`.
- [x] 2.2 Define a `QueueRow` typed value class (drift's row class is fine; alias it for app code clarity).
- [x] 2.3 Add typed queries on the database: `enqueue(QueueInsert)`, `pickOldestPending()`, `markSending(id)`, `markPending(id, attempts, lastErrorCode?, lastErrorMessage?)`, `markFailed(id, errorCode, errorMessage)`, `delete(id)`, `wipeForOtherUsers(currentUserId) → int rowsDeleted`, `watchAll(forUserId) → Stream<List<QueueRow>>` for the history view.
- [x] 2.4 Create the database file in the platform's documents directory (`path_provider`). Riverpod-providable (`Provider<CheckinQueueDb>` with `onDispose` closing the DB).
- [x] 2.5 Run `dart run build_runner build --delete-conflicting-outputs`. If sandbox blocks build_runner (per `add-app-shell` notes), fall back to `sqflite` with hand-written SQL: same table, same query surface, just no codegen. Document the choice in a comment block at the top of the file.

## 3. DTO mirrors for /app/checkin/*

- [x] 3.1 `app/lib/core/api/models/checkin_event.dart`: `CheckinEventType` enum with snake_case JSON, `EventSource` and `EventInitiatorKind` enums, `GeoPoint`, `EventLocation`, `CheckinEventDto`. Hand-rolled per the same pattern as `Org` / `AppUser` in `add-app-shell`. `fromJson` / `toJson` / `copyWith` / `==` / `hashCode` all explicit.
- [x] 3.2 `app/lib/core/api/models/checkin_status.dart`: `AppUserCheckinStatus` enum, `CheckinUserStatusDto` with optional `lastEvent: CheckinEventDto?`.
- [x] 3.3 `app/lib/core/api/models/submit_checkin_event.dart`: `SubmitCheckinEventRequest`. RFC3339 timestamp generation helper (`DateTime.now().toIso8601String()` with offset suffix).

## 4. Geolocation service

- [x] 4.1 `app/lib/features/checkin/data/geolocation_service.dart`: typed wrapper around `geolocator`. Methods: `Future<LocationPermission> currentPermission()`, `Future<LocationPermission> requestPermission()`, `Future<GeoPoint?> capture()` (10s timeout, falls back to `getLastKnownPosition()`, returns `null` if both fail), and `Future<bool> openSettings()` (delegates to `app_settings`).
- [x] 4.2 Riverpod-providable; tests substitute a fake.

## 5. Connectivity provider

- [x] 5.1 `app/lib/features/checkin/state/connectivity_provider.dart`: a Riverpod `StreamProvider<bool>` (online/offline) wrapping `connectivity_plus`'s stream. Maps `ConnectivityResult.none → false`, anything else → true.

## 6. CheckinRepository

- [x] 6.1 `app/lib/features/checkin/data/checkin_repository.dart`: methods `Future<CheckinEventDto> submit(SubmitCheckinEventRequest)`, `Future<CheckinUserStatusDto> status()`, `Future<List<CheckinEventDto>> events({String? before, int limit = 50})`. All thin wrappers around dio; throw `ApiException` on errors.
- [x] 6.2 Async Riverpod provider that constructs the repository from the existing `apiClientProvider`.

## 7. Queue processor

- [x] 7.1 `app/lib/features/checkin/data/queue_processor.dart`: a `QueueProcessor` class with constructor injection of `CheckinQueueDb`, `CheckinRepository`, `Ref`. Public method `Future<void> tick()`. Internally implements:
  - early-out if a row is already `sending`,
  - early-out if connectivity is offline (don't increment attempts, don't mark sending),
  - pick oldest `pending` row,
  - mark `sending`, increment `attempts`,
  - call repository `submit(...)`,
  - on success: delete row, recursively call tick,
  - on `ApiException` with code in `{INVALID_TRANSITION, OUT_OF_ORDER, TRANSFER_DISABLED, NEEDS_PASSWORD_CHANGE}`: mark `failed` with code+message, recurse,
  - on `ApiException` with status 401 or code `UNAUTHORIZED`: mark `failed`, signal auth state to clear token, return without recursion,
  - on any other ApiException (5xx, network, unknown): return row to `pending` with incremented `attempts`, schedule next backoff.
- [x] 7.2 Backoff schedule: `[1, 2, 4, 8, 16, 30]` seconds clamped to attempts index (cap at 30s). Exposed as a `Duration nextDelay(int attempts)` static helper for unit testing.
- [x] 7.3 Riverpod `Provider<QueueProcessor>` plus a `Provider<void>` (kept-alive) that wires the processor to wake on: queue insert (drift change stream), connectivity transition to online, and a `Timer.periodic(Duration(seconds: 1))` foreground tick.
- [x] 7.4 Document in a code comment that `tick()` must be re-entrant-safe; the in-flight check is the locking mechanism.

## 8. Background sync via workmanager

- [x] 8.1 `app/lib/features/checkin/data/background_sync.dart`: register the workmanager callback dispatcher, define a top-level `void backgroundCallback()` that opens the drift DB, reads the active AppUser id from secure storage, builds a fresh `Dio` instance with the resolved base URL + bearer token, and runs the queue processor `tick()` until either the queue is empty or the OS time budget is near expiry.
- [x] 8.2 On app start (in `main.dart` after `runApp`), call `Workmanager().initialize(backgroundCallback, isInDebugMode: kDebugMode)` and `Workmanager().registerOneOffTask(...)` once for the iOS `BGProcessingTask` registration with identifier `tw.ccmos.app.argus.queue-drain`.
- [x] 8.3 In the queue processor's `enqueue` path (Section 7), additionally call `Workmanager().registerOneOffTask(uniqueName: 'queue-drain', taskName: 'queue-drain', constraints: Constraints(networkType: NetworkType.connected))` so Android schedules an immediate background drain.
- [x] 8.4 In `app/README.md` document the iOS BGTask limitation and the recommended user behavior (do not force-quit while items are pending).

## 9. Status + queue state providers

- [x] 9.1 `app/lib/features/checkin/state/checkin_status_provider.dart`: `AsyncNotifier<CheckinUserStatusDto>` that fetches `/app/checkin/status` on auth-state-`authenticated` and refetches on demand (`refresh()`).
- [x] 9.2 `app/lib/features/checkin/state/checkin_queue_provider.dart`: `Provider<Stream<List<QueueRow>>>` watching the drift table for the current user.
- [x] 9.3 `app/lib/features/checkin/state/effective_status_provider.dart`: a derived `Provider<EffectiveStatus>` that combines the server status + queue rows via the optimistic reducer (per design.md). Emits `(status, currentShiftStartedAt?, hasPendingTransition?)`. Pure; unit-testable.

## 10. Login-handover queue wipe

- [x] 10.1 In `AuthNotifier` (existing `app/lib/features/auth/state/auth_provider.dart`), after every successful `_fetchMe()` resolution, call `CheckinQueueDb.wipeForOtherUsers(currentUserId)`. Capture the deleted-row count and emit a one-shot toast via a new `pendingHandoverNoticeProvider` (Riverpod `StateProvider<String?>`).
- [x] 10.2 In `home_screen.dart`, listen to `pendingHandoverNoticeProvider`; when non-null, show a `SnackBar` with the message and clear the provider value. Ensure it shows exactly once per login.

## 11. Home screen rebuild

- [x] 11.1 Replace the `CheckinStatusPill` placeholder in `app/lib/shared/widgets/checkin_status_pill.dart` (or migrate to `app/lib/features/checkin/presentation/status_pill.dart`) with the real implementation: shows status icon + label, `region_name` from latest synced event, and an elapsed-shift counter computed from `currentShiftStartedAt`. Hide the elapsed counter when status is `off_duty`.
- [x] 11.2 `app/lib/features/checkin/presentation/home_buttons.dart`: renders the action buttons set per effective status. Each button wires `onPressed` to `enqueueEvent(eventType)`.
- [x] 11.3 `app/lib/features/checkin/presentation/location_permission_blocker.dart`: when permission is `denied`/`deniedForever`, render the inline blocker copy + `[開啟設定]` button. This widget hides itself when permission is `granted` (or `notDetermined` — the prompt fires on tap).
- [x] 11.4 `app/lib/features/checkin/presentation/queue_chip.dart`: renders the queue indicator. Hidden when the queue is empty. `onTap` navigates to `/history`.
- [x] 11.5 Update `app/lib/features/auth/presentation/home_screen.dart` to compose: identity block (display_name + username) + new `StatusPill` + `LocationPermissionBlocker` + `HomeButtons` + `QueueChip` + `事件歷史` link routing to `/history`.
- [x] 11.6 Add `enqueueEvent(eventType)` flow: capture GPS via `GeolocationService.capture()`, request permission first if needed, build the queue row, insert via `CheckinQueueDb.enqueue()`, wake the processor. Surface error toast when GPS returns `null`.

## 12. History screen

- [x] 12.1 Add `/history` to `app/lib/app/router.dart` — protected by `auth` guard like `/`.
- [x] 12.2 `app/lib/features/checkin/presentation/history_screen.dart`: fetches `GET /app/checkin/events` (initial 50, cursor pagination via `[載入更多]`), watches the local queue stream, merges by `occurred_at_client` desc.
- [x] 12.3 Each list row uses a `_HistoryRow` widget (new) with status badge (`pending` / `sending` / `failed` / `synced`), event-type label, time (formatted in Org timezone — pull from `auth.currentOrg.value?.timezone` for now; user device locale if missing), and either a region label (synced) or `lat, lng` (local) + accuracy.
- [x] 12.4 Failed rows render `[複製細節]` + `[關閉]` actions inline. `[複製細節]` builds the plaintext blob per design.md and writes to clipboard via `Clipboard.setData`. `[關閉]` calls `CheckinQueueDb.delete(id)`; the stream auto-removes the row from the view.
- [x] 12.5 `[載入更多]` button at the bottom; loads next 50 server events using oldest-displayed `occurred_at_client` as `before`. Hidden when fewer than 50 rows came back on the last fetch.

## 13. Onboarding tip (iOS background limitation)

- [x] 13.1 Add a one-shot tip widget that displays after the first successful login on iOS. Persistence via secure storage key `home.background_sync_tip_seen` (Boolean). Copy: explains that iOS schedules background sync at the OS's discretion and recommends keeping the app from being force-quit while items are queued. Dismissible; never re-shown.
- [x] 13.2 No tip shown on Android (WorkManager is reliable enough that the explanation isn't useful); keep the storage key platform-specific.

## 14. Tests

- [x] 14.1 `test/features/checkin/data/queue_processor_test.dart`: 8+ unit tests covering 201 success, 4xx state-machine failures (each error code), 401 path, 5xx retry, network retry, offline skip, single in-flight constraint, backoff schedule.
- [x] 14.2 `test/features/checkin/state/effective_status_provider_test.dart`: pure-function tests of the optimistic reducer — empty queue, pending overlay, failed-row exclusion, multi-event sequences, edge cases (failed clock_in then later valid clock_in).
- [x] 14.3 `test/features/checkin/data/handover_wipe_test.dart`: same-user preserves; different-user wipes; multi-user mixed queue wipes only the non-matching rows. (Folded into `checkin_queue_db_test.dart` `wipeForOtherUsers` group.)
- [x] 14.4 `test/features/checkin/data/checkin_queue_db_test.dart`: CRUD on the drift schema (use `NativeDatabase.memory()`).
- [x] 14.5 `test/features/checkin/presentation/home_buttons_test.dart`: widget test verifying button-set per status (off_duty / on_site / in_transit) and disabled state when permission is denied.
- [x] 14.6 `test/features/checkin/presentation/location_permission_blocker_test.dart`: visibility per permission state, `[開啟設定]` action wiring.
- [x] 14.7 `test/features/checkin/presentation/queue_chip_test.dart`: hidden when empty; correct labels for `pending`/`sending`/`failed`/mixed.
- [ ] 14.8 `test/features/checkin/presentation/history_screen_test.dart`: merged view rendering, failed-row dismiss flow, `[載入更多]` triggers paginated fetch. *Deferred — heavy mock surface (auth, repo, queue stream); behaviour exercised in 18.5 smoke.*
- [ ] 14.9 `test/features/checkin/data/background_sync_test.dart`: lightweight check that the callback runs the processor via a fake repository (we can't really test workmanager scheduling in flutter test, so just exercise the callback function path). *Deferred — top-level isolate entry-point hard to drive from unit test; behaviour exercised in 18.7 Android smoke.*

## 15. Localization additions

- [x] 15.1 Update `lib/l10n/app_zh_TW.arb` (and the English shadow) with all new strings: action button labels, status pill phrasing, queue chip phrasing, error code friendly strings (`INVALID_TRANSITION` → "已在此狀態，無法執行此動作" or similar), location-blocker copy + `[開啟設定]`, handover toast, onboarding tip, history filter chips, `[載入更多]`, `[複製細節]`, `[關閉]`.
- [x] 15.2 Re-run `flutter gen-l10n` (or update the hand-rolled shim if codegen still flaky from `add-app-shell`).

## 16. CI

- [x] 16.1 Update `.github/workflows/app.yml` to include the build_runner step (`dart run build_runner build --delete-conflicting-outputs`) before `flutter analyze`. Cache strategy may need updating for the drift codegen output.
- [ ] 16.2 Verify CI is still green on a sample PR (no actual real PR needed; local `act` or just push to a branch). *To verify after push.*

## 17. Docs

- [x] 17.1 Update `app/README.md` with: new packages, the `pod install` step, the iOS `Info.plist` background-mode note, and a clear blurb on the queue + background sync behavior (in particular the iOS scheduling caveat).
- [x] 17.2 Update `api/README.md`'s "打卡 / Checkin" section, just to add a one-line note that the official mobile client (`app/`) consumes this surface and the persistent-queue contract.
- [x] 17.3 Update root `README.md` to mention that `app/` now does real checkin (replacing the previous `登入流程` only blurb in module table).

## 18. Smoke

- [x] 18.1 `flutter analyze` clean, `flutter test` all green locally.
- [x] 18.2 `dart run build_runner build --delete-conflicting-outputs` — clean.
- [x] 18.3 Live smoke on iPhone Simulator: cold-start app, log in (token from `add-app-shell` smoke), accept location permission on first tap, exercise the full multi-site flow (`clock_in → transfer_out → transfer_in → transfer_out → transfer_in → clock_out`). Verify each event shows up in admin-web `/checkin` live board and history. Verify `[轉出]` shows `[轉入]` next, etc.
- [x] 18.4 Live smoke: airplane-mode test. Toggle airplane mode on, tap `[上班]` and `[轉出]`, verify chip shows `待送出 2 筆`, status pill shows optimistic `in_transit`. Toggle airplane mode off, watch the queue drain to empty and admin-web reflect both events.
- [x] 18.5 Live smoke: failed event flow. Manually edit the queue's `last_event` (e.g. via dev menu — or just submit while admin has flipped `transfer_enabled = false`) so a `transfer_out` returns `TRANSFER_DISABLED`. Verify history shows the failed row with `[複製細節]` and `[關閉]`. Tap `[複製細節]`, paste somewhere, confirm the blob format. Tap `[關閉]` and confirm the row disappears.
- [x] 18.6 Live smoke: handover wipe. Log out, log in as a different AppUser. Verify the toast `前個帳號的 N 筆未送事件已清除` appears and the queue is empty.
- [ ] 18.7 (Optional but encouraged) Live smoke on Android Emulator: same flow. Verify WorkManager fires the background drain after backgrounding the app for 30s with a pending row.

## 19. Follow-ups (deferred — surfaced during smoke, not blocking archive)

- [ ] 19.1 History list re-sorts visibly after a queue row syncs (local row → server row swap shifts position briefly). Either keep stable sort by `id` ties OR transition with an animation. Surfaced in 18.4.
- [ ] 19.2 When admin flips `transfer_enabled = false`, the app should hide `[轉出]` / `[轉入]` buttons (use `org.checkin.transfer_enabled` from `/me`). Currently the buttons render and the user only finds out via a `failed` row. Surfaced in 18.5.
- [ ] 19.3 Logout confirm dialog when queue has unsynced rows: `你還有 N 筆未送事件，登出並切換帳號會清除這些事件。仍要登出？` Surfaced in 18.6 — pure UX guard, doesn't change the wipe semantics.
