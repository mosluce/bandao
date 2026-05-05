## ADDED Requirements

### Requirement: Home action buttons follow the active checkin status

The system SHALL render exactly one set of action buttons on the home screen, derived from the AppUser's effective checkin status (the server-confirmed status overlaid with non-failed local queue events). When `status == off_duty` the screen SHALL show a single `[上班]` button. When `status == on_site` it SHALL show `[下班]` and `[轉出]`. When `status == in_transit` it SHALL show `[下班]` and `[轉入]`. The buttons SHALL be disabled while the location permission is missing or denied.

#### Scenario: Off-duty AppUser sees only the clock-in button

- **WHEN** the AppUser's effective status is `off_duty`
- **THEN** the home screen shows the `[上班]` button
- **AND** does NOT show `[下班]`, `[轉出]`, or `[轉入]`

#### Scenario: On-site AppUser sees clock-out and transfer-out

- **WHEN** the effective status is `on_site`
- **THEN** the home screen shows `[下班]` and `[轉出]`
- **AND** does NOT show `[上班]` or `[轉入]`

#### Scenario: In-transit AppUser sees clock-out and transfer-in

- **WHEN** the effective status is `in_transit`
- **THEN** the home screen shows `[下班]` and `[轉入]`
- **AND** does NOT show `[上班]` or `[轉出]`

#### Scenario: Buttons disabled when location permission denied

- **WHEN** `geolocator.checkPermission()` returns `denied` or `deniedForever`
- **THEN** the visible action buttons render in disabled state
- **AND** an inline banner above them reads `需要定位權限才能打卡` with an `[開啟設定]` button

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

The system SHALL use the `workmanager` package to schedule background sync of the queue. On Android the system SHALL register a `OneTimeWorkRequest` with `Constraints(networkType: connected)` whenever a row is enqueued. On iOS the system SHALL register a `BGProcessingTask` once on app start with identifier `tw.ccmos.app.argus.queue-drain`. The background callback SHALL run the same processor logic as the foreground tick. The system SHALL document in `app/README.md` and a one-shot in-app onboarding tip that iOS background scheduling is best-effort and the OS may delay execution.

#### Scenario: Android enqueues a OneTimeWorkRequest on enqueue

- **WHEN** a row is inserted into `pending_events` on Android
- **THEN** a workmanager `OneTimeWorkRequest` is registered with the queue-drain identifier
- **AND** the request carries `networkType: connected` so it doesn't run while offline

#### Scenario: iOS registers BGProcessingTask once on app start

- **WHEN** the app launches on iOS
- **THEN** workmanager registers a `BGProcessingTask` with identifier `tw.ccmos.app.argus.queue-drain`
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

The system SHALL provide a `/history` route rendering a unified timeline of (1) server events fetched from `GET /app/checkin/events?limit=50&before=<oldest_loaded>` and (2) all local `pending_events` rows for the current user, sorted by `occurred_at_client` descending. Each row SHALL display a status badge: `pending`, `sending`, `failed`, or `synced` (server-fetched). A `[載入更多]` button SHALL only request additional server pages; local rows SHALL always be fully visible.

#### Scenario: Pending and synced rows render together

- **WHEN** the user has 2 local `pending` rows with `occurred_at_client` of `09:30` and `08:00` and the server returns 1 event at `07:00`
- **THEN** the history shows three entries in order: pending 09:30, pending 08:00, synced 07:00

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

The system SHALL, on every render of the home screen, check the current location permission via `geolocator.checkPermission()`. When the permission is `denied` or `deniedForever`, the action buttons SHALL render disabled and an inline blocker SHALL appear above them with the copy `需要定位權限才能打卡` and an `[開啟設定]` button. The `[開啟設定]` action SHALL open the app's OS settings page via the `app_settings` package. The system SHALL NOT pre-prompt for permission on app start; the prompt SHALL only fire on the first event-button tap.

#### Scenario: Denied permission blocks buttons

- **WHEN** location permission is `denied`
- **THEN** the action buttons render disabled
- **AND** the inline blocker is visible

#### Scenario: First tap requests permission

- **WHEN** the user taps an enabled action button for the first time after install
- **AND** location permission is `notDetermined`
- **THEN** `geolocator.requestPermission()` is invoked, surfacing the OS permission dialog

#### Scenario: deniedForever exposes settings deep-link

- **WHEN** location permission is `deniedForever`
- **THEN** the inline blocker exposes `[開啟設定]` linking via `app_settings` to the OS app-settings page
- **AND** the action buttons remain disabled
