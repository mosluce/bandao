## Context

The Flutter app shipped by `add-app-shell` runs end-to-end against the auth surface but stops short of doing anything with the user it identifies. The placeholder `尚未實作` pill on home was always going to be replaced by this change. Meanwhile `add-checkin-events` shipped the full server checkin surface — four event types, three-state machine, dual timestamps, transfer toggle with state-lock, force-checkout, reverse-geocoded `region_name`, dual-timestamp skew warnings — but nothing has consumed it from a mobile client. This change is the consumer.

The product brief locked into explore is short: tap a button → location captured → event appears in history → eventually shows up on the admin-web live board. The non-trivial parts are everything between "tap" and "appears on admin board":

- The phone is offline a lot. We can't drop or reorder events when network comes back.
- The user expects the UI to react instantly. Pessimistic UI on a 1-second network round-trip is fine; on a 3-hour offline stretch it would be a bug.
- The OS will move the app to background. We should keep submitting when allowed.
- A device might be handed between employees. Past events from the prior account must not silently submit under a new login.

The decisions below all flow from those constraints.

## Goals / Non-Goals

**Goals:**

- Make the home screen a working clock-in/clock-out tool against the existing `/app/checkin/*` surface, with a UX that survives offline → online cycles cleanly.
- Establish the device-local queue + processor pattern so `add-location-tracking` can reuse the plumbing for streaming location pings.
- Surface the unified history (server + local queue) in one screen so users can see exactly what their phone has and hasn't sent.
- Give failed events a non-disruptive resting state with a "回報" path (copy detail) so support can act without having to reproduce.
- Get background sync working as far as the OS lets us — explicitly documenting iOS limitations rather than pretending they aren't there.

**Non-Goals:**

- Continuous location tracking between events. The "where was Alice during transit?" story is `add-location-tracking`'s job.
- Push notifications, deep links, biometric login, multi-language beyond zh-TW.
- A reactive "current shift" timer that ticks every second visually (the displayed elapsed time is computed on rebuild only — close enough for v1).
- Edit / delete past events. Server is append-only; the only post-hoc remediation is admin force-checkout, already on `/checkin/*`.
- Manual location entry mid-shift (override `region_name` or coordinates). Not in spec; deferred.
- Web build of the Flutter app. Still iOS + Android only.

## Decisions

### Optimistic UI with `failed`-only rollback

The `effective` status the home screen renders is the result of replaying queued events on top of the last server-confirmed `CheckinUserStatusDto.status`. `pending` and `sending` rows contribute (they represent intent the server hasn't acknowledged yet but the user expects to be live). `failed` rows are EXCLUDED — if the server rejected `clock_in` because we were already on shift, the optimistic transition is rolled back so the home buttons match reality.

State reducer:

```
effectiveStatus(serverStatus, queue):
  status = serverStatus
  for event in queue ordered by occurred_at_client asc:
    if event.status == failed: continue   # skip rejected
    status = applyStateMachine(status, event.event_type)
    # If applyStateMachine rejects, status stays put — but a row that
    # the server would reject is also rejected by the local replay so
    # the queue shouldn't contain it for long.
  return status
```

The reducer is pure, easy to unit-test, and produces the shift-elapsed counter as a side product (find the most recent `clock_in` not preceded by a `clock_out` later in time and use its `occurred_at_client`).

Why optimistic: a worker on a remote site needs `[上班]` to feel like it worked even with no signal. Pessimistic UI would freeze on the spinner for the entire offline stretch — confusing and indistinguishable from a broken app.

Why failed-only rollback: hiding `pending` from the optimistic status would defeat the point. Hiding `failed` is correct because the server has spoken: that event isn't going to happen.

### Persistent queue in drift, single in-flight, strict serialization

A single SQLite table with `(status, occurred_at_client)` index. The processor does:

```
loop:
  if there is already a row in `sending`: return  // single in-flight
  pick oldest row where status = pending order by occurred_at_client asc
  if none: return
  mark row sending, attempts++
  POST /app/checkin/events
  case 201: delete row; loop
  case 4xx state machine: mark failed; loop
  case 401: mark failed; raise auth-fail signal; return
  case 5xx / network: mark pending; back off; return
```

The "strict serialization" rule lives in the spec we already shipped: server rejects `OUT_OF_ORDER`. Client honors this by never having more than one row in flight, and only advancing on `2xx`. This means a single bad row can stall the queue — but a 5xx/network failure puts the row back to `pending` and backs off, while a 4xx state-machine rejection marks it `failed`, and `failed` rows are skipped when picking the next pending row.

We picked drift over `sqflite` for typed migrations and query DSL. Drift requires `dart run build_runner build` — we paid that cost back already in this change since it brings in the codegen tooling we deferred in `add-app-shell`.

### Backoff + connectivity awareness

Exponential `1, 2, 4, 8, 16, 30s capped` after each retryable failure (5xx / network). No attempt cap — when connectivity returns, the queue resumes. We DON'T increment attempts while `connectivity_plus` reports offline; that prevents the backoff window from growing during a long offline stretch and snapping to 30s the moment connectivity returns. When online, normal exp backoff applies.

The processor wakes on three triggers:

1. Each `enqueue` call.
2. Each `connectivity_plus` change to `online`.
3. A periodic `Timer.periodic(Duration(seconds: 1))` while the app is foreground (cheap, just checks queue head).
4. Background callbacks from `workmanager`.

### Background sync via `workmanager`

`workmanager` package wraps Android `WorkManager` and iOS `BGTaskScheduler`. We use it as follows:

- **Android**: register a `OneTimeWorkRequest` whenever a row is enqueued. WorkManager schedules it under the OS's job constraints; we set `Constraints(networkType: connected)` so it doesn't even try while offline. Multiple enqueues coalesce — WorkManager dedupes by unique-name.
- **iOS**: register a `BGProcessingTask` once on app start with a unique identifier (`tw.ccmos.app.argus.queue-drain`). When the app moves to background and the OS schedules our task, the callback drains the queue. There is NO real-time guarantee — iOS may delay it for hours, especially on locked devices with low battery. Documented in README + onboarding tip.

The background callback runs the same processor logic as the foreground. A short timeout budget (~25s on iOS) keeps the OS happy.

We explicitly DO NOT try to keep the app alive in background via the `audio` or `location` background modes. Those are reserved for actual continuous use cases (`add-location-tracking` will need the `location` mode); abusing them now risks App Store rejection.

### Login-handover queue wipe

Each enqueued row stores `app_user_id` (snapshot of `AppUser.id` at enqueue time). On every successful login (manual or auto), the auth notifier reads the resolved `user.id` and triggers a wipe pass:

```
delete from pending_events where app_user_id != current_user_id;
if any rows were deleted: surface toast "前個帳號的 N 筆未送事件已清除"
```

This handles two real cases:

1. Phone shared between coworkers; Alice logs out and Bob logs in. Bob shouldn't have Alice's clock-in events submit under his name.
2. Token expired and Bob re-logs in as a different person — same outcome.

The wipe runs as part of the auth state machine's `authenticated` transition, before the home screen renders. We do NOT prompt the user to confirm — the loss of the prior account's queue is a pure security/correctness move, not a soft-delete.

For the same-user re-login case, app_user_id matches and nothing is wiped.

### Direct submit on tap (no bottom sheet)

Tapping `[上班]` does:

1. Capture GPS (`LocationAccuracy.high`, 10s timeout, fallback `getLastKnownPosition()`, fallback `null` accuracy).
2. Build the `pending_events` row with current AppUser id, current state-derived event type, captured coordinates, `occurred_at_client = DateTime.now().toIso8601String()`, status `pending`.
3. Insert. Wake the processor.

No confirmation dialog, no override-location field, no preview. Tapping IS the commit.

We considered a confirm bottom-sheet with location preview. Rejected: most workers will check in five times a day, the friction adds up, and the optimistic-with-rollback model means an accidental tap shows up as a `failed` row in history (that they can dismiss / report) — far less disruptive than a popup blocking every event.

The location-preview-as-region-name path was also considered. Rejected: the server already does the reverse geocoding and returns `region_name` in the event response (and on later GET). Doing it on the client would (a) require shipping a geocoder dep, (b) potentially disagree with the server's render, (c) burn rate against whatever provider the client used. Until the server has confirmed the row, the history shows location as `lat, lng`. Once the server replies with `region_name`, history switches to the human-readable label.

### Failed-only dismiss + plaintext detail copy

Per the explore session, only `failed` rows get a `[關閉]` action that deletes the row. `pending` and `sending` rows are not user-cancellable — once the user committed to an event by tapping the button, it has to either succeed or be rejected by the server. The design cuts off the foot-gun where a user taps `[上班]`, sees no instant network response, and starts cancelling-and-retrying creating a duplicate-events mess.

Failed rows also get `[複製細節]`, which puts a single plaintext blob on the clipboard:

```
Argus checkin event report
event_id: queue#1234
event_type: clock_in
occurred_at_client: 2026-05-04T18:00:12+08:00
lat, lng: 25.04792, 121.56401 (±15m)
attempts: 4
last_error_code: INVALID_TRANSITION
last_error_message: cannot clock_in from on_site
last_attempt_at: 2026-05-04T18:00:13+08:00
app_user_id: 6841c7f9c10e8a1b8d2f3450
```

The user can paste it into Slack / email / a screenshot annotation when reporting. A future change can pipe this into a structured `Submit feedback` flow; v1 keeps it as plaintext that's easy to trust.

### Permission UX

`geolocator.requestPermission()` and `geolocator.checkPermission()` cover the matrix. The home buttons are disabled (with a banner above them) when permission is `denied` or `deniedForever`. The banner contains an `[開啟設定]` button that uses `app_settings` to deep-link into the OS settings page for this app.

We don't pre-prompt for permission on app start — the prompt only appears the first time the user taps an event button. Two reasons: (1) a fresh install pre-prompt feels intrusive when the user hasn't even seen the home; (2) iOS permission grants tied to a clear in-app trigger get higher consent rates (well-known UX pattern).

For `denied` (iOS: user can re-prompt; Android: re-prompt with rationale on most flows), tap on the disabled buttons re-runs `requestPermission()`. For `deniedForever`, only the settings deep-link works.

### Unified history view

`/history` route renders a single `ListView`:

```
[ chip: 全部 / 待送出 / 失敗 ]   <- filter

  ⏳ 上班         剛剛
     pending · 25.0479, 121.5640

  ✗ 轉出         5 分前
     failed · INVALID_TRANSITION 已在現場
     [複製細節] [關閉]

  ✓ 下班 @ 信義區     09:30
  ✓ 上班 @ 中山區     08:00
  ✓ 下班 @ 大安區     昨天 18:00

  [載入更多 server 歷史]
```

The list is the merge of:

1. All `pending_events` rows for the current user (regardless of status) — these are local.
2. The cursor-paginated server result from `GET /app/checkin/events?limit=50&before=<oldest_loaded>`.

Sort order: `occurred_at_client` desc. When local + server share an `occurred_at_client` (extremely unlikely but possible if the server's clock and the client's match to the millisecond), break ties by `(server-event > local-pending > local-sending > local-failed)`. The UI displays the server row, the local row drops out — that's actually expected: the queue's `done` rows have already been deleted, so a local row at the same client time as a server row means the queue tracked a row through to `2xx` and is about to be deleted.

The `[載入更多]` button only fetches more server pages; local rows are always all-shown.

### DTO surface (locked)

```
enum CheckinEventType { clockIn, clockOut, transferOut, transferIn }
  with snake_case JSON: clock_in / clock_out / transfer_out / transfer_in.

enum AppUserCheckinStatus { offDuty, onSite, inTransit }
  snake_case JSON: off_duty / on_site / in_transit.

enum EventSource { app, adminForce }
  snake_case JSON: app / admin_force.

enum EventInitiatorKind { appUser, dashboardUser }
  snake_case JSON: app_user / dashboard_user.

class GeoPoint { lat, lng }

class EventLocation {
  GeoPoint coordinates
  double? accuracyMeters
  String? regionName
  String? manualLabel
}

class CheckinEventDto {
  String id
  String appUserId
  CheckinEventType eventType
  String occurredAtClient   // RFC3339 with offset
  String occurredAtServer   // RFC3339 UTC
  EventSource source
  EventInitiatorKind initiatedByKind
  String initiatedById
  EventLocation location
  String? reason
  bool hasSkewWarning
}

class CheckinUserStatusDto {
  String appUserId
  AppUserCheckinStatus status
  String? currentShiftStartedAt
  CheckinEventDto? lastEvent
  bool hasSkewWarning
}

class SubmitCheckinEventRequest {
  CheckinEventType eventType
  double lat
  double lng
  double? accuracy
  String? manualLabel
  String occurredAtClient
}
```

These mirror the Rust DTOs in `api/src/handlers/checkin_dto.rs`. As with the auth DTOs in `add-app-shell`, we hand-roll value classes for now; OpenAPI codegen replaces them in a later ROADMAP item.

## Risks / Trade-offs

- **Drift codegen reintroduces `build_runner`** — `add-app-shell` deferred this. The risk noted there (sandbox can't run codegen) is now in scope. Mitigation: only one schema (`pending_events`) plus a few drift queries; if codegen is genuinely blocked we fall back to `sqflite` + raw SQL with a small typed wrapper. Documented in tasks.md as a recovery path.
- **iOS background scheduling is unreliable** — workers may report "I clocked out before leaving the site but it didn't show up on the admin board until I opened the app on the bus home." This is OS-imposed, not a bug. We mitigate with documentation, an onboarding tip, and the visible queue chip on home so users can see what's still local.
- **Optimistic rollback can confuse users** — if a `clock_in` is rejected because the user was already on shift, the home pill briefly says `on_site (你剛按了上班)` then snaps back to whatever it was. Mitigation: the failed row in history says exactly why it was rejected; the pill never shows fake permanent state.
- **Queue handover on user switch** — wiping a previous user's queue means losing real events the prior user had captured but not synced. This is the right trade for our threat model (we don't want events submitted under the wrong identity), but it's a real data loss path. Documented in `app/README.md` as "log out only when you have signal and the queue chip says 0".
- **Single in-flight stalls on a bad row** — if the head of the queue is `pending` and keeps hitting `5xx`, no later events submit. Mitigation: this matches the server's `OUT_OF_ORDER` rule (we couldn't submit later events out of order anyway), and exp-backoff prevents thrashing. The user can see which row is stuck via the chip + history view.
- **Permission denied UX** on iOS: `deniedForever` is a one-way street (user must go to Settings). Mitigation: clear `[開啟設定]` button that deep-links via `app_settings`. We don't try to keep prompting because iOS won't show the system prompt again anyway.
- **GPS cold-start slowness** on simulators (and sometimes real devices) — `LocationAccuracy.high` can take 5+ seconds on first request after fresh boot. Mitigation: 10s timeout, `getLastKnownPosition()` fallback, `null`-coordinate event rejected at submission time (we won't enqueue without a valid lat/lng).
- **`occurred_at_client` carries device-local timezone offset** (RFC3339 with `+08:00` style suffix) — server has been compatible with this since `add-checkin-events` (it parses to absolute UTC server-side). Tests confirm.
- **Drift + workmanager cold start cost** — on background wake, the database has to open, current AppUser id has to come from secure storage, and the dio client has to bootstrap with a refreshed token. We accept the few-hundred-millisecond latency on each background callback; iOS allows ~25s of work per `BGProcessingTask`, so we have plenty of margin.
- **App Store review risk** — adding `BGProcessingTask` and `UIBackgroundModes: processing` is reviewed but not unusual for productivity / time-tracking apps. We'll include the rationale in the App Store metadata when we eventually submit (out of scope here).

## Migration Plan

There is no data migration. The new SQLite database (drift) is created on first launch after this change ships; nothing pre-existed.

For developers:

1. `flutter pub get` resolves new packages (`drift`, `drift_dev`, `geolocator`, `app_settings`, `workmanager`, `connectivity_plus`, `sqlite3_flutter_libs`).
2. `dart run build_runner build --delete-conflicting-outputs` generates the drift schema bindings.
3. iOS: a one-shot `pod install` from `app/ios/` after `flutter pub get`. Xcode project picks up native sources automatically.
4. Android: Gradle resolves on next `flutter run`.

For end-users on devices already running the `add-app-shell` build:

- The app will request location permission the first time they tap an event button.
- iOS users will see a one-shot onboarding tip on home explaining the background-sync caveat (dismissible, never re-shown).

No rollback plan needed — pre-launch, no production data.

## Open Questions

None blocking implementation. Deferred but worth noting:

- Whether to surface a per-event `app_user_id` mismatch warning on the rare case where a queued event's `app_user_id` matches the current user but the server rejects with `NOT_A_MEMBER` (couldn't happen given the AppUser ↔ Org 1:1 model, but defensive). Today: that would just be a `failed` row with the server's error message.
- Whether to add a "force resync" button somewhere that re-fetches `/app/checkin/status` and `/app/checkin/events`. Probably yes once we observe drift in practice — but we don't expect drift since the queue is the source of truth.
- Whether to pre-warm the GPS chip on app launch (request a location update silently) so the first event tap is faster. Adds battery cost; revisit if real users complain about the 1–3s first-tap latency.
- Whether to provide a "queue is empty / fully synced" affirmative indicator (like Slack's "all caught up"). Currently the chip just hides — we may want explicit positive feedback for users who like to confirm everything is sent before logging out.
