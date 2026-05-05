## 1. Native config

- [x] 1.1 iOS `Info.plist`: add `location` to the existing `UIBackgroundModes` array (currently has `processing` for BGTaskScheduler). Update `NSLocationWhenInUseUsageDescription` copy to mention ongoing tracking — something like `Argus 需要您的位置記錄打卡事件以及工作期間的軌跡。`.
- [x] 1.2 Android `AndroidManifest.xml`: add `<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />` and `<uses-permission android:name="android.permission.FOREGROUND_SERVICE_LOCATION" />` (the latter required Android 14+). Geolocator's plugin already declares the runtime location permissions; we keep the existing `ACCESS_FINE_LOCATION` / `ACCESS_COARSE_LOCATION`.
- [x] 1.3 Bump `app/pubspec.yaml` `version: 0.3.0+3`.
- [x] 1.4 `cd app/ios && pod install` (no new pods expected — geolocator native is already pulled — but re-run to refresh project state).

## 2. Drift schema additions

- [x] 2.1 In `app/lib/features/checkin/data/checkin_queue_db.dart`, add a new `@TableIndex(name: 'idx_pending_loc_status_time', columns: {#status, #occurredAtClient})` annotation and a `class PendingLocationPings extends Table` with columns `id (autoincrement int)`, `appUserId (text)`, `lat (real)`, `lng (real)`, `accuracy (real, nullable)`, `occurredAtClient (text)`, `status (text, default 'pending')`, `attempts (int, default 0)`, `lastErrorCode (text, nullable)`, `lastErrorMessage (text, nullable)`, `lastAttemptAt (text, nullable)`, `enqueuedAt (text)`. Add to `@DriftDatabase(tables: [...])`.
- [x] 2.2 Bump `schemaVersion` to `2` in `CheckinQueueDb`. Add a `migration` strategy in `MigrationStrategy.onUpgrade` that creates the new table when migrating from `v1` (use `m.createTable(pendingLocationPings)`).
- [x] 2.3 Add typed queries on the database: `enqueueLocationPing(PendingLocationPingsCompanion)`, `pickPendingLocationBatch(int max)` returning up to `max` oldest `pending` rows, `markLocationSending(List<int>)`, `markLocationFailed(int, code, message)`, `deleteLocationPings(List<int>)`, `pendingLocationCountForUser(String) -> Future<int>`, `latestPendingLocationEnqueuedAt() -> Future<DateTime?>`.
- [x] 2.4 Run `dart run build_runner build --delete-conflicting-outputs`. Verify regeneration succeeds and `flutter analyze` is clean.

## 3. DTO mirrors

- [x] 3.1 In `app/lib/core/api/models/`, create `location_ping.dart` with `LocationPingDto` (same fields as `CheckinEventDto`'s `EventLocation`-flavored shape — `id, appUserId, lat, lng, accuracyMeters?, occurredAtClient, occurredAtServer`). Hand-rolled per the existing pattern (explicit `fromJson`, `toJson`, `==`, `hashCode`).
- [x] 3.2 In the same file, add `LocationPingInput` (the request-body shape: `lat, lng, accuracy?, occurredAtClient`), `SubmitLocationPingsRequest { pings }`, `SubmitLocationPingsResponse { acceptedCount, rejected }`, and `RejectedPingDto { index, code, message }`. All hand-rolled.
- [x] 3.3 In `app/lib/core/api/api_error.dart`, add `static const String locationTrackingDisabled = 'LOCATION_TRACKING_DISABLED';` to the `ApiErrorCode` constants. Update the `friendlyZh` extension to map this code to e.g. `'定位追蹤已被組織關閉'`.

## 4. LocationRepository

- [x] 4.1 Create `app/lib/features/checkin/data/location_repository.dart`. Class `LocationRepository(this._dio)`. Method `Future<SubmitLocationPingsResponse> submitBatch(SubmitLocationPingsRequest req)` that POSTs to `/app/checkin/locations` and unwraps `DioException` via the existing `_unwrap` pattern. Riverpod-providable as `locationRepositoryProvider` (FutureProvider that builds from `apiClientProvider`).

## 5. LocationTrackingService

- [x] 5.1 Create `app/lib/features/checkin/data/location_tracking_service.dart`. Wraps `Geolocator.getPositionStream`. Public surface: `Future<void> start()`, `Future<void> stop()`, `bool get isActive`, `DateTime? get startedAt`, `Stream<DateTime> get tickStream` (emits per-second ticks while active so the chip can rebuild).
- [x] 5.2 Inside `start`, configure platform settings via `Geolocator.getPositionStream(locationSettings: ...)`. Use `AppleSettings(accuracy: high, distanceFilter: 100, pausesLocationUpdatesAutomatically: false, showBackgroundLocationIndicator: true, activityType: ActivityType.other)`. Use `AndroidSettings(accuracy: high, distanceFilter: 100, foregroundNotificationConfig: ForegroundNotificationConfig(notificationTitle: 'Argus', notificationText: '工作期間定位追蹤中', enableWakeLock: true, setOngoing: true))`.
- [x] 5.3 Subscribe to the stream. On each `Position` event, throttle: track `_lastEnqueuedAt`; if `now - _lastEnqueuedAt < 60s`, drop. Otherwise insert into drift via `db.enqueueLocationPing(...)` with `occurredAtClient = nowOccurredAtClient(now)`, `appUserId` from the currently-authenticated user, status `pending`.
- [x] 5.4 On `stop`, cancel the subscription, set `_isActive = false`, set `_startedAt = null`. Set the secure-storage flag `argus.location_tracking.last_clean_stop` to the current ISO8601 timestamp. Cancel the per-second tick timer.
- [x] 5.5 Riverpod-providable as `locationTrackingServiceProvider` (Provider — singleton tied to container lifetime).

## 6. LocationTrackingController

- [x] 6.1 Create `app/lib/features/checkin/state/location_tracking_controller.dart`. Class `LocationTrackingController` is a Riverpod `Notifier<bool /* isRunning */>` (or `Provider<void>` with `keepAlive` if a `Notifier` feels heavyweight; designer call). On `build()`, set up two `ref.listen` subscriptions:
  - On `checkinStatusProvider`: when the new value's status is non-`off_duty`, call `_maybeStart()`.
  - On `effectiveStatusProvider`: when the new value's status is `off_duty`, call `_maybeStop()`.
- [x] 6.2 `_maybeStart` is idempotent — early-return if already running. Calls `service.start()`. Updates the notifier state to `true`.
- [x] 6.3 `_maybeStop` is idempotent — early-return if already stopped. Calls `service.stop()`. Updates state to `false`.
- [x] 6.4 Expose `DateTime? get startedAt` (proxy to service) so the chip can compute elapsed.
- [x] 6.5 Cold-start path: in the listen-on-status, the very first emission (initial fetch resolution) flowing through is the cold-start signal. No special-casing needed — `_maybeStart` triggers the same way.

## 7. LocationPingProcessor

- [x] 7.1 Create `app/lib/features/checkin/data/location_ping_processor.dart`. Mirrors the structure of the existing `QueueProcessor` but with batch semantics. Constructor injection of `CheckinQueueDb`, `Future<LocationRepository> Function()`, `bool Function() isOnline`, `Future<void> Function()? onAuthExpired`, `Future<void> Function()? onTrackingDisabled` (the 403 handler). `Future<void> tick()` is re-entrant-safe via an internal `_running` guard.
- [x] 7.2 Inside `tick`, evaluate triggers: `pendingCount >= 30` OR `now - _lastFlushAt >= 5min` OR `_pendingFinal == true` (the shift-end flush flag). If none satisfied, return.
- [x] 7.3 Pick batch via `db.pickPendingLocationBatch(100)`. Mark all picked rows as `sending` in one update.
- [x] 7.4 Call `repo.submitBatch(...)`. On success: delete inserted rows (computed by exclusion of `rejected[].index`); also delete `rejected[]` rows silently with a logger.warn. Set `_lastFlushAt = now`.
- [x] 7.5 On `ApiException` with code `LOCATION_TRACKING_DISABLED`: delete all in-flight rows from drift; invoke `onTrackingDisabled?.call()`; the controller's `_maybeStop()` runs and the tracker hides.
- [x] 7.6 On `ApiException` with status 401: delete all in-flight rows; invoke `onAuthExpired?.call()`; queue paused (no further tick until next login).
- [x] 7.7 On 5xx / network: mark in-flight rows back to `pending`, set `last_attempt_at = now`, increment `attempts`. Use the existing `nextDelay(attempts)` from the events queue's `QueueProcessor` for the backoff schedule.
- [x] 7.8 Add `flushFinal()` method that sets `_pendingFinal = true` and calls `tick()`, then in `tick` after a successful flush, recursively call `tick` again until `pendingCount == 0` (or a non-success outcome is hit).

## 8. Wiring + lifecycle

- [x] 8.1 In the foreground sync runner (`queueProcessorRunnerProvider` or its sibling — a new `locationPingProcessorRunnerProvider`), wire wake triggers: drift change stream on the new table, connectivity transition to online, 1Hz timer.
- [x] 8.2 Hook into the events queue's `clock_out` success path. When the events processor's `onStatusFresh` fires with `off_duty`, invoke `locationPingProcessor.flushFinal()` to drain any remaining pings. (Find an extension point on the events processor — likely a new optional `onStatusFresh` consumer or a separate listener that watches `checkinStatusProvider` for off_duty transitions.)
- [x] 8.3 In `CheckinActions.enqueueEvent` for `clock_in`: before the existing flow, check `org.checkin.locationTrackingEnabled` (from auth state) and the secure-storage consent flag. If toggle is on AND consent flag is missing, surface the consent dialog. The dialog returns a bool; `false` aborts (no enqueue), `true` writes the consent flag and proceeds.
- [x] 8.4 In `app/lib/main.dart` or `app/lib/app/argus_app.dart`, ensure the `LocationTrackingController` is bootstrapped at app start (via `ref.watch(...)` somewhere that survives the lifetime — likely in `argus_app.dart` near the existing queue-processor wiring).

## 9. Privacy URL config

- [x] 9.1 In `app/lib/core/env/env.dart`, add `static const String _privacyUrlDartDefine = String.fromEnvironment('PRIVACY_URL');` and `static String privacyUrl()` — returns the dart-define if non-empty, else `http://localhost:3000/privacy` (dev default). Mirror the existing `compileTimeDefault` pattern.
- [x] 9.2 Add `dev.privacy_url_override` to `SecureStorageKeys` (in `core/storage/secure_storage.dart`). Add read / write / clear methods. The dev menu page (`/dev-server-config`) gets a parallel section for "Privacy URL override".
- [x] 9.3 Update the dev menu screen (`features/auth/presentation/dev_server_config_screen.dart`) to include the privacy URL override. Layout mirrors the existing API base URL row.
- [x] 9.4 Add a resolver helper `core/storage/privacy_url.dart` (mirroring `api_base_url.dart`) — Riverpod async provider that returns the override or the env default.

## 10. Consent dialog UI

- [x] 10.1 Create `app/lib/features/checkin/presentation/location_consent_dialog.dart`. A `showDialog<bool>` returning `true` on confirm, `false` (or null) on cancel. Title: `啟用定位追蹤`. Body has bullet points covering: per-minute sampling cadence, 100m distance filter, 90-day retention, who reads (your Org admin), and a tappable "查看完整政策" link that opens `<privacyUrl>/privacy` via `url_launcher` with `LaunchMode.inAppWebView`.
- [x] 10.2 Buttons: `[取消]` / `[同意並上班]`. Default focus on the confirm button.
- [x] 10.3 Add `url_launcher` to `pubspec.yaml` dependencies (latest stable; ~80 KB).
- [x] 10.4 In `CheckinActions`, before clock_in enqueue:
  ```
  if (eventType == clockIn && org.location_tracking_enabled && !consented):
    bool agreed = await showLocationConsentDialog(context);
    if (!agreed) return EnqueueOutcome.consentDeclined;
    secureStorage.writeLocationConsent(appUserId);
  ```
  Add `EnqueueOutcome.consentDeclined` enum variant. The HomeButtons handler shows no error toast for this outcome — silent no-op (user explicitly cancelled).

## 11. Home tracking chip

- [x] 11.1 Create `app/lib/features/checkin/presentation/tracking_chip.dart`. Widget renders only when `LocationTrackingController.isRunning == true`. Reads `controller.startedAt` and re-renders on the controller's tick stream. Shows `📍 定位追蹤中 · MM:SS` formatted as elapsed time.
- [x] 11.2 Insert the new chip into the home screen layout next to the existing `QueueChip`. Both can be visible simultaneously.

## 12. Force-quit recovery banner

- [x] 12.1 Create `app/lib/features/checkin/presentation/tracking_recovery_banner.dart`. A widget that on mount checks: `auth.user.id != null`, server status is non-`off_duty`, and the `last_clean_stop` flag is missing or older than the latest pending row's `enqueued_at`. If all true, render a dismissible material banner with the text `定位追蹤上次中斷過，已恢復記錄`. Auto-dismiss after 10 seconds.
- [x] 12.2 Insert into the home screen layout (above the chips area).
- [x] 12.3 In `LocationTrackingService.start`, clear the `last_clean_stop` key.

## 13. Localization

- [x] 13.1 Add zh-TW strings to `app/lib/l10n/app_localizations.dart` (and the English shadow): consent dialog title / body / buttons / privacy link label, tracking chip label, recovery banner text, error friendly string for `LOCATION_TRACKING_DISABLED`, dev menu privacy URL row label.

## 14. Tests

- [x] 14.1 `test/features/checkin/data/location_tracking_service_test.dart` — UNIT test on the throttle logic. Inject a fake position stream; emit 5 positions in 30 seconds; verify only one `enqueueLocationPing` call was made.
- [x] 14.2 `test/features/checkin/state/location_tracking_controller_test.dart` — controller start/stop matrix (fake service, fake providers): off_duty → no start; pending clock_in (effective on_site, server off_duty) → no start; server-confirmed on_site → start; transfer events → no cycling; effective off_duty → stop; server status rebound after clock_out failure → re-start.
- [x] 14.3 `test/features/checkin/data/location_ping_processor_test.dart` — happy path (5 valid pings → 5 deleted), partial reject (5 sent, 1 rejected → 5 deleted with warn), 30-row threshold trigger, 5-min trigger (use injected clock), connectivity restoration trigger, `flushFinal` drains all, 5xx returns rows to `pending`, 403 stops tracker + clears in-flight, 401 pauses queue.
- [x] 14.4 `test/features/checkin/data/checkin_queue_db_test.dart` — extend the existing tests with `pendingLocationPings` CRUD: enqueue, pickPendingLocationBatch, markLocationSending, markLocationFailed, deleteLocationPings, pendingLocationCountForUser.
- [x] 14.5 `test/features/checkin/presentation/location_consent_dialog_test.dart` — widget test: dialog renders all expected copy elements, cancel returns false (and no consent written), confirm returns true (and consent flag written), privacy link tap launches in-app browser (verified via the `url_launcher` mock).
- [x] 14.6 `test/features/checkin/presentation/tracking_chip_test.dart` — widget test: chip hidden when controller inactive, chip visible with correct elapsed format when active, elapsed time updates after the tick stream emits.
- [x] 14.7 `test/features/checkin/presentation/tracking_recovery_banner_test.dart` — widget test: banner hidden when off_duty, banner visible when status is on_site AND last_clean_stop is missing, banner auto-dismisses after 10s, banner dismissible by user.

## 15. Documentation

- [x] 15.1 Append a "Location tracking" section to `app/README.md` covering: how to enable for an Org (admin-web), how the consent flow works, the iOS blue bar and Android sticky notification, force-quit handling, and the dev menu privacy URL override.
- [x] 15.2 Update root `README.md`'s `app/` description to mention「上班期間軌跡記錄（Org toggle 開啟）」.

## 16. CI

- [x] 16.1 Verify `.github/workflows/app.yml` still passes after the schema bump and new tests. The build_runner step handles the drift codegen change automatically.

## 17. Smoke

- [x] 17.1 `flutter analyze` clean, `flutter test` all green locally.
- [x] 17.2 `dart run build_runner build --delete-conflicting-outputs` clean.
- [x] 17.3 Live smoke on iPhone Simulator: enable Org toggle in admin-web; cold-start app; tap `[上班]` → consent dialog appears with privacy link; tap link → in-app browser shows the privacy page; close → back to dialog; confirm → clock_in proceeds.
- [x] 17.4 Live smoke: while `[上班]`-confirmed (server confirmed on_site), simulate movement on the iOS Simulator (`Features → Location → City Run` etc.). Verify pings appear in `location_pings` collection within ~1 minute (via mongosh or the admin endpoint). Verify `📍 定位追蹤中` chip is visible.
- [x] 17.5 Live smoke: tap `[下班]`. Verify chip disappears immediately; verify a final flush sends remaining pings; verify clean-stop flag is set.
- [x] 17.6 Live smoke (force-quit): repeat 17.3 to start tracking; force-quit Argus from the multitasking switcher; reopen Argus. Verify the recovery banner appears + tracker auto-restarts.
- [x] 17.7 Live smoke (toggle off mid-batch): admin disables the Org toggle while the worker is off-duty; have the worker clock_in (toggle change requires off_duty so this is the realistic flow); verify the consent dialog does NOT appear (toggle is now false) and no pings flow.
- [x] 17.8 ~~Deferred~~ — Android emulator smoke 推遲到 Android beta release 前，已加進 ROADMAP side ideas。
