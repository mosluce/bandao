## Why

`add-location-tracking-server` shipped the `/app/checkin/locations` ingest
endpoint, the per-AppUser query, the xlsx export, and the `Org.settings.checkin
.location_tracking_enabled` toggle. Nothing currently produces the data —
no AppUser-side collector, no consent flow, no UX. This change is the
client side that turns the empty `location_pings` collection into something
admins actually have a reason to look at.

The previous changes also already laid much of the device-side scaffolding:
`add-app-checkin-polish` brought drift, workmanager, geolocator,
connectivity_plus, and a foreground / background queue processor pattern
that the events flow proved out. We can mirror that pattern at the next
level of complexity (continuous stream rather than discrete tap) instead of
building a parallel system.

`add-org-privacy-policy` shipped a public `/privacy` URL containing the
worker-facing disclosure (90-day retention, "若您所屬組織開啟此功能時"
phrasing). The consent dialog this change introduces links to that URL.

Deferring further would leave the server-side feature shipped but unused.

## What Changes

- **Native config**: iOS `Info.plist` adds `location` to
  `UIBackgroundModes` (alongside the existing `processing`); the
  `NSLocationWhenInUseUsageDescription` string is updated to mention
  ongoing tracking. Android `AndroidManifest.xml` gains
  `FOREGROUND_SERVICE` and `FOREGROUND_SERVICE_LOCATION` permissions.
  Bumps `pubspec.yaml` version to 0.3.0+3.

- **`pending_location_pings` drift table** (new), parallel to the existing
  `pending_events` table: `id PK, app_user_id, lat, lng, accuracy?,
  occurred_at_client, status, attempts, last_error_code, last_error_message,
  last_attempt_at, enqueued_at`. Index on `(status, occurred_at_client)`
  for the batch-pick query.

- **`LocationTrackingService`** (new): wraps `Geolocator.getPositionStream`
  with `LocationAccuracy.high`, `distanceFilter: 100`,
  iOS `pausesLocationUpdatesAutomatically: false` +
  `showBackgroundLocationIndicator: true`, Android
  `foregroundNotificationConfig` with sticky notification. Adds a
  client-side 60-second throttle on top of the OS-level distance filter
  so we honor the "60s AND 100m" sample rule.

- **`LocationTrackingController`** (new): a `Notifier` that decides start
  vs stop based on two providers — start when
  `checkinStatusProvider` reports a server-confirmed status of `on_site`
  or `in_transit`; stop when `effectiveStatusProvider` reports
  `off_duty`. Cold-start path included: if the app boots and the server
  status is non-`off_duty`, the tracker starts on first frame. Transfer
  events do NOT cycle the tracker (it stays running across `on_site` ↔
  `in_transit`).

- **`LocationPingProcessor`** (new): the batch-flush counterpart to the
  existing `QueueProcessor`. Picks up to 100 pending pings, sends via
  `POST /app/checkin/locations`, deletes successful rows from drift,
  marks rows in the response's `rejected[]` array as `failed`. Triggers:
  ≥30 unsent rows, ≥5 minutes since last flush, connectivity transition
  to online, `clock_out`-driven shift-end (final flush). Backoff on
  network / 5xx: same 1/2/4/8/16/30s schedule as events.

- **Consent dialog**: shown the first time the AppUser presses `[上班]`
  (clock_in) while `org.location_tracking_enabled = true` and they
  haven't previously consented on this device for this account. Copy
  includes the 90-day retention disclosure and a tappable link to
  `<admin-web URL>/privacy`. `[取消]` aborts the clock_in entirely (no
  event enqueued); `[同意並上班]` sets a per-AppUser consent flag in
  secure storage and proceeds with the existing `enqueueEvent` flow.
  Subsequent shifts skip the dialog forever (org toggle flipping
  off→on does NOT re-prompt — admin's state-lock guarantee means workers
  are off-duty during flips).

- **Home tracking chip**: a new always-visible chip on the home screen
  shown whenever `LocationTrackingController.isRunning == true`.
  Renders `📍 定位追蹤中` plus the elapsed time since tracking started
  ("追蹤中 · 02:14"). Visually distinct from the existing
  queue-processor chip (which represents events queue, not pings).

- **Force-quit recovery banner**: on app boot, if server status is
  non-`off_duty` AND the secure-storage flag
  `tracking.last_clean_stop` is missing or older than the most recent
  `enqueued_at` of any local row, a one-shot banner surfaces on home
  reading `定位追蹤上次中斷過，已恢復記錄`. The flag is cleared when
  the tracker starts, set when it stops cleanly. force-quits / OS kills
  cannot set it → next boot detects the gap.

- **`LocationRepository.submitBatch`** (new): wraps `POST /app/checkin/
  locations`, returns the typed
  `SubmitLocationPingsResponse { acceptedCount, rejected[] }`. Throws
  the existing `ApiException` shape; `LOCATION_TRACKING_DISABLED` (403)
  is mapped via the existing error interceptor.

- **DTO mirrors** for the location-tracking surface:
  `LocationPingDto`, `SubmitLocationPingsRequest`, `LocationPingInput`,
  `SubmitLocationPingsResponse`, `RejectedPingDto`. Hand-rolled per the
  existing pattern.

- **Privacy URL config**: a new compile-time constant in
  `core/env/env.dart` (`Env.privacyUrl()`) that defaults to the
  admin-web localhost URL in dev and the prod admin-web URL via
  `--dart-define=PRIVACY_URL=...`. Dev menu adds a row for runtime
  override (parallel to the existing `api_base_url` override).

- **Localization**: zh-TW strings for the consent dialog (title /
  body / buttons / privacy link), the home tracking chip,
  the force-quit recovery banner, and the iOS background-tip update
  ("定位追蹤期間請勿關閉 Argus").

- **App resume hook extension** (already present in
  `add-app-checkin-polish`): no change — the hook already refreshes
  `/me` and `checkinStatusProvider`, which propagates org toggle changes
  and triggers the tracker controller to re-evaluate start / stop.

- **Tests**: unit tests for the controller's start/stop matrix
  (off_duty → no, server on_site (cold start) → yes,
  pending clock_in → no, server-confirmed clock_in → yes,
  effective off_duty (clock_out tap) → stop, transfer events → no
  cycling). Unit tests for the batch processor (happy path, partial
  reject from `rejected[]`, network retry, batch-size cap). Drift
  schema CRUD tests. Widget tests for consent dialog (skip if already
  consented, cancel aborts, confirm proceeds, link tappable). Widget
  test for home tracking chip rendering. Widget test for force-quit
  recovery banner.

## Capabilities

### New Capabilities

(none — `app-checkin` already exists and absorbs the new requirements.)

### Modified Capabilities

- `app-checkin`: gains location-tracking-specific requirements as
  ADDED entries — consent dialog gating, tracking lifecycle bound to
  server status, batch flush rules, force-quit recovery, home tracking
  chip. The existing requirements (events queue, status pill, history,
  etc.) are unchanged.

## Impact

- **Code**: new `LocationTrackingService`, `LocationTrackingController`,
  `LocationPingProcessor`, `LocationRepository`, ping DTOs, drift
  schema additions, consent dialog, home tracking chip widget,
  force-quit banner widget. Changes confined to
  `app/lib/features/checkin/` and `app/lib/core/api/models/`.

- **Native config**: iOS `Info.plist` 1-line add (`location` to
  `UIBackgroundModes`), updated `NSLocationWhenInUseUsageDescription`
  copy. Android manifest 2-line add. iOS `pod install` (no new pods —
  geolocator already supplies the native code).

- **No new pub deps**: geolocator, app_settings, drift, workmanager,
  connectivity_plus all reused. zero net deps added.

- **Build runner**: drift schema regen via `dart run build_runner build
  --delete-conflicting-outputs`. CI workflow already covers this.

- **Battery + privacy**: visible iOS blue bar / Android sticky
  notification while tracker is active. Documented in updated
  `app/README.md` "Polish iteration" successor section.

- **App size**: no measurable impact (no new pkgs).

- **Server contract**: unchanged. This change consumes
  `add-location-tracking-server`'s endpoints exactly as specced.

- **Downstream**: unblocks `add-location-tracking-dashboard`'s
  trajectory page (live data starts arriving on the server once any Org
  enables the toggle and a worker takes a shift).

Out of scope (deferred ROADMAP):

- Worker-facing "我的軌跡" page in app (個資法 §10 self-access). Workers
  exercise the right via admin export.
- Pause / resume tracker from a UI affordance (only auto-controlled by
  shift status).
- Native channel for in-app heading / speed (just lat/lng/accuracy).
- Per-shift battery summary ("今天定位追蹤耗電 3%").
