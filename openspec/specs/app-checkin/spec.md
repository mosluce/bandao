# app-checkin Specification

## Purpose
TBD - created by archiving change add-app-checkin. Update Purpose after archive.
## Requirements
### Requirement: Home action buttons follow the active checkin status

The system SHALL render exactly one set of action buttons on the home screen, derived from the AppUser's effective checkin status (the server-confirmed status overlaid with non-failed local queue events). When `status == off_duty` the screen SHALL show a single `[上班]` button. When `status == on_site` it SHALL show `[下班]` and `[轉出]`. When `status == in_transit` it SHALL show `[下班]` and `[轉入]`. When `Org.checkin.transferEnabled == false`, the system SHALL NOT render `[轉出]` or `[轉入]`; the `on_site` and `in_transit` button sets SHALL collapse to `[下班]` only. The buttons SHALL be disabled while the location permission is `deniedForever`.

#### Scenario: Off-duty AppUser sees only the clock-in button

- **WHEN** the AppUser's effective status is `off_duty`
- **THEN** the home screen shows the `[上班]` button
- **AND** does NOT show `[下班]`, `[轉出]`, or `[轉入]`

#### Scenario: On-site AppUser sees clock-out and transfer-out

- **WHEN** the effective status is `on_site` and `Org.checkin.transferEnabled` is `true`
- **THEN** the home screen shows `[下班]` and `[轉出]`
- **AND** does NOT show `[上班]` or `[轉入]`

#### Scenario: In-transit AppUser sees clock-out and transfer-in

- **WHEN** the effective status is `in_transit` and `Org.checkin.transferEnabled` is `true`
- **THEN** the home screen shows `[下班]` and `[轉入]`
- **AND** does NOT show `[上班]` or `[轉出]`

#### Scenario: On-site AppUser hides transfer when org disables transfers

- **WHEN** the effective status is `on_site` and `Org.checkin.transferEnabled` is `false`
- **THEN** the home screen shows only `[下班]`
- **AND** does NOT show `[轉出]`, `[上班]`, or `[轉入]`

#### Scenario: In-transit AppUser hides transfer when org disables transfers

- **WHEN** the effective status is `in_transit` and `Org.checkin.transferEnabled` is `false`
- **THEN** the home screen shows only `[下班]`
- **AND** does NOT show `[轉入]`, `[上班]`, or `[轉出]`

#### Scenario: Buttons disabled when location permission is permanently denied

- **WHEN** `geolocator.checkPermission()` returns `deniedForever`
- **THEN** the visible action buttons render in disabled state
- **AND** an inline banner above them reads `需要定位權限才能打卡` with an `[開啟設定]` button

#### Scenario: Buttons remain enabled when permission has not been determined

- **WHEN** `geolocator.checkPermission()` returns `denied` (the iOS first-install state, treated as "not yet determined")
- **THEN** the visible action buttons render in enabled state
- **AND** the inline blocker is hidden
- **AND** tapping a button triggers the OS permission dialog before GPS capture

### Requirement: Tapping an action button enqueues an event with captured GPS

The system SHALL, when an enabled action button is tapped, capture the current GPS coordinates with `LocationAccuracy.high` and a 10-second timeout, fall back to `getLastKnownPosition()` on timeout, insert a row in the `pending_events` queue with the captured coordinates, the user's current `app_user_id`, the event type implied by the tapped button, `occurred_at_client = DateTime.now().toIso8601String()`, status `pending`, attempts `0`, and immediately wake the queue processor. The system SHALL NOT show a confirmation dialog, modal, or bottom sheet between the tap and the enqueue. If GPS capture returns no position even after fallback, the system SHALL surface an error toast and SHALL NOT enqueue the row.

#### Scenario: Tap captures GPS and enqueues a pending row

- **WHEN** the AppUser taps `[上班]` and GPS returns coordinates `(25.0479, 121.5640)` with accuracy `15`
- **THEN** a `pending_events` row is inserted with `event_type = clock_in`, `lat = 25.0479`, `lng = 121.5640`, `accuracy = 15`, `app_user_id` matching the current user, `status = pending`, `occurred_at_client` equal to the wall time at the moment of tap (RFC3339 with offset), and `attempts = 0`
- **AND** the queue processor is signalled to wake

#### Scenario: GPS timeout falls back to last known position

- **WHEN** `getCurrentPosition()` does not resolve within 10 seconds
- **AND** `getLastKnownPosition()` returns coordinates
- **THEN** the row is enqueued using the last-known coordinates

#### Scenario: GPS unavailable rejects the tap

- **WHEN** both `getCurrentPosition()` and `getLastKnownPosition()` return `null`
- **THEN** no row is inserted into `pending_events`
- **AND** the user sees an error toast indicating the location is unavailable

### Requirement: Optimistic status overlay reflects pending and sending events

The system SHALL render the `effective` checkin status as the result of replaying the local queue's `pending` and `sending` rows on top of the most recent server-confirmed `CheckinUserStatusDto.status`. `failed` rows SHALL be excluded from the overlay (the status rolls back as if the failed event never happened). When the queue is empty, the effective status SHALL equal the server-confirmed status exactly.

#### Scenario: Pending event drives optimistic status

- **WHEN** the server-confirmed status is `off_duty`
- **AND** a `pending_events` row exists with `event_type = clock_in`, `status = pending`
- **THEN** the home screen renders effective status `on_site`
- **AND** the action buttons reflect `on_site` (showing `[下班]` and `[轉出]`)

#### Scenario: Failed event rolls back optimistic status

- **WHEN** the server-confirmed status is `off_duty`
- **AND** a `pending_events` row with `event_type = clock_in` is marked `failed` (e.g. server rejected with `INVALID_TRANSITION` because the AppUser was already `on_site` from a prior shift)
- **THEN** the home screen's effective status equals the server-confirmed `off_duty`
- **AND** the action buttons return to the `off_duty` set

#### Scenario: Empty queue collapses to server status

- **WHEN** the `pending_events` table holds zero rows for the current user
- **THEN** the effective status equals the server-confirmed status from `CheckinUserStatusDto.status`

### Requirement: Queue processor enforces strict serialization

The system SHALL maintain at most one queue row in `sending` state at any moment. The processor SHALL pick the oldest `pending` row by `occurred_at_client`, transition it to `sending`, increment `attempts`, and `POST /app/checkin/events` with the row's payload. On `201` the system SHALL delete the row and immediately attempt to advance to the next pending row. On any retryable failure the system SHALL leave the row in `pending` state for the next processor tick.

#### Scenario: Single in-flight constraint

- **WHEN** the queue contains 5 `pending` rows
- **AND** the processor selects the oldest and marks it `sending`
- **THEN** no other row is also in `sending` state
- **AND** subsequent ticks see the in-flight row and return without picking another row

#### Scenario: Successful submission advances the queue

- **WHEN** the in-flight row's POST returns `201`
- **THEN** that row is deleted from `pending_events`
- **AND** the processor immediately picks the next oldest `pending` row

### Requirement: 4xx state-machine and ordering errors mark the row failed without retry

The system SHALL, on receiving any of `INVALID_TRANSITION`, `OUT_OF_ORDER`, `TRANSFER_DISABLED`, or `NEEDS_PASSWORD_CHANGE` from `POST /app/checkin/events`, mark the in-flight row `status = failed`, store the response's `error.code` and `error.message` on the row, and SHALL NOT retry that row. The processor SHALL then advance to the next `pending` row (if any).

#### Scenario: INVALID_TRANSITION marks failed and skips

- **WHEN** the server returns `422 INVALID_TRANSITION` for the in-flight row
- **THEN** the row's `status` becomes `failed`, `last_error_code = "INVALID_TRANSITION"`, `last_error_message` matches the response body
- **AND** the processor picks the next `pending` row

#### Scenario: OUT_OF_ORDER marks failed and skips

- **WHEN** the server returns `409 OUT_OF_ORDER`
- **THEN** the row is marked `failed` with the corresponding error details

#### Scenario: TRANSFER_DISABLED marks failed and skips

- **WHEN** the server returns `403 TRANSFER_DISABLED` for a `transfer_out` or `transfer_in` event
- **THEN** the row is marked `failed`

### Requirement: 401 marks the row failed and signals auth state

The system SHALL, on receiving `401 UNAUTHORIZED` from `POST /app/checkin/events`, mark the in-flight row `failed`, signal the auth state machine to clear the bearer token (returning the AppUser to `/login`), and pause queue processing until the next successful login.

#### Scenario: 401 pauses the queue and clears the session

- **WHEN** the in-flight row's POST returns `401`
- **THEN** the row's `status` becomes `failed`
- **AND** the bearer token is cleared from secure storage
- **AND** the AppUser is navigated to `/login`
- **AND** the queue processor stops attempting further submissions until the next successful login

### Requirement: 5xx and network errors retry with exponential backoff

The system SHALL, on receiving any 5xx response or network error from `POST /app/checkin/events`, return the in-flight row to `status = pending` (keeping the incremented `attempts`), and schedule the next attempt with exponential backoff. The backoff sequence SHALL be `1, 2, 4, 8, 16, 30s` and SHALL cap at 30 seconds. There SHALL be no maximum retry count. While `connectivity_plus` reports the device as offline, `attempts` SHALL NOT be incremented and the backoff window SHALL NOT grow.

#### Scenario: 5xx retries with backoff

- **WHEN** the in-flight row's POST returns `500`
- **AND** the device is online
- **THEN** the row returns to `status = pending`, `attempts` is incremented, and the next attempt is scheduled `1, 2, 4, 8, 16, or 30s` later (depending on attempt count)
- **AND** no other queue row is processed in the meantime

#### Scenario: Network error retries with backoff

- **WHEN** dio reports a connection error (no response from the server)
- **AND** the device is online
- **THEN** the row returns to `status = pending` with `attempts` incremented and a backoff schedule

#### Scenario: Offline doesn't grow the backoff window

- **WHEN** `connectivity_plus` reports the device as offline
- **THEN** the queue processor does NOT mark the row as `sending` and does NOT consume an attempt
- **AND** when connectivity returns the next attempt happens at the smallest backoff step appropriate for the current `attempts` count

### Requirement: Queue processor wakes on enqueue and connectivity restoration

The system SHALL signal the queue processor to wake on every successful enqueue (`pending_events` insert) and on every `connectivity_plus` transition from offline to online. The processor SHALL also tick at most once per second while the app is in foreground.

#### Scenario: Enqueue wakes the processor

- **WHEN** a new row is inserted into `pending_events`
- **THEN** the processor is invoked within 100ms of the insert (no idle wait)

#### Scenario: Connectivity restoration wakes the processor

- **WHEN** `connectivity_plus` transitions to online with at least one `pending` row in the queue
- **THEN** the processor is invoked

### Requirement: Background sync via workmanager

The system SHALL use the `workmanager` package to schedule background sync of the queue. On Android the system SHALL register a `OneTimeWorkRequest` with `Constraints(networkType: connected)` whenever a row is enqueued. On iOS the system SHALL register a `BGProcessingTask` once on app start with identifier `tw.ccmos.app.bandao.queue-drain`. The background callback SHALL run the same processor logic as the foreground tick. The system SHALL document in `app/README.md` and a one-shot in-app onboarding tip that iOS background scheduling is best-effort and the OS may delay execution.

#### Scenario: Android enqueues a OneTimeWorkRequest on enqueue

- **WHEN** a row is inserted into `pending_events` on Android
- **THEN** a workmanager `OneTimeWorkRequest` is registered with the queue-drain identifier
- **AND** the request carries `networkType: connected` so it doesn't run while offline

#### Scenario: iOS registers BGProcessingTask once on app start

- **WHEN** the app launches on iOS
- **THEN** workmanager registers a `BGProcessingTask` with identifier `tw.ccmos.app.bandao.queue-drain`
- **AND** the task descriptor declares it requires network connectivity

#### Scenario: Onboarding tip explains iOS background limits

- **WHEN** an iOS AppUser successfully logs in for the first time on this build
- **THEN** the home screen surfaces a one-shot dismissible tip explaining that iOS may delay background sync
- **AND** the tip is not shown again on subsequent launches

### Requirement: Login-handover queue wipe protects against device sharing

The system SHALL, on every successful login or auto-login, compare each `pending_events` row's `app_user_id` against the currently authenticated `user.id`, delete every row whose `app_user_id` does not match, and surface a one-shot toast `前個帳號的 N 筆未送事件已清除` when one or more rows are deleted. Rows whose `app_user_id` matches SHALL be preserved untouched (this covers the same-user re-login case after token expiry).

#### Scenario: Different user wipes the previous queue

- **WHEN** the queue contains 3 rows with `app_user_id = X` and the current login resolves to `user.id = Y`
- **THEN** all 3 rows are deleted from `pending_events`
- **AND** a toast `前個帳號的 3 筆未送事件已清除` appears once

#### Scenario: Same user preserves the queue

- **WHEN** the queue contains 2 rows with `app_user_id = X` and the current login resolves to `user.id = X`
- **THEN** no rows are deleted
- **AND** no toast is shown

### Requirement: History merges server events with local queue rows

The system SHALL provide a `/history` route rendering a unified timeline of (1) server events fetched from `GET /app/checkin/events?limit=50&before=<oldest_loaded>`, (2) all local `pending_events` rows for the current user, and (3) recently-synced events held in an in-memory cache populated by the queue processor on each successful submit (`SubmitCheckinEventResponse.event` payload). All three sources are sorted by `occurred_at_client` descending and de-duplicated by event `id` (server-fetched and recently-synced rows for the same `id` collapse into a single entry; the server-fetched row wins on conflict). Each row SHALL display a status badge: `pending`, `sending`, `failed`, or `synced` (server-fetched or recently-synced). A `[載入更多]` button SHALL only request additional server pages; local queue rows and recently-synced rows SHALL always be fully visible.

#### Scenario: Pending and synced rows render together

- **WHEN** the user has 2 local `pending` rows with `occurred_at_client` of `09:30` and `08:00` and the server returns 1 event at `07:00`
- **THEN** the history shows three entries in order: pending 09:30, pending 08:00, synced 07:00

#### Scenario: Just-synced event stays visible after queue row is deleted

- **WHEN** a `pending_events` row at `09:30` is submitted and the server returns `201` with the corresponding `CheckinEventDto`
- **AND** the queue row is deleted per the strict-serialization rule
- **THEN** the history view continues to show one row at `09:30` with the badge `synced` (or `已上傳`)
- **AND** the row carries the server's `region_name` once the server has reverse-geocoded it

#### Scenario: Recently-synced event is de-duplicated when a server fetch returns it

- **WHEN** an event at `09:30` is in the recently-synced cache
- **AND** the user taps `[載入更多]` and the next server page contains an event with the same `id` at `09:30`
- **THEN** the history view shows exactly one row at `09:30` (no duplicate)
- **AND** the row carries the server-fetched values (region name, server timestamp, etc.)

#### Scenario: Failed row renders with error and dismiss controls

- **WHEN** a queue row has `status = failed`, `last_error_code = "INVALID_TRANSITION"`, `last_error_message = "cannot clock_in from on_site"`
- **THEN** the history row shows the badge `failed`, the inline message `INVALID_TRANSITION`, and the friendly Chinese `cannot clock_in from on_site` (or its mapped translation)
- **AND** a `[複製細節]` action and a `[關閉]` action are visible

#### Scenario: Load more only fetches server pages

- **WHEN** the user taps `[載入更多]` while 1 local pending row and 50 synced rows are visible
- **THEN** the next 50 server rows are appended (using the oldest currently-displayed server row's `occurred_at_client` as the `before` cursor)
- **AND** the local pending row remains in place above them

### Requirement: Failed rows are user-cancellable; pending and sending are not

The system SHALL, on the `failed` row's `[關閉]` action, delete the row from `pending_events`. The system SHALL NOT provide any user-driven cancellation for rows in `pending` or `sending` state — those rows can only leave the queue by being submitted (`201` → deleted) or transitioning to `failed`. The `[複製細節]` action SHALL place a plaintext blob on the system clipboard containing `event_id` (synthesized from queue id), `event_type`, `occurred_at_client`, lat/lng with optional accuracy, `attempts`, `last_error_code`, `last_error_message`, `last_attempt_at`, and `app_user_id`.

#### Scenario: Dismiss failed row deletes it

- **WHEN** the user taps `[關閉]` on a `failed` history row
- **THEN** the corresponding `pending_events` row is deleted
- **AND** the row disappears from the history view

#### Scenario: Pending rows have no cancel control

- **WHEN** a row is in `pending` state
- **THEN** the history row shows the `pending` badge but NO `[關閉]` action

#### Scenario: Sending rows have no cancel control

- **WHEN** a row is in `sending` state
- **THEN** the history row shows the `sending` badge but NO `[關閉]` action

#### Scenario: Copy detail produces a plaintext blob

- **WHEN** the user taps `[複製細節]` on a failed row
- **THEN** the system clipboard contains a plaintext blob with one labelled field per line covering: `event_id`, `event_type`, `occurred_at_client`, `lat, lng (±accuracy)`, `attempts`, `last_error_code`, `last_error_message`, `last_attempt_at`, `app_user_id`

### Requirement: Queue indicator on home

The system SHALL render a chip on the home screen reflecting the queue's state. When the queue holds zero non-`done` rows for the current user, the chip SHALL be hidden. When at least one row is in `pending` or `sending`, the chip SHALL display `送出中` (when any row is `sending`) or `待送出 N 筆` (when all are `pending`). When at least one row is `failed`, the chip SHALL display `1 筆失敗` (or `N 筆失敗`). Tapping the chip SHALL navigate to `/history`.

#### Scenario: Empty queue hides the chip

- **WHEN** the queue holds zero rows for the current user
- **THEN** no queue chip is visible on home

#### Scenario: Pending rows show count

- **WHEN** the queue holds 3 `pending` rows and 0 `sending` / `failed`
- **THEN** the chip displays `待送出 3 筆`

#### Scenario: Sending shows in-flight indicator

- **WHEN** the queue holds 1 `sending` row
- **THEN** the chip displays `送出中`

#### Scenario: Failed counts shown distinctly

- **WHEN** the queue holds 1 `failed` row alongside 2 `pending`
- **THEN** the chip surfaces both: `待送出 2 筆 · 1 筆失敗` (or equivalent)

#### Scenario: Tap routes to history

- **WHEN** the user taps the queue chip
- **THEN** the app navigates to `/history`

### Requirement: Location permission gates the action buttons

The system SHALL, on every render of the home screen, check the current location permission via `geolocator.checkPermission()`. When the permission is `deniedForever`, the action buttons SHALL render disabled and an inline blocker SHALL appear above them with the copy `需要定位權限才能打卡` and an `[開啟設定]` button. The `[開啟設定]` action SHALL open the app's OS settings page via the `app_settings` package. When the permission is `denied` (the iOS first-install "not yet determined" state), the buttons SHALL remain enabled and the blocker SHALL stay hidden — the system permission dialog fires on the first event-button tap. The system SHALL NOT pre-prompt for permission on app start.

#### Scenario: deniedForever blocks buttons and shows blocker

- **WHEN** location permission is `deniedForever`
- **THEN** the action buttons render disabled
- **AND** the inline blocker is visible with `[開啟設定]`

#### Scenario: Never-asked state does not block

- **WHEN** location permission is `denied` (and the user has never been prompted)
- **THEN** the action buttons render enabled
- **AND** the inline blocker is NOT visible

#### Scenario: First tap requests permission

- **WHEN** the user taps an enabled action button for the first time after install
- **AND** location permission is `denied`
- **THEN** `geolocator.requestPermission()` is invoked, surfacing the OS permission dialog
- **AND** the captured GPS coordinates use the resulting permission grant

#### Scenario: deniedForever exposes settings deep-link

- **WHEN** location permission is `deniedForever`
- **THEN** the inline blocker exposes `[開啟設定]` linking via `app_settings` to the OS app-settings page
- **AND** the action buttons remain disabled

### Requirement: Logout requires confirmation when the queue is non-empty

The system SHALL, when the home screen's `登出` menu action is triggered, count `pending_events` rows for the currently authenticated user across all of `pending`, `sending`, and `failed` statuses. When the count is greater than zero, the system SHALL surface a confirmation dialog with the title or body containing the count and the explanation that different-account login wipes those rows. The dialog SHALL offer at minimum a `取消` action and a `仍要登出` (or equivalent) confirm action. The system SHALL only proceed with `authProvider.logout()` if the user confirms; cancel SHALL leave the user on the home screen with no state change. When the count is zero, the system SHALL proceed with logout immediately (no dialog).

#### Scenario: Non-empty queue prompts confirmation

- **WHEN** the queue holds 3 rows for the current user (any combination of `pending`, `sending`, `failed`)
- **AND** the user taps `登出`
- **THEN** a confirmation dialog appears with copy referencing the count `3` and the wipe consequence
- **AND** the dialog offers `取消` and `仍要登出` (or equivalent) actions

#### Scenario: Confirm proceeds with logout

- **WHEN** the user taps `仍要登出` on the confirmation dialog
- **THEN** `authProvider.logout()` is invoked
- **AND** the user is navigated to `/login`

#### Scenario: Cancel preserves session

- **WHEN** the user taps `取消` on the confirmation dialog
- **THEN** the user remains on the home screen
- **AND** the bearer token, queue state, and all session state are unchanged

#### Scenario: Empty queue logs out without prompt

- **WHEN** the queue holds zero rows for the current user
- **AND** the user taps `登出`
- **THEN** `authProvider.logout()` is invoked immediately
- **AND** no confirmation dialog appears

### Requirement: History pull-to-refresh refetches the server first page and refreshes status

The system SHALL provide a pull-to-refresh gesture on the `/history` screen. The gesture SHALL trigger: (a) clearing the recently-synced events cache, (b) resetting the locally-held server-events list and pagination state to empty / first-page, (c) fetching the first server page via `GET /app/checkin/events?limit=50` (no `before` cursor), and (d) refreshing the `checkinStatusProvider`. The local queue stream is already live and SHALL NOT need explicit invalidation.

#### Scenario: Pull-to-refresh resets cache and fetches first page

- **WHEN** the user pulls down on `/history`
- **THEN** the recently-synced events cache is cleared
- **AND** the locally-held server-events list is reset (no `before` cursor on the first fetch)
- **AND** `GET /app/checkin/events?limit=50` is fired
- **AND** `checkinStatusProvider` is refreshed

#### Scenario: Pull-to-refresh keeps queue rows visible

- **WHEN** the user has 1 local `pending` row and pulls down on `/history`
- **THEN** the `pending` row remains visible during and after the refresh (it is sourced from the live queue stream, not the server fetch)

### Requirement: App resume refreshes the cached identity and checkin status

The system SHALL, on `AppLifecycleState.resumed` while the home screen is mounted, refresh both the cached `/me` response (via `authProvider.refreshMe()`) and the cached `checkin status` (via `checkinStatusProvider.refresh()`). The system SHALL NOT re-run the login-handover queue wipe on resume (that is a login-time guard). The system SHALL continue to refresh the location permission state on resume (existing behavior is preserved).

#### Scenario: Resume refreshes /me and status

- **WHEN** the app transitions from background to foreground while the home screen is mounted
- **THEN** `GET /app/me` is fired
- **AND** `GET /app/checkin/status` is fired
- **AND** `geolocator.checkPermission()` is re-evaluated

#### Scenario: Resume reflects admin-side transfer flip

- **WHEN** the admin flips `Org.checkin.transferEnabled` from `true` to `false` while the AppUser is in the background
- **AND** the AppUser brings the app to the foreground while on the home screen
- **THEN** the cached `Org.checkin.transferEnabled` updates to `false` after the resume refresh
- **AND** the visible button set updates to hide `[轉出]` / `[轉入]` per the home action buttons rules

#### Scenario: Resume does not re-run handover wipe

- **WHEN** the app transitions from background to foreground
- **THEN** the queue's `pending_events` rows are not modified by the resume hook
- **AND** the `pendingHandoverNoticeProvider` is not set

### Requirement: Consent dialog gates the first clock_in when location tracking is enabled

The system SHALL, when an AppUser taps `[上班]` and `Org.checkin
.location_tracking_enabled == true` and the secure-storage flag
`bandao.location_tracking.consent.<app_user_id>` is absent, surface a
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
  `bandao.location_tracking.consent.<app_user_id>`
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
`bandao.location_tracking.last_clean_stop` to the current ISO8601
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
- **THEN** secure storage's `bandao.location_tracking.last_clean_stop`
  is set to the current ISO8601 timestamp
- **AND** the tracker is stopped

#### Scenario: Force-quit during shift surfaces the banner on next launch

- **GIVEN** the tracker was active and the user force-quit 班到 from
  the multitasking switcher (no clean shutdown, no flag write)
- **WHEN** the user re-opens 班到 and the home screen finishes its
  initial frame
- **THEN** the recovery banner is visible
- **AND** the tracker has been started

#### Scenario: Banner does not appear when status is off_duty

- **GIVEN** the AppUser is `off_duty` (no shift in progress)
- **WHEN** the user opens 班到
- **THEN** no recovery banner appears (no shift to recover)

#### Scenario: Banner does not appear after a clean stop

- **GIVEN** the tracker stopped cleanly during the previous app
  session (flag is fresh)
- **WHEN** the user re-opens 班到
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
