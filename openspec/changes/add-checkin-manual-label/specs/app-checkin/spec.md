## ADDED Requirements

### Requirement: Every check-in carries a worker-supplied location label

The home screen SHALL provide a location-label field above the action buttons, and its value SHALL be submitted as `manual_label` on whichever action the AppUser presses (`上班`, `下班`, `轉出`, `轉入`). The field SHALL be required: it is the field these workers filled on every record in the system this app replaced.

The field SHALL be cleared after each successful enqueue, so a label is never inherited by a later event. A worker who moves to another site and forgets to update a sticky value would otherwise produce confidently wrong records with nothing in the UI to signal it.

The label SHALL accept free text of 1–120 characters, matching the server's existing validation. The app SHALL NOT restrict the value to a predefined set — the replaced system used free text, and constraining it here would be a behaviour change rather than a port.

The label SHALL travel with the queue row, so an event enqueued offline submits its label when connectivity returns without the worker re-entering anything.

#### Scenario: The label is submitted with the event

- **WHEN** the AppUser enters `乙工地` and presses `[上班]`
- **THEN** the enqueued row carries that label
- **AND** the submitted event's `manual_label` is `乙工地`

#### Scenario: The field clears after each event

- **GIVEN** the AppUser has just submitted an event labelled `乙工地`
- **WHEN** the home screen returns to its resting state
- **THEN** the label field is empty
- **AND** the action buttons are disabled until a new label is supplied

#### Scenario: Each event type carries its own label

- **GIVEN** the AppUser clocked in at `丙工地` and later transfers
- **WHEN** they enter `丁工地` and press `[轉出]`
- **THEN** the transfer event's `manual_label` is `丁工地`
- **AND** the earlier `clock_in` event's label is unchanged

#### Scenario: A label queued offline survives until submission

- **GIVEN** the device is offline
- **WHEN** the AppUser labels an event and presses an action button
- **THEN** the label is stored with the pending row
- **AND** when connectivity returns the event is submitted carrying that label, with no further input

#### Scenario: Free text is always available

- **WHEN** the AppUser is at a site they have never visited
- **THEN** they can type its name
- **AND** the app does not require it to match any existing value

### Requirement: The label field offers the worker's own recent labels

The label field SHALL offer the AppUser's own recently used labels as one-tap options, so that supplying a label is normally a tap rather than typing Chinese on a phone. Without this, requiring a label on every event would be punitive.

The options SHALL be derived from the AppUser's own event history, which the events endpoint already returns — no new endpoint is required. They SHALL cover the last 30 days, be ordered by frequency of use, and be capped at 6. Frequency rather than recency so the dominant site keeps a stable position and becomes muscle memory; a cap of 6 because the median AppUser has used only 7 distinct labels in total.

The computed list SHALL be cached on the device and used when the history cannot be fetched. Offline check-in is a core scenario of this app, and a worker out of signal must still be able to label an event. When no cached list exists, the field SHALL fall back to free text alone.

Selecting an option SHALL fill the field, not submit the event — the action buttons remain the only way to record a check-in.

#### Scenario: Recent labels appear as one-tap options

- **GIVEN** the AppUser's recent events used `甲工地`, `乙工地` and `丙工地`
- **WHEN** they open the home screen
- **THEN** those labels are offered as selectable options
- **AND** selecting one fills the field without submitting anything

#### Scenario: Ordered by frequency, not recency

- **GIVEN** `甲工地` is the AppUser's most-used label and `丁倉庫` their least-used
- **WHEN** the options are rendered
- **THEN** `甲工地` appears before `丁倉庫`

#### Scenario: Capped at six

- **GIVEN** the AppUser has used 19 distinct labels in the last 30 days
- **WHEN** the options are rendered
- **THEN** at most 6 are offered

#### Scenario: Options survive going offline

- **GIVEN** the AppUser has previously loaded their history
- **WHEN** they open the home screen with no connectivity
- **THEN** the cached options are still offered

#### Scenario: A device with no history offers free text only

- **GIVEN** a fresh install with no cached options and no connectivity
- **WHEN** the AppUser opens the home screen
- **THEN** no options are offered
- **AND** the field still accepts typed input

## MODIFIED Requirements

### Requirement: Home action buttons follow the active checkin status

The system SHALL render exactly one set of action buttons on the home screen, derived from the AppUser's effective checkin status (the server-confirmed status overlaid with non-failed local queue events). When `status == off_duty` the screen SHALL show a single `[上班]` button. When `status == on_site` it SHALL show `[下班]` and `[轉出]`. When `status == in_transit` it SHALL show `[下班]` and `[轉入]`. When `Org.checkin.transferEnabled == false`, the system SHALL NOT render `[轉出]` or `[轉入]`; the `on_site` and `in_transit` button sets SHALL collapse to `[下班]` only. The buttons SHALL be disabled while the location permission is `deniedForever`.

The buttons SHALL additionally be disabled while the location-label field is empty. Gating the buttons is how the label is collected without violating the requirement that nothing appear between the tap and the enqueue: the label is supplied *before* the press, exactly as the login form gates its submit on its three fields, so the press itself stays instantaneous. A modal or sheet after the press is prohibited by that requirement and would slow the end of every shift.

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

#### Scenario: Buttons disabled when the label is empty

- **WHEN** the location-label field is empty
- **THEN** the visible action buttons render in disabled state
- **AND** no dialog, sheet, or prompt is shown

#### Scenario: Supplying a label enables the buttons

- **WHEN** the AppUser types a label or selects a recent one
- **THEN** the visible action buttons become enabled
- **AND** pressing one enqueues immediately, with nothing in between

#### Scenario: Buttons disabled when location permission is permanently denied

- **WHEN** `geolocator.checkPermission()` returns `deniedForever`
- **THEN** the visible action buttons render in disabled state
- **AND** an inline banner above them reads `需要定位權限才能打卡` with an `[開啟設定]` button

#### Scenario: Buttons remain enabled when permission has not been determined

- **WHEN** `geolocator.checkPermission()` returns `denied` (the iOS first-install state, treated as "not yet determined")
- **AND** the location-label field is non-empty
- **THEN** the visible action buttons render in enabled state
- **AND** the inline blocker is hidden
- **AND** tapping a button triggers the OS permission dialog before GPS capture

### Requirement: History merges server events with local queue rows

The system SHALL provide a `/history` route rendering a unified timeline of (1) server events fetched from `GET /app/checkin/events?limit=50&before=<oldest_loaded>`, (2) all local `pending_events` rows for the current user, and (3) recently-synced events held in an in-memory cache populated by the queue processor on each successful submit (`SubmitCheckinEventResponse.event` payload). All three sources are sorted by `occurred_at_client` descending and de-duplicated by event `id` (server-fetched and recently-synced rows for the same `id` collapse into a single entry; the server-fetched row wins on conflict). Each row SHALL display a status badge: `pending`, `sending`, `failed`, or `synced` (server-fetched or recently-synced). A `[載入更多]` button SHALL only request additional server pages; local queue rows and recently-synced rows SHALL always be fully visible.

Each row's location line SHALL show `manual_label` and `region_name` together, comma-separated, and SHALL degrade when either is absent: label only, region only, or — when neither exists — the existing coordinate fallback. Showing only the region discards what the worker actually wrote, which for imported records is present on every row.

A local queue row SHALL show its label immediately, since the worker supplied it on this device; it gains the region once the server has reverse-geocoded the synced event.

#### Scenario: Pending and synced rows render together

- **WHEN** the user has 2 local `pending` rows with `occurred_at_client` of `09:30` and `08:00` and the server returns 1 event at `07:00`
- **THEN** the history shows three entries in order: pending 09:30, pending 08:00, synced 07:00

#### Scenario: Both label and region are shown

- **GIVEN** an event with `manual_label = "甲工地"` and `region_name = "高雄市鳳山區頂庄路"`
- **WHEN** its row renders
- **THEN** the location line reads `甲工地, 高雄市鳳山區頂庄路`

#### Scenario: Label alone when the region is missing

- **GIVEN** an event with `manual_label = "甲工地"` and no `region_name`
- **WHEN** its row renders
- **THEN** the location line reads `甲工地`

#### Scenario: Region alone for events recorded before labels existed

- **GIVEN** an event with `region_name` and no `manual_label`
- **WHEN** its row renders
- **THEN** the location line reads the region

#### Scenario: Coordinates when neither exists

- **GIVEN** an event with neither `manual_label` nor `region_name`
- **WHEN** its row renders
- **THEN** the location line falls back to the coordinates

#### Scenario: A pending row shows its label before syncing

- **GIVEN** a local queue row the worker labelled `乙工地`
- **WHEN** it renders while still `pending`
- **THEN** the location line shows `乙工地`
- **AND** the region is added once the event syncs and the server has geocoded it

#### Scenario: Just-synced event stays visible after queue row is deleted

- **WHEN** a `pending_events` row at `09:30` is submitted and the server returns `201` with the corresponding `CheckinEventDto`
- **AND** the queue row is deleted per the strict-serialization rule
- **THEN** the history view continues to show one row at `09:30` with the badge `synced` (or `已上傳`)
- **AND** the row carries the server's `region_name` once the server has reverse-geocoded it
