## Context

`add-location-tracking-server` shipped the API endpoints and toggle. This
change is the AppUser-facing collector + UX. Two prior changes did most of
the foundational work that this one builds on:

- `add-app-checkin` introduced drift, workmanager, geolocator, the queue
  processor pattern, and the optimistic-status reducer.
- `add-app-checkin-polish` added `recentlySyncedEventsProvider`, the
  app-resume `/me` refresh, and `RefreshIndicator` on history. Those
  pieces give this change the data-flow and lifecycle hooks it needs.

The technical core is "background location collector that writes to a
local queue and batch-uploads". The non-trivial parts are at the edges:

- Tracking lifecycle is bound to two providers (server-confirmed status
  for start, effective status for stop) — different ends use different
  providers because the asymmetry matches privacy expectation. Workers
  shouldn't be tracked before they've actually clocked in (server
  confirmation), and they shouldn't be tracked after they've tapped clock
  out (immediate, before server confirms).
- iOS allows whenInUse + UIBackgroundModes:location to track in the
  background only as long as the app process exists. Force-quit kills it
  with no recovery. Android FGS via geolocator is similar (sticky
  notification protects against swipe-up but not "force stop").
- Consent must be taken before the first ping but the ping flow runs out
  of `enqueueEvent` (events queue, not pings queue). The dialog
  intercepts the clock_in tap and either gates or aborts the entire
  shift-start flow.

## Goals / Non-Goals

**Goals:**

- Land the AppUser-side collector cleanly so an Org enabling the toggle
  starts seeing real `location_pings` flowing within seconds of the
  next worker shift.
- Honor the explore-locked sample rule (60s AND 100m, AND condition,
  client-side throttle on top of OS distance filter).
- Keep the tracker's start/stop reasoning testable in pure Dart — no
  platform channel mocks needed for the lifecycle decisions; the
  service-vs-controller split makes that possible.
- Reuse the existing queue processor pattern (drift table + 1Hz tick +
  exp backoff + foreground / background sync) rather than introducing a
  parallel architecture.
- Make consent first-class: the dialog gates the shift; cancel aborts
  cleanly; copy is precise about retention.
- Make force-quit recovery a positive UX moment ("we noticed and
  recovered") rather than a silent failure.

**Non-Goals:**

- A separate "I'm currently tracking" panel with rich stats (just a chip).
- Manual override to keep tracking after clock_out (out of scope, against
  the design intent).
- Real-time live-location preview to the worker (privacy direction:
  worker doesn't see their own pings on a map in v1).
- Worker self-service erasure of their pings (admin-side via
  `add-location-tracking-dashboard` export then mongoexport delete).
- Heading / speed / altitude. Just lat/lng + accuracy.
- Per-Org consent text override. The dialog uses the platform-uniform
  privacy policy URL; Orgs that want to override link elsewhere can do
  so as a future change.

## Decisions

### Asymmetric start vs stop triggers

The tracker starts when `checkinStatusProvider` (server-confirmed
status) reports anything but `off_duty`. It stops when
`effectiveStatusProvider` (server overlay + non-failed queue rows)
reports `off_duty`.

Why asymmetric:

- **Start conservative**: a worker tapping `[上班]` puts a pending
  clock_in into the events queue. The optimistic effective status flips
  to `on_site` immediately. But if the server later rejects the
  clock_in (e.g. the worker was already on shift from a prior
  device-handover, surfaced as `INVALID_TRANSITION`), the effective
  status rolls back to `off_duty`. We don't want pings collected
  during that gap. Server confirmation is the safe gate.

- **Stop optimistic**: a worker tapping `[下班]` expects to no longer
  be tracked, immediately. Continuing to ping for the seconds it takes
  to reach the server would violate the principle of "user-visible
  state matches reality". The clock_out itself is rarely rejected
  (state machine allows clock_out from both on_site and in_transit;
  server-side `OUT_OF_ORDER` is the only realistic 4xx). On the rare
  rollback, the tracker re-starts (server status still `on_site`,
  effective rebounds, controller re-evaluates).

This asymmetry is implementable as two `ref.listen`s on the same
controller:

```
ref.listen(checkinStatusProvider, (prev, next) {
  if (next.value?.status != AppUserCheckinStatus.offDuty) {
    controller.maybeStart();  // idempotent
  }
});

ref.listen(effectiveStatusProvider, (prev, next) {
  if (next.status == AppUserCheckinStatus.offDuty) {
    controller.maybeStop();  // idempotent
  }
});
```

`maybeStart` early-returns if already running; `maybeStop` early-returns
if already stopped. No race because both run on the same Riverpod
container synchronously.

### Service / Controller split

`LocationTrackingService` wraps the platform plugin (`Geolocator`)
and exposes `start()`, `stop()`, `isActive` — pure I/O, no logic.

`LocationTrackingController` owns the start/stop decision matrix and
the secure-storage interactions for force-quit detection. Pure Dart
business logic, testable without platform mocks.

Tests substitute a fake `LocationTrackingService` to verify the
controller calls `start` / `stop` at the right moments; the real
service is exercised in live smoke only.

### `LocationPingProcessor` — batch model, not single-flight

The events queue uses single-in-flight strict serialization because
the server's `OUT_OF_ORDER` rule requires events arrive in
`occurred_at_client` order. Pings have no such ordering rule —
`location_pings` collection has no equivalent invariant. The batch
processor:

```
loop:
  if no rows in 'pending' status: return
  if connectivity offline: return
  if not (≥30 unsent OR ≥5 min since last flush OR shift-end): return
  pick up to 100 rows, mark 'sending'
  call POST /app/checkin/locations with batch
  on 201:
    delete rows in inserted_indices
    mark rows in rejected[] as 'failed' (or by simple delete — see below)
  on 5xx / network: backoff 1/2/4/8/16/30s, mark 'pending'
  on 403 LOCATION_TRACKING_DISABLED: mark all 'failed', stop tracker
  on 401: mark all 'failed', signal auth state to clear token
```

Triggers (when the processor's `tick` actually fires):

- Each `enqueueLocationPing` call (drift change stream listener,
  parallel to the events queue's wake-on-insert).
- Each `connectivity_plus` transition to online.
- A 1Hz foreground timer (cheap, just checks the threshold conditions).
- `clock_out` event finalization: explicit `processor.flushFinal()` call
  that bypasses the 30/5min thresholds. Drains whatever is left even if
  it's only 5 rows.

### `rejected[]` rows: mark `failed` vs delete

The events queue keeps `failed` rows visible (history with `[複製細節]`
+ `[關閉]`). For pings, this is overkill — there's no history view for
pings, the user doesn't need to see them, and a steady stream of
`INVALID_PING_TIMESTAMP` (e.g. clock skew) would clutter without
benefit. We **delete `rejected[]` rows on response receipt**, logging a
warning at the local logger level. If diagnosing a real issue requires
seeing rejected pings, the next change adds a debug toggle. v1 silently
drops them.

### 100m + 60s, AND not OR

OS-level `distanceFilter: 100` is the cheap path — Apple / Google's
fused providers handle this in C and don't even wake the app for
sub-100m moves. The 60s minimum interval is enforced in pure Dart in
the service: keep `_lastEnqueuedAt`; on each `Position` callback, if
`now - _lastEnqueuedAt < 60s`, drop. This is an AND filter (both
must pass), matching the explore decision.

For a worker driving fast (60s + 1km move), OS may emit ~10 callbacks
in that window; the throttle keeps only the first. Lost callbacks are
fine for visualization (line drawn directly between sampled points).

### Consent dialog gates `enqueueEvent`

The existing `enqueueEvent(eventType)` flow on `[上班]` tap is wrapped
with a consent check:

```
on tap [上班]:
  if eventType == clockIn AND org.locationTrackingEnabled:
    if !hasConsented(appUserId):
      show consent dialog
      if cancelled: return without enqueue
      mark consented (secure storage)
  proceed with enqueueEvent(clockIn)  // existing flow unchanged
```

`hasConsented` reads `argus.location_tracking.consent.<app_user_id>`
from secure storage. Per-AppUser key (vs global) handles the
device-handover case naturally — Bob logging in after Alice shouldn't
inherit her consent.

The dialog body explicitly mentions:
- Sampling cadence (each minute, only when moved 100m+)
- Retention (90 days)
- Who reads it (your Org admin)
- Link to platform privacy policy

`[取消]` aborts. `[同意並上班]` writes consent + proceeds.

### Force-quit recovery

A separate secure-storage key tracks "tracker was cleanly stopped":

```
key: argus.location_tracking.last_clean_stop
value: ISO8601 timestamp (or absent)

start tracker → clear key
graceful stop tracker (clock_out / app dispose) → set to now()
force-quit / OS kill → key is whatever it was last set to
```

On app boot, the home screen runs:

```
status = await checkinStatusProvider.future
if status != off_duty:
  lastClean = await secureStorage.read('argus.location_tracking.last_clean_stop')
  if lastClean is null OR older than the latest pending_location_pings.enqueued_at:
    show banner: "定位追蹤上次中斷過，已恢復記錄"
    (banner auto-dismisses after 10s; user can also dismiss)
  controller.maybeStart()
```

The "older than the latest pending row" check covers a corner case
where the tracker was cleanly stopped earlier but a force-quit
happened mid-shift after a re-clock-in. We trust the row enqueue time
because it's set by the tracker on enqueue, not by the user.

### Privacy URL config

Compile-time constant in `core/env/env.dart`:

```
class Env {
  static const String _privacyUrlDartDefine =
    String.fromEnvironment('PRIVACY_URL');

  static String privacyUrl() {
    if (_privacyUrlDartDefine.isNotEmpty) return _privacyUrlDartDefine;
    return 'http://localhost:3000/privacy';  // dev default
  }
}
```

Dev menu adds a row to override (parallel to the existing
`api_base_url` override; the secure storage key is
`dev.privacy_url_override`). Production builds get the real URL via
`--dart-define=PRIVACY_URL=https://argus.example.com/privacy` in the
release build script.

### Home tracking chip — separate from queue chip

The existing `QueueChip` reflects the events queue (pending /
sending / failed counts for events). A second chip for the location
tracker has different semantics:

- "Active" status (boolean, no count)
- Elapsed time since `controller.startedAt`
- Visible whenever tracker is running, regardless of queue state

Two chips can coexist on the home screen layout. Visual distinction:
queue chip is a count-shape "待送出 N 筆"; tracker chip is a status-
shape "📍 定位追蹤中 · 02:14".

### Privacy link tap launches in-app browser, not external

`url_launcher` package — but on iOS we want
`LaunchMode.inAppWebView` (or `inAppBrowserView`) so the worker
doesn't get bounced out of Argus into Safari mid-clock-in. After
reading they tap close → return to consent dialog. Same on Android
via Custom Tabs.

The `url_launcher` package is small (~80KB) and already widely used
in the Flutter ecosystem.

## Risks / Trade-offs

- **iOS Background Location revoked unilaterally**: User can flip
  Argus's location permission from `Always` / `WhenInUse` to `Never`
  in Settings any time. The tracker's `Stream<Position>` will stop
  emitting. We don't detect this gracefully in v1 — the worker just
  has a gap in their trajectory. Mitigation: the iOS blue bar gives
  visibility; if it disappears the worker can guess permission was
  revoked. v2 adds a `Geolocator.checkPermission()` poll on app
  resume that surfaces a banner if permission degraded.

- **Android battery optimization**: Some OEMs (Xiaomi, OPPO) ship
  aggressive doze policies that kill foreground services even with
  the sticky notification. Mitigation: README documents the issue
  and tells admins to whitelist Argus in battery settings. Worker
  experience: gap in trajectory, indistinguishable from poor signal.

- **Memory usage of in-process queue**: a worker with no signal for
  8 hours generates ~150 pings (after 100m filter, given typical
  movement). 150 rows in drift is trivial; even pathological 8-hour
  stationary periods are ~0 rows. Worst case (8-hour drive on bumpy
  GPS) is ~480 rows. SQLite handles this without thinking.

- **Force-quit recovery banner false positive**: if app was killed
  by OS for unrelated reasons (low memory) at literally the same
  moment as a graceful stop, the banner might mis-show. Acceptable —
  banner is informational, not blocking.

- **Consent dialog latency**: dialog renders + secure storage write
  takes ~100ms. Worker tapping `[上班]` sees 100ms delay before
  dialog. Below perception threshold for the first time only;
  subsequent shifts skip the dialog so latency is moot.

- **OS-level pausesLocationUpdatesAutomatically**: setting it to
  `false` is non-default and may surprise iOS reviewer. We document
  the choice in the App Store metadata when we eventually submit. The
  intent matches established time-tracking apps (Strava, Toggl).

- **Race: server-confirmed clock_in happens, controller starts
  tracker, FIRST ping arrives after `[轉出]` tap**: the first ping's
  `occurred_at_client` is right after the clock_in's, so it reads as
  on-shift activity even though effective status briefly transitioned
  to in-transit. Visualization-wise this is fine (the ping is on the
  trajectory between sites). Server doesn't care about ping → event
  alignment.

- **Consent flag survives app reinstall? Maybe not.** Secure storage
  on iOS uses Keychain (survives reinstall), on Android uses
  EncryptedSharedPreferences (lost on reinstall). The asymmetry is
  unfortunate; on Android a reinstall re-prompts consent. Since
  reinstall is rare, accept the asymmetry. Explicitly noted in the
  consent flow's tests.

## Migration Plan

No data migration. New drift table created on next app launch (drift
auto-migrates schema version 1 → 2). New native config requires:

- iOS: edit `Info.plist`, run `pod install`.
- Android: edit `AndroidManifest.xml`. (Geolocator's plugin already
  declares `FOREGROUND_SERVICE` permission internally, but
  `FOREGROUND_SERVICE_LOCATION` is required separately on
  Android 14+ and we add it explicitly.)

For end-users:

- Existing AppUsers see no change until their Org enables the toggle.
- After Org enables: next `[上班]` tap surfaces consent dialog. Once
  consented, future shifts skip the dialog.
- Battery + privacy expectation set by the iOS blue bar / Android
  sticky notification — both OS-mandated, can't be opted out of.

For developers:

1. `flutter pub get` (no new packages).
2. `dart run build_runner build --delete-conflicting-outputs` (drift
   schema).
3. iOS: `cd ios && pod install`.
4. Android: Gradle resolves on next `flutter run`.

No rollback plan needed pre-launch.

## Open Questions

- **Should the tracker keep running through a force-change-password
  flow?** Worker is technically "on shift" (clock_in succeeded),
  but the app navigates them to `/force-change-password`. The home
  screen is unmounted; tracker controller's lifecycle is tied to the
  Riverpod container, not the home widget — so it keeps running.
  Probably fine; revisit if real user feedback says it's confusing.

- **Should we throttle the chip's elapsed-time render?** Currently
  reads `now() - startedAt` on each rebuild. If running > 8 hours
  the rebuild frequency is fine. If we render every second via a
  Timer.periodic the chip is always live. Probably yes; will add as
  a micro-task in the chip widget itself.

- **Consent text translation**: zh-TW only for v1. If a non-zh-TW
  worker reads the dialog they see Chinese. Acceptable per the
  v1 i18n scope (see app/README.md "Provisional bundle id" note).

- **Should the tracker self-stop when the app loses location
  permission mid-shift?** Geolocator's stream emits an error, we
  catch and log, the stream dies, but the controller doesn't know
  to mark the tracker as "stopped due to permission". The chip
  would still say "tracking" but no pings would land. v2 listens for
  errors and updates the chip to "📍 追蹤已暫停 · 需要定位權限".
  v1 keeps it simple.
