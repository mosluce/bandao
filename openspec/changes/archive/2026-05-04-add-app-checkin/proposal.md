## Why

`add-app-shell` produced a runnable Argus iPhone build that can log in and identify the AppUser, but the home screen ends with a `尚未實作` placeholder pill. `add-checkin-events` shipped the full server-side checkin surface — four event types, state machine, dual timestamps, reverse-geocoded region names, transfer toggles, force-checkout — but no real client has exercised it beyond `curl`. This change closes the loop: it makes the phone the actual day-to-day tool an employee uses to clock in, transfer between worksites, and clock out, against the existing `/app/checkin/*` API.

The detail that drove most of the design is that workers will be in remote locations, between buildings, and underground — so offline-first isn't a nice-to-have. Every event lives in a persistent device-local queue with strict serialization (event N waits for the server's `2xx` on N-1) and the UI is optimistic so a tap on `[上班]` immediately changes the status pill, even with no signal. When the network returns, the queue drains in order. When the OS lets us, we drain the queue in the background so the worker's phone in their pocket continues uploading.

## What Changes

- **Home screen action buttons** keyed to current status:
  - `off_duty` → single `[上班]` button.
  - `on_site` → `[下班]` and `[轉出]` buttons.
  - `in_transit` → `[下班]` and `[轉入]` buttons.
  - Tap fires GPS capture → enqueue → optimistic local status update. No bottom-sheet confirm; the tap is the commit.

- **Checkin status pill** replaces the placeholder. Renders the *effective* status — server-confirmed status overlaid with locally-pending events that haven't been rejected. When `on_site`, shows the most recent location's `region_name` once the server has reverse-geocoded it; before that, shows lat/lng. When `on_site` or `in_transit`, shows shift-elapsed counter (`已上班 2 時 14 分`).

- **Persistent event queue** in a new local SQLite database (drift). Every `/app/checkin/events` POST goes through the queue; nothing bypasses it. Schema:

  ```
  pending_events
    id INTEGER PK AUTOINCREMENT
    app_user_id TEXT (snapshot of the AppUser who enqueued — used by the
                     login-handover wipe rule)
    event_type TEXT (clock_in | clock_out | transfer_out | transfer_in)
    lat REAL
    lng REAL
    accuracy REAL nullable
    manual_label TEXT nullable
    occurred_at_client TEXT (RFC3339 with offset, captured at button tap)
    status TEXT (pending | sending | failed)         -- "done" rows are deleted
    attempts INTEGER (incremented on every send try)
    last_error_code TEXT nullable
    last_error_message TEXT nullable
    enqueued_at TEXT
  ```

- **Queue processor** that maintains strict serialization:
  - Single in-flight: only one row in `sending` state at a time.
  - Picks the oldest `pending` row by `occurred_at_client`, marks `sending`, POSTs.
  - `201` → delete the row, advance.
  - `4xx` errors that are state-machine or order-related (`INVALID_TRANSITION`, `OUT_OF_ORDER`, `TRANSFER_DISABLED`, `NEEDS_PASSWORD_CHANGE`) → mark `failed`, do NOT retry. Surface to user.
  - `401` → mark `failed`; signal auth state listener to clear the token; queue paused until next login.
  - `5xx` / network errors → exponential backoff `1, 2, 4, 8, 16, 30s` (capped at 30s), no attempt cap. Returns row to `pending` for the next tick.

- **Optimistic status overlay**: the visible status is computed by replaying the in-queue events on top of the last server-confirmed status. `pending` and `sending` events both contribute. `failed` events are EXCLUDED from the overlay (status rolls back as if they hadn't happened). When the queue drains, the overlay collapses back into the server status naturally.

- **Background sync via `workmanager`**:
  - Android: WorkManager handles enqueued `OneTimeWorkRequest` whenever an event is added or connectivity returns.
  - iOS: `BGTaskScheduler` registers a background-processing task; OS schedules it on its own cadence (no real-time guarantee).
  - The user-facing README and a one-time onboarding tip explain that iOS sync is best-effort and the phone should not be force-quit.

- **Login-handover queue wipe**: on every successful login or auto-login, the queue is filtered against the current `app_user_id`. Rows belonging to a different user are deleted and the user sees a one-shot toast: `前個帳號的 N 筆未送事件已清除`. This guards against device handoff between employees without a clean app uninstall.

- **History screen** at `/history`, accessible from a "事件歷史" entry on home. Renders a unified timeline:
  - Server-fetched events from `GET /app/checkin/events` (cursor pagination via `before` query, page size 50).
  - Local queue rows in `pending`, `sending`, or `failed` state are merged in by `occurred_at_client`.
  - Each row badged: `pending` / `sending` / `failed` / `synced`.
  - `failed` rows show the error code and message inline plus two actions: `[複製細節]` (copies a plaintext summary including event_type, occurred_at_client, lat/lng, error_code, error_message, attempts, last_attempt_at) and `[關閉]` (deletes the failed row from the queue — the only path by which a queued row is user-cancellable). `pending` and `sending` rows are NOT cancellable.

- **Queue indicator on home**: a small chip showing `送出中 / 待送出 N 筆 / 1 筆失敗` — tap routes to `/history` already-filtered to non-`synced` rows. When the queue is empty and online, the chip is hidden.

- **Connectivity awareness**: `connectivity_plus` packaged as a Riverpod provider. Offline → home shows a dim banner `離線中`; queue keeps accepting taps. Online → queue processor wakes immediately. The processor itself doesn't care about connectivity — it tries and lets the network fail organically — but knowing the state lets us avoid backoff growth when offline (we don't increment attempts while connectivity is reported as offline).

- **Location permission flow**:
  - First button tap → request permission via `geolocator`.
  - Granted → capture GPS (`LocationAccuracy.high`, 10s timeout, fallback to `getLastKnownPosition`).
  - Denied → inline blocker: `需要定位權限才能打卡` + `[開啟設定]` button (uses `app_settings`). The home buttons are disabled until permission is granted.
  - Denied-forever (iOS) / "Don't ask again" (Android) → same blocker + persistent "open settings" affordance.

- **DTO mirrors** for `/app/checkin/*` shapes:
  - `CheckinEventType` enum: `clock_in | clock_out | transfer_out | transfer_in`.
  - `AppUserCheckinStatus` enum: `off_duty | on_site | in_transit`.
  - `EventSource` enum: `app | admin_force`.
  - `EventInitiatorKind` enum: `app_user | dashboard_user`.
  - `EventLocation { coordinates: GeoPoint, accuracy_meters?, region_name?, manual_label? }`.
  - `CheckinEventDto`, `CheckinUserStatusDto`, `SubmitCheckinEventRequest`.

- **iOS `Info.plist` + Android manifest** updated with the location permissions. Background mode flags added (`UIBackgroundModes: processing` on iOS; nothing extra on Android — WorkManager handles itself).

- **Localization** (`zh-TW`): event-type labels (`上班/下班/轉出/轉入`), button copy, status pill phrasing, queue chip phrasing, error friendly strings, location-denied blocker copy, "前個帳號的 N 筆未送事件已清除" toast.

- **Tests**: unit on the queue processor (every retry / failure branch), unit on the optimistic-status reducer, unit on the login-handover wipe; widget on home buttons gating by status, on the bottom of the location-denied blocker (button disabled), on the history merged view rendering, on the failed-row dismiss flow.

Out of scope (deferred ROADMAP items, not this change):

- Continuous location tracking between events (a worker's path) — `add-location-tracking`.
- Trajectory map visualization on admin-web — its own ROADMAP entry.
- Auto-checkout heuristics — separate ROADMAP entry.
- Push notifications, deep links into history, biometric unlock.
- Web build (still iOS + Android only).

## Capabilities

### New Capabilities

- `app-checkin`: the entire mobile checkin client surface — home action buttons, optimistic status pill, persistent device-local queue with strict serialization and exp-backoff retry, login-handover queue wipe, foreground+background sync via `workmanager`, location permission UX, history merged view with failed-row dismiss UI, and the DTO mirrors for `/app/checkin/*`.

### Modified Capabilities

(none — all behavior added; `app-shell` is unchanged. The placeholder `尚未實作` pill on home is replaced but it was always documented as a stub waiting for this change, so no spec amendment is required there.)

## Impact

- **Code**: a new `app/lib/features/checkin/` tree (`data/`, `presentation/`, `state/`) and additions to `app/lib/core/api/models/`. A new local SQLite database (drift) requires `drift` + `drift_dev` + `sqlite3_flutter_libs` packages. New runtime deps: `geolocator`, `app_settings`, `workmanager`, `connectivity_plus`, `drift`. New dev deps: `drift_dev`.
- **Native config**: iOS `Info.plist` gains `NSLocationWhenInUseUsageDescription` and `UIBackgroundModes: processing` (with the `BGTaskSchedulerPermittedIdentifiers` entry). Android `AndroidManifest.xml` gains `ACCESS_FINE_LOCATION` + `ACCESS_COARSE_LOCATION`. Both platforms' `Podfile` / `build.gradle` may need version pinning to satisfy the new packages.
- **Build runner**: drift requires `dart run build_runner build` (the same chain we deferred in `add-app-shell`). This change re-introduces the codegen step. The CI workflow under `.github/workflows/app.yml` already runs it.
- **Tests**: 30+ new unit tests + 10+ widget tests; CI Flutter test target stays under a minute.
- **App size**: drift + sqlite3 add ~1 MB to the iOS binary (rough). Acceptable for an internal app.
- **No api or admin-web changes**: this change consumes only existing surfaces. The Rust / TypeScript sides are untouched.
- **iOS background sync caveat**: documented in `app/README.md` and surfaced in a one-shot onboarding tip after first successful login. iOS users may see queue items pause until they reopen the app — this is OS-imposed, not a bug.
- **Downstream**: `add-location-tracking` will reuse this change's queue + GPS plumbing, and the trajectory feature on admin-web depends on tracking data. So this change unblocks the next layer of features.
