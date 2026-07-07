# app-personal-trajectory Specification

## Purpose
TBD - created by archiving change add-app-personal-trajectory. Update Purpose after archive.
## Requirements
### Requirement: Main navigation SHALL expose a top-level "我的軌跡" destination

The app SHALL render a Material `NavigationBar` (or equivalent indexed-stack shell) as the persistent bottom chrome on every authenticated top-level screen. The bar SHALL contain at minimum three destinations in this order: `/home` (首頁), `/history` (歷史), `/trajectory` (我的軌跡). The trajectory destination SHALL be reachable in one tap from the home, history, and trajectory screens themselves — it SHALL NOT be hidden behind a settings, drawer, or overflow menu.

Navigation between destinations SHALL preserve each destination's local state (clock-in pill, scroll position) for the duration of the session.

#### Scenario: NavigationBar visible on home

- **WHEN** an authenticated AppUser is on `/home`
- **THEN** a `NavigationBar` is rendered at the bottom of the screen
- **AND** it shows three destinations: 首頁, 歷史, 我的軌跡

#### Scenario: One-tap reach from any top-level surface

- **GIVEN** the user is on `/history`
- **WHEN** the user taps the 我的軌跡 destination in the nav bar
- **THEN** the app routes to `/trajectory`
- **AND** the active destination indicator highlights 我的軌跡

#### Scenario: Home state survives tab switch

- **GIVEN** the user has tapped 上班 on `/home` and the status pill reads "工作中"
- **WHEN** the user switches to /trajectory and back to /home
- **THEN** the status pill still reads "工作中"
- **AND** the clock-in button does not re-render in its idle state

### Requirement: Trajectory screen SHALL render the AppUser's own daily polyline with summary stats

The `/trajectory` screen SHALL fetch the caller's own pings for the active date via `GET /app/checkin/me/locations?from=&to=` (range = one calendar day in the Org's timezone). When pings are returned the screen SHALL render:

- A `flutter_map` map view with OSM/CARTO Positron tiles (matching admin-web's tile choice) and the required `© OpenStreetMap contributors © CARTO` attribution string.
- A polyline drawn through pings ordered ascending by `occurred_at_client`.
- A start marker at the first ping and an end marker at the last ping.
- Auto-fit map bounds to encompass all rendered points on initial load.
- Three summary stats below the map:
  - **走動距離**: sum of geodesic distances between consecutive pings, displayed in km to one decimal place.
  - **在班時長**: elapsed time between the first and last ping of the day, formatted as `H 小時 M 分`.
  - **位置點**: integer count of pings returned.

When zero pings are returned for the date range the screen SHALL render the text `該日無軌跡資料` and SHALL NOT instantiate the map.

#### Scenario: Today renders polyline and stats

- **WHEN** the AppUser opens `/trajectory` with today selected and the server returns 38 pings
- **THEN** a map renders with a polyline through 38 ordered points
- **AND** a start marker is drawn at the earliest ping
- **AND** an end marker is drawn at the latest ping
- **AND** stats show 走動距離, 在班時長, 位置點 = 38

#### Scenario: Empty day shows text, no map

- **WHEN** the server returns zero pings for the selected date
- **THEN** the screen shows the text `該日無軌跡資料`
- **AND** no map widget is instantiated
- **AND** no attribution string is shown

#### Scenario: Polyline ordered chronologically

- **WHEN** the server returns pings newest-first (per the API contract)
- **THEN** the client sorts ascending by `occurred_at_client` before drawing the polyline

#### Scenario: Auto fit bounds on initial render

- **WHEN** pings render for the first time on the screen
- **THEN** the map viewport encompasses every plotted coordinate

### Requirement: Trajectory screen SHALL offer a date selector covering today plus the previous 7 days

The `/trajectory` screen SHALL provide a date selector (e.g. a dropdown in the app bar) listing today plus the seven previous calendar days in the Org's timezone, newest first. Selecting an option SHALL re-fetch pings for that day and re-render the map and stats. The default selected option on first open SHALL be today.

The selector SHALL NOT offer dates older than 7 days from today nor any date in the future, regardless of what data exists server-side.

#### Scenario: Default is today

- **WHEN** an AppUser opens `/trajectory` for the first time in the session
- **THEN** the date selector shows today as the active selection in the Org's timezone

#### Scenario: Picker offers exactly 8 options

- **WHEN** the user opens the date selector
- **THEN** the option list contains exactly 8 entries — today and the seven previous calendar days

#### Scenario: Selecting a past day re-fetches

- **GIVEN** the user is viewing today's polyline
- **WHEN** the user picks yesterday from the selector
- **THEN** the screen issues a new `GET /app/checkin/me/locations` call with yesterday's date range
- **AND** the map and stats re-render for the returned data

#### Scenario: Picker does not offer dates older than 7 days

- **WHEN** the user opens the date selector
- **THEN** the oldest option SHALL be exactly 7 calendar days before today
- **AND** no option older than that is offered

### Requirement: Trajectory screen SHALL handle the missing-permission state gracefully

When location permission has been denied (system level), the trajectory screen SHALL NOT show a broken empty state. It SHALL instead render a primer card explaining that location permission is required to record a work-day trail, with a button that opens the system Settings via `app_settings` so the user can grant permission. The map widget SHALL NOT be instantiated in this state.

This state SHALL be distinct from the "permission granted, but zero pings today" state.

#### Scenario: Permission denied shows a primer card

- **GIVEN** `Geolocator.checkPermission()` returns `denied` or `deniedForever`
- **WHEN** the AppUser opens `/trajectory`
- **THEN** the screen shows a card explaining location permission is required
- **AND** the card has a button labelled "前往系統設定" that calls `AppSettings.openAppSettings`
- **AND** no map widget is instantiated

#### Scenario: Permission granted but no pings is still the empty-data path

- **GIVEN** permission is granted but no pings exist for today
- **WHEN** the AppUser opens `/trajectory`
- **THEN** the screen shows `該日無軌跡資料` (not the permission primer card)

### Requirement: Home screen SHALL show a dynamic "我的今天" summary card

The home screen SHALL render a "我的今天" card showing the AppUser's distance walked and elapsed on-shift duration for the current day, computed from `GET /app/checkin/me/locations` for today's range. The card SHALL be visible whenever the AppUser has at least one ping for today OR is currently on shift; it SHALL NOT show on a no-data, off-shift day.

Tapping the card SHALL route the user to `/trajectory` with today selected.

During an active shift, the displayed stats SHALL refresh at most once per 60 seconds (matching the existing ping enqueue throttle); a refresh trigger SHALL also fire on app foreground.

#### Scenario: Card visible on a day with pings

- **GIVEN** the AppUser has at least one ping for today
- **WHEN** the user opens `/home`
- **THEN** the "我的今天" card is rendered
- **AND** it shows 走動距離 and 在班時長 values computed from today's pings

#### Scenario: Card hidden on a no-data off-shift day

- **GIVEN** the AppUser is off shift and has zero pings for today
- **WHEN** the user opens `/home`
- **THEN** the "我的今天" card is not rendered

#### Scenario: Card tap routes to trajectory

- **WHEN** the user taps the "我的今天" card
- **THEN** the app routes to `/trajectory` with today selected

#### Scenario: Card refresh throttled during a shift

- **GIVEN** the AppUser is on shift and the card is visible
- **WHEN** more than one ping is enqueued within a 60-second window
- **THEN** the card SHALL NOT issue more than one network refresh in that window

### Requirement: Personal trajectory SHALL be readable independent of the Org's current location_tracking_enabled toggle

A persisted ping continues to belong to the AppUser who produced it. The AppUser SHALL be able to read their own pings via `GET /app/checkin/me/locations` regardless of the current value of `Org.settings.checkin.location_tracking_enabled`. The toggle gates *new* ingest (`POST /app/checkin/locations`) but does not retroactively hide already-recorded pings from their owner.

#### Scenario: Toggle disabled does not block self-read

- **GIVEN** an AppUser whose Org has `location_tracking_enabled = false` after previously having pings ingested while it was `true`
- **WHEN** the AppUser calls `GET /app/checkin/me/locations` for a date with prior pings
- **THEN** the response is `200` with those pings
- **AND** the `/trajectory` screen renders them normally

### Requirement: Clock-in consent dialog SHALL lead with the personal-log framing

The Flutter consent dialog presented before location tracking begins SHALL lead its body text with the personal benefit ("you will be able to review your own work-day movement inside the app") before any reference to org-side records or admin dashboards. The dialog SHALL also state when tracking starts (after pressing 上班), how it is visually indicated (iOS blue bar / Android sticky notification), and how it stops (pressing 下班).

#### Scenario: Consent body leads with personal-log framing

- **WHEN** the consent dialog is shown
- **THEN** the first sentence of the body text references the in-app personal log ("我的工作日記" or equivalent) as the primary use of the data
- **AND** the dialog also mentions the iOS blue indicator / Android sticky notification, and how to stop tracking
