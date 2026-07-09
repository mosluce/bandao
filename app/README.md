# bandao_app

Flutter mobile client for AppUsers (the workers checking in and out of sites).
Talks to the Bandao API at `/app/*`. Built for iOS + Android only.

> Production cut release runbook：見 [`DEPLOY.md`](../DEPLOY.md) 的「App cut release」章節。

## Stack

- Flutter 3.29 (`.tool-versions`-pinned via asdf)
- State: `flutter_riverpod` 2.x
- Routing: `go_router`
- HTTP: `dio` with bearer-token interceptor
- Local SQLite queue: `drift` (with `drift_dev` codegen via `build_runner`)
- Location: `geolocator` + `app_settings`
- Connectivity: `connectivity_plus`
- Background sync: `workmanager` (Android `WorkManager` / iOS `BGTaskScheduler`)
- Locale: zh-TW only, hand-rolled `app_localizations.dart` shim

## First-time setup

```bash
asdf install                        # ruby 3.4.4 (for CocoaPods)
flutter pub get
dart run build_runner build --delete-conflicting-outputs
cd ios && pod install && cd -
```

The codegen step generates the drift bindings for the checkin queue. CI runs
this between `pub get` and `analyze`; locally re-run it any time you touch
the `@DriftDatabase` schema.

## Run

```bash
flutter run -d "iPhone 17"           # boot iPhone Simulator first
flutter run -d emulator-5554          # Android emulator
```

The default API base URL is `http://localhost:9090` on iOS Simulator and
`http://10.0.2.2:9090` on Android. Override at runtime from the **伺服器設定**
link on `/login` (route `/server-config`). In debug the override is loose
(any `http`/`localhost`/LAN IP is accepted); in release only `https` URLs
with a host are accepted (see `Self-hosted server` below).

## Self-hosted server

The repo is public: anyone can deploy their own `api/` (+ Mongo) backend and
point the shipped app at it — no need to publish your own build. On `/login`,
open **伺服器設定**, enter your `https://` API base URL, and save. The login
screen shows which server you're connected to (official default vs. a custom
host). Changing the server clears the stored bearer token, so you re-log in
against the new backend.

Constraints (v1): release builds accept **https public URLs only** — plain
`http`, LAN IPs, and self-signed hosts are not supported (this also means no
iOS ATS / Android cleartext exception is needed). Deploy `api/` behind a TLS
domain. Native requests carry no CORS, so nothing on the backend needs to
change for the app to reach it.

## Testing

```bash
flutter test
```

The full suite is unit + widget tests, no device required. The drift queue
tests use `NativeDatabase.memory()`, so no SQLite file is created.

## Background sync caveat (iOS)

`workmanager` schedules a `BGProcessingTask` with identifier
`tw.ccmos.app.bandao.queue-drain`. iOS does NOT guarantee when this runs —
the OS may delay it for hours, especially on locked devices, low battery,
or when the user has rarely opened the app. **Do not force-quit 班到 while
the queue chip on home shows pending events.** A force-quit prevents the
OS from waking the app for that task identifier ever again until the user
relaunches.

Android's WorkManager is much more reliable; an enqueue triggers a
`OneTimeWorkRequest` with `networkType: connected` that fires within
seconds of being online.

A one-shot onboarding tip on home (iOS only) reminds users of this on
first login. The dismissed flag lives in `home.background_sync_tip_seen`.

## Native config notes

- iOS `Info.plist` adds:
  - `NSLocationWhenInUseUsageDescription` (zh-TW)
  - `UIBackgroundModes: <array><string>processing</string></array>`
  - `BGTaskSchedulerPermittedIdentifiers: ["tw.ccmos.app.bandao.queue-drain"]`
- Android `AndroidManifest.xml` adds `ACCESS_FINE_LOCATION` and
  `ACCESS_COARSE_LOCATION` `<uses-permission>` entries.

If you bump native deps, re-run `pod install` from `app/ios/`.

## Code layout

```
lib/
  app/                router.dart, bandao_app.dart (root)
  core/
    api/              dio client, interceptors, DTOs
    env/              compile-time base URL
    storage/          secure storage + dev override
  features/
    auth/             login, splash, force-change-password, home shell
    checkin/
      data/           drift db, repo, queue processor, background sync
      state/          riverpod providers (status, queue, effective, etc.)
      presentation/   home buttons, status pill, history, etc.
  l10n/               hand-rolled localization shim
```

## Polish iteration

`add-app-checkin-polish` layered the following on top of the initial checkin
client without touching the API surface:

- **Just-synced events stay visible in `/history`** — a small in-memory
  `recentlySyncedEventsProvider` carries the server's `CheckinEventDto` from
  each successful submit so the row at that `occurred_at_client` doesn't
  disappear during the brief gap between queue-row deletion and the next
  paginated server fetch. The merge dedupes by event `id`.
- **Transfer buttons hide when the org disables transfer** —
  `HomeButtons` now reads `auth.org.checkin.transferEnabled` and collapses
  the on-site / in-transit sets to `[下班]` only when transfers are off.
- **Logout asks for confirmation when the queue is non-empty** — the home
  `…` menu's `登出` action surfaces a dialog with the row count
  (pending + sending + failed) before clearing the session, so a
  device-handover doesn't silently wipe data.
- **App resume refreshes `/me` and the checkin status** — the
  `WidgetsBindingObserver` hook on the home screen now also calls
  `authProvider.refreshMe()` and `checkinStatusProvider.refresh()` (in
  addition to the existing permission re-check). Resume is NOT a login
  event, so the handover wipe does NOT run from this path.
- **`/history` supports pull-to-refresh** — clears the recently-synced
  cache, resets the paginated server-events list, refetches the first page,
  and refreshes `checkinStatusProvider`.

## Location tracking (shift trajectory)

`add-location-tracking-app` introduced continuous GPS tracking during a
worker's shift. The feature is **opt-in per Org** — admins flip a toggle on
the `Org` document (`org.checkin.locationTrackingEnabled`); the worker sees a
first-time consent dialog before the very first `[上班]` and tracking only
runs while the server-confirmed status is non-`off_duty`.

### Lifecycle

| Event | Trigger | Behavior |
|-------|---------|----------|
| **Start** | `checkinStatusProvider` resolves to `on_site` / `in_transit` (server-confirmed) | `LocationTrackingController` calls `service.start()`. Conservative — a pending `clock_in` that has NOT been uploaded does NOT start the tracker. |
| **Stop** | `effectiveStatusProvider` flips to `off_duty` (server OR optimistic) | Tracker stops immediately — a `[下班]` tap kills the GPS stream before the server confirms. If the clock-out fails and effective rolls back, the start path picks up again. |
| **No-op** | `transfer_in` / `transfer_out` | Tracker keeps running — transfers stay on-shift. |

### Pings & throttle

Two filters AND together: the OS distance filter (100m) and a 60-second
client-side throttle on `_lastEnqueuedAt`. Idle workers produce ~0 rows;
walking workers produce ~1 row/min. Each row lands in the `pending_location_pings`
drift table (`schemaVersion: 2`) with the `pending` status.

### Batch upload

`LocationPingProcessor` flushes to `POST /app/checkin/locations` when ANY of:

- Pending count `>= 30`
- 5 minutes have elapsed since the last successful flush
- A shift-end signal (`flushFinal()`) — fired when status transitions to `off_duty`
- Connectivity restored to online

Server response carries `acceptedCount` and an optional `rejected[]` array
(by index). Both accepted and rejected rows are deleted from drift —
rejected rows are silently logged (`logger.w(...)`), not surfaced as a UI
error, since the per-ping rejection reasons (`INVALID_PING_TIMESTAMP`,
`OUT_OF_RANGE`, etc.) are not actionable for the worker mid-shift.

403 with code `LOCATION_TRACKING_DISABLED` (Org admin flipped the toggle
mid-shift) tears down the tracker via `onTrackingDisabled` and clears
in-flight rows. 401 invokes `onAuthExpired` and pauses further ticks until
the next login.

### Force-quit recovery

`last_clean_stop` (secure storage) is written every time `service.stop()`
runs cleanly. On app boot, `TrackingRecoveryBanner` checks: if the server
status is non-`off_duty` AND `last_clean_stop` is missing or older than the
newest `enqueuedAt` row, it shows a one-shot banner (`定位追蹤上次中斷過，
已恢復記錄`) and the controller's normal start path resumes tracking.

### Consent dialog

`showLocationConsentDialog` runs once per AppUser per device — the consent
flag (`bandao.location_tracking.consent.<appUserId>`) is keyed on the user
id so a different login on the same device gets prompted again. The dialog
covers cadence / distance / retention / audience and links out to the full
privacy policy via `url_launcher` (`LaunchMode.inAppBrowserView`).

### Privacy URL override (server-config screen)

The **伺服器設定** screen adds a parallel "隱私政策網址" override row mirroring
the API base URL pattern. The compile-time default comes from
`--dart-define=PRIVACY_URL=...` falling back to `http://localhost:3000/privacy`.
The override lives at `secureStorage["dev.privacy_url_override"]` — a release
build with no override falls back to the dart-define default. (Unlike the API
base URL, the privacy override stays loosely validated; it targets admin-web,
not the self-hosted `api/`.)

### iOS / Android specifics

- **iOS**: `UIBackgroundModes: location` is set in `Info.plist`. While
  tracking, the system blue bar is visible (`showBackgroundLocationIndicator: true`).
  Permission scope is `whileInUse` (mirroring the Google Maps pattern, not
  `always` — the blue bar serves as the user-visible cue).
- **Android**: `FOREGROUND_SERVICE` + `FOREGROUND_SERVICE_LOCATION` permissions
  declared in the manifest. Geolocator's `ForegroundNotificationConfig`
  posts a sticky notification (`工作期間定位追蹤中`) so the OS won't kill
  the process under memory pressure.

## Provisional bundle id

`tw.ccmos.app.bandao`. Final id is decided pre-store-submit.
