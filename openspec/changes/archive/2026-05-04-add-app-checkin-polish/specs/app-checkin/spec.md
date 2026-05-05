## MODIFIED Requirements

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

## ADDED Requirements

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
