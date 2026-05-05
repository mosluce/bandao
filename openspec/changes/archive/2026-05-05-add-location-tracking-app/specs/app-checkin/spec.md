## ADDED Requirements

### Requirement: Consent dialog gates the first clock_in when location tracking is enabled

The system SHALL, when an AppUser taps `[上班]` and `Org.checkin
.location_tracking_enabled == true` and the secure-storage flag
`argus.location_tracking.consent.<app_user_id>` is absent, surface a
modal dialog before invoking the existing `enqueueEvent(clockIn)`
flow. The dialog SHALL include text describing the sampling cadence
("約每分鐘記錄一次，移動超過 100 公尺才記錄"), the 90-day retention,
and a tappable link opening `<admin-web>/privacy` in an in-app
browser. The dialog SHALL offer a `[取消]` action and a `[同意並上班]`
action. `[取消]` SHALL abort — no event is enqueued. `[同意並上班]`
SHALL write the consent flag to secure storage and proceed with
`enqueueEvent(clockIn)` as before. Once the flag is set, subsequent
`[上班]` taps SHALL skip the dialog. An Org toggle change from `false`
back to `true` SHALL NOT re-prompt (the underlying state-lock
guarantees workers are off-duty during such flips).

#### Scenario: First-time clock_in with toggle enabled prompts consent

- **GIVEN** `Org.checkin.location_tracking_enabled == true`
- **AND** the secure-storage flag for the AppUser is absent
- **WHEN** the AppUser taps `[上班]`
- **THEN** the consent dialog appears
- **AND** no `pending_events` row is enqueued yet

#### Scenario: Cancel aborts the shift start

- **WHEN** the consent dialog is visible and the user taps `[取消]`
- **THEN** the dialog dismisses
- **AND** no `pending_events` row is enqueued
- **AND** the AppUser's `effective_status` remains `off_duty`

#### Scenario: Confirm proceeds and persists consent

- **WHEN** the consent dialog is visible and the user taps `[同意並上班]`
- **THEN** secure storage gains an entry at
  `argus.location_tracking.consent.<app_user_id>`
- **AND** the existing `enqueueEvent(clockIn)` flow runs, inserting a
  `pending_events` row

#### Scenario: Subsequent clock_in skips the dialog

- **GIVEN** the AppUser has previously consented (the secure-storage
  flag exists)
- **WHEN** the AppUser taps `[上班]`
- **THEN** the consent dialog does NOT appear
- **AND** the `enqueueEvent(clockIn)` flow runs immediately

#### Scenario: Toggle disabled means no consent prompt

- **GIVEN** `Org.checkin.location_tracking_enabled == false`
- **WHEN** the AppUser taps `[上班]`
- **THEN** the consent dialog does NOT appear
- **AND** the `enqueueEvent(clockIn)` flow runs as it did before this change

#### Scenario: Different AppUser on the same device sees own dialog state

- **GIVEN** AppUser A has consented on this device (flag exists for A)
- **AND** AppUser B logs in afterwards (handover wipe runs)
- **WHEN** AppUser B taps `[上班]` for the first time
- **THEN** the consent dialog appears (the flag key is per-AppUser-id)

### Requirement: Location tracker starts on server-confirmed non-off_duty status

The system SHALL run a background `LocationTrackingService` whose
active state is governed by two providers:

- **Start trigger**: the tracker SHALL transition from inactive to
  active when `checkinStatusProvider.value.status` becomes any value
  other than `off_duty`. This includes:
  - server-confirmed clock_in: `pending_events` clock_in row syncs,
    response updates server status to `on_site`.
  - cold-start path: app boots, `checkinStatusProvider.future` resolves
    to `on_site` or `in_transit` (existing shift carried across app
    restart).
- **Stop trigger**: the tracker SHALL transition from active to
  inactive when `effectiveStatusProvider.value.status == off_duty`.
  Effective status reflects optimistic state — the tap on `[下班]`
  flips it before server confirms.

The tracker SHALL NOT cycle on `transfer_out` / `transfer_in`
transitions (effective status stays non-`off_duty` across those).

The tracker's start/stop SHALL be idempotent — calling start when
already active or stop when already inactive SHALL be no-ops.

#### Scenario: Optimistic clock_in does NOT start the tracker

- **GIVEN** the AppUser is `off_duty` and has consented
- **WHEN** the AppUser taps `[上班]` and the clock_in event enters the
  pending state
- **THEN** the tracker remains inactive
- **AND** no rows are inserted into `pending_location_pings` while the
  clock_in event is `pending` or `sending`

#### Scenario: Server-confirmed clock_in starts the tracker

- **GIVEN** a `pending_events` clock_in row is in flight
- **WHEN** the row's POST returns `201` and the queue processor
  invokes `onStatusFresh` with the new server status `on_site`
- **THEN** `LocationTrackingService.start()` is invoked

#### Scenario: Cold-start with existing on_site status starts the tracker

- **WHEN** the app cold-starts and `checkinStatusProvider.future`
  resolves to `on_site` (or `in_transit`)
- **THEN** the tracker starts on the next frame
- **AND** the force-quit recovery banner appears if the
  `last_clean_stop` flag is missing or stale

#### Scenario: Tap on clock_out stops the tracker immediately

- **GIVEN** the tracker is active
- **WHEN** the AppUser taps `[下班]` and the optimistic effective
  status flips to `off_duty`
- **THEN** the tracker's stop is invoked before the clock_out event
  has been confirmed by the server

#### Scenario: Clock_out failure rebounds and re-starts the tracker

- **GIVEN** the tracker is active
- **WHEN** the AppUser taps `[下班]`
- **AND** the clock_out POST returns a 4xx that triggers effective
  status rollback to `on_site`
- **THEN** the tracker re-activates (server status remained `on_site`)

#### Scenario: Transfer events do not cycle the tracker

- **GIVEN** the tracker is active
- **WHEN** a `transfer_out` event is enqueued and confirmed
- **AND** later a `transfer_in` event is enqueued and confirmed
- **THEN** the tracker remains active across both transitions
- **AND** at no point during these events does
  `LocationTrackingService.stop()` get invoked

### Requirement: Sampling honors the 60s + 100m AND filter

The `LocationTrackingService` SHALL configure platform location
streams with `LocationAccuracy.high` and `distanceFilter: 100`
(meters). On iOS the service SHALL set
`pausesLocationUpdatesAutomatically: false` and
`showBackgroundLocationIndicator: true`. On Android the service SHALL
configure a foreground notification (sticky) via geolocator's
`AndroidSettings.foregroundNotificationConfig`. On top of the
OS-level distance filter the service SHALL impose an in-process
60-second minimum interval — when a `Position` callback fires within
60 seconds of the last enqueued ping, the callback SHALL be discarded
without enqueue. The combined effect is the AND condition: a ping is
enqueued only when both `≥ 100m moved since last enqueue` (OS) AND
`≥ 60s elapsed since last enqueue` (client) are satisfied.

#### Scenario: Two callbacks within 60s — only first enqueues

- **GIVEN** the tracker has just enqueued a ping at time `T`
- **WHEN** the OS emits a second `Position` callback at time `T + 30s`
  (worker drove fast, crossed the 100m threshold twice)
- **THEN** the second callback is discarded
- **AND** no row is added to `pending_location_pings`

#### Scenario: Stationary worker emits no pings

- **GIVEN** the tracker is active and the worker is sitting still
- **WHEN** 8 hours pass with no movement greater than 100m
- **THEN** zero rows are added to `pending_location_pings`
- **AND** the OS does NOT emit `Position` callbacks (distance filter
  is enforced at the platform layer)

#### Scenario: Worker walks 1km in 10 minutes — 10 pings

- **GIVEN** the tracker is active
- **WHEN** the worker walks 1km in 10 minutes (≈ 100m/min)
- **THEN** approximately 10 pings are enqueued (one per ~minute,
  assuming OS callbacks arrive cleanly at each 100m crossing)

### Requirement: Pings are batch-uploaded with ≥30 / ≥5min / shift-end triggers

The `LocationPingProcessor` SHALL pick rows from the
`pending_location_pings` table and submit them via
`POST /app/checkin/locations` in batches of up to 100 pings per
request. The processor SHALL fire when ANY of the following triggers
are met:

- `pending` row count is ≥ 30, AND device is online.
- ≥ 5 minutes have elapsed since the last successful flush, AND any
  `pending` row exists, AND device is online.
- `connectivity_plus` transitions from offline to online AND any
  `pending` row exists.
- A `clock_out` event finalization explicitly invokes
  `processor.flushFinal()`, which bypasses the threshold conditions
  and drains all `pending` rows in successive batches.

On a successful response, the processor SHALL delete rows
corresponding to indices in `accepted_count` (computed by exclusion
from `rejected[]`'s indices). Rows in `rejected[]` SHALL be deleted
silently (with a warning logged), since v1 does not surface ping-level
errors to the user. On `5xx` / network errors the processor SHALL
return rows to `pending` with attempts incremented, and back off using
the same `1, 2, 4, 8, 16, 30s` schedule as the events queue. On `403
LOCATION_TRACKING_DISABLED` the processor SHALL delete all in-flight
rows (the Org has clearly disabled tracking; pings would never
succeed) and signal the `LocationTrackingController` to stop the
tracker.

#### Scenario: 30-row threshold triggers flush

- **GIVEN** the tracker is active and the device is online
- **WHEN** the 30th row is enqueued into `pending_location_pings`
- **THEN** `processor.tick()` fires within 100ms
- **AND** `POST /app/checkin/locations` is called with up to 100 rows

#### Scenario: 5-minute timer triggers flush even with sparse pings

- **GIVEN** the tracker is active, device is online, and 5 rows have
  been enqueued over the last 6 minutes
- **WHEN** the 5-minute interval elapses since the last flush
- **THEN** `processor.tick()` fires
- **AND** the 5 rows are sent in one batch

#### Scenario: Connectivity restoration drains backlog

- **GIVEN** the tracker has been active for 2 hours offline, with 50
  rows in `pending_location_pings`
- **WHEN** `connectivity_plus` transitions to online
- **THEN** `processor.tick()` fires immediately
- **AND** rows are sent in batches of up to 100 until the queue is empty

#### Scenario: clock_out triggers final flush

- **GIVEN** the tracker is active and 12 rows are pending
- **WHEN** the AppUser taps `[下班]` and the clock_out event finalizes
- **THEN** `processor.flushFinal()` runs
- **AND** all 12 rows are sent regardless of the 30-row threshold

#### Scenario: rejected[] rows are deleted silently

- **GIVEN** a batch of 10 pings is sent
- **WHEN** the server responds with `accepted_count = 9, rejected: [{
  index: 4, code: "INVALID_PING_TIMESTAMP" }]`
- **THEN** all 10 corresponding rows are removed from
  `pending_location_pings`
- **AND** a warning is logged for the rejected entry
- **AND** the user sees no UI surface for the rejection

#### Scenario: 403 LOCATION_TRACKING_DISABLED stops the tracker

- **GIVEN** an admin flips the Org toggle to `false` while the
  tracker is active (only possible in a corner case where a worker
  re-clocked in despite the state-lock; not normally possible)
- **WHEN** the next batch is sent and the server responds `403
  LOCATION_TRACKING_DISABLED`
- **THEN** all in-flight rows are deleted
- **AND** `LocationTrackingService.stop()` is invoked
- **AND** the tracker chip on home is hidden

### Requirement: Force-quit recovery surfaces a one-shot banner

The system SHALL set a secure-storage key
`argus.location_tracking.last_clean_stop` to the current ISO8601
timestamp whenever the tracker stops cleanly (clock_out flow, app
disposal). The system SHALL clear this key whenever the tracker
starts. On every app cold-start, the system SHALL evaluate whether a
force-quit happened during a previous shift: if `checkinStatusProvider`
reports `on_site` or `in_transit` AND the
`last_clean_stop` key is absent OR older than the most recent
`enqueued_at` of any row in `pending_location_pings`, the system
SHALL display a one-shot banner on the home screen with the message
`定位追蹤上次中斷過，已恢復記錄`. The banner SHALL be dismissible by the
user and SHALL auto-dismiss after a brief duration (around 10
seconds). The system SHALL automatically invoke
`LocationTrackingController.maybeStart()` after rendering the banner.

#### Scenario: Clean shutdown writes the flag

- **GIVEN** the tracker is active
- **WHEN** the AppUser taps `[下班]` and the clock_out flow runs
- **THEN** secure storage's `argus.location_tracking.last_clean_stop`
  is set to the current ISO8601 timestamp
- **AND** the tracker is stopped

#### Scenario: Force-quit during shift surfaces the banner on next launch

- **GIVEN** the tracker was active and the user force-quit Argus from
  the multitasking switcher (no clean shutdown, no flag write)
- **WHEN** the user re-opens Argus and the home screen finishes its
  initial frame
- **THEN** the recovery banner is visible
- **AND** the tracker has been started

#### Scenario: Banner does not appear when status is off_duty

- **GIVEN** the AppUser is `off_duty` (no shift in progress)
- **WHEN** the user opens Argus
- **THEN** no recovery banner appears (no shift to recover)

#### Scenario: Banner does not appear after a clean stop

- **GIVEN** the tracker stopped cleanly during the previous app
  session (flag is fresh)
- **WHEN** the user re-opens Argus
- **THEN** no recovery banner appears

### Requirement: Home shows a tracking chip while the tracker is active

The system SHALL render a chip on the home screen whenever the
`LocationTrackingController.isRunning == true`. The chip's content
SHALL include a location-pin icon (`📍` or equivalent), the static
label `定位追蹤中`, and an elapsed-time indicator computed from
`controller.startedAt` (e.g. `02:14`). The chip SHALL update its
elapsed-time display at least once per second while visible. When
the tracker is inactive, the chip SHALL NOT be rendered. The chip
SHALL be visually distinct from the existing queue chip
(`待送出 N 筆 / 1 筆失敗`); both can coexist on the home layout
without overlap.

#### Scenario: Chip visible during shift

- **GIVEN** the tracker is active for 2 minutes 14 seconds
- **WHEN** the home screen renders
- **THEN** the chip displays `📍 定位追蹤中 · 02:14` (or equivalent)

#### Scenario: Chip hidden when off_duty

- **GIVEN** the AppUser is `off_duty`
- **WHEN** the home screen renders
- **THEN** no tracking chip is visible

#### Scenario: Chip elapsed time updates

- **GIVEN** the tracker has been running for some duration and the
  chip is visible
- **WHEN** one second passes
- **THEN** the chip's elapsed-time display has incremented

#### Scenario: Tracker chip and queue chip coexist

- **GIVEN** the tracker is active AND the events queue holds 2
  pending rows
- **WHEN** the home screen renders
- **THEN** both `📍 定位追蹤中` and `待送出 2 筆` chips are visible
- **AND** they do not overlap

### Requirement: Tracker submits pings via LocationRepository against the server endpoint

The `LocationRepository` SHALL provide a `submitBatch(pings:
List<LocationPingDto>) -> Future<SubmitLocationPingsResponse>`
method that wraps `POST /app/checkin/locations`. The body SHALL
follow the server's contract: `{ pings: [{ lat, lng, accuracy?,
occurred_at_client }, ...] }`. The method SHALL throw `ApiException`
on transport / 4xx / 5xx errors as the existing `dio` interceptor
already maps `403 LOCATION_TRACKING_DISABLED` to
`ApiErrorCode.locationTrackingDisabled` (a new entry in the existing
`ApiErrorCode` constants). On `2xx`, the method SHALL deserialize
the response into a typed `SubmitLocationPingsResponse` containing
`acceptedCount: int` and `rejected: List<RejectedPingDto>`, where
`RejectedPingDto` carries `index: int`, `code: String`, `message:
String`.

#### Scenario: Successful batch parses response shape

- **WHEN** `submitBatch` is called with 5 pings and the server returns
  `201 { "accepted_count": 5, "rejected": [] }`
- **THEN** the returned `SubmitLocationPingsResponse` has
  `acceptedCount = 5` and `rejected.isEmpty == true`

#### Scenario: Partial accept response surfaces rejected indices

- **WHEN** `submitBatch` is called with 5 pings and the server returns
  `201 { "accepted_count": 4, "rejected": [{ "index": 2, "code":
  "INVALID_PING_TIMESTAMP", "message": "..." }] }`
- **THEN** the returned response has `acceptedCount = 4`
- **AND** `rejected.length == 1`
- **AND** `rejected[0].index == 2`
- **AND** `rejected[0].code == "INVALID_PING_TIMESTAMP"`

#### Scenario: 403 surfaces as ApiException

- **WHEN** the server returns `403 { "error": { "code":
  "LOCATION_TRACKING_DISABLED" } }`
- **THEN** the call throws `ApiException`
- **AND** the exception's `code` field equals
  `ApiErrorCode.locationTrackingDisabled`
