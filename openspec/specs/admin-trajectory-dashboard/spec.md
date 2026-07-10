# admin-trajectory-dashboard Specification

## Purpose
TBD - created by archiving change add-location-tracking-dashboard. Update Purpose after archive.
## Requirements
### Requirement: Trajectory page renders one AppUser's daily polyline + event markers

The admin-web SHALL provide a route `/checkin/:appUserId/trajectory` accepting an optional `?date=YYYY-MM-DD` query parameter. When `date` is absent the page SHALL default to the current calendar date in the active Org's timezone. The page SHALL convert the `date` parameter to an RFC3339 range covering that calendar day in the active Org's timezone (`from = <date>T00:00:00<tz_offset>`, `to = <next date>T00:00:00<tz_offset>`) before requesting pings.

The page SHALL fetch:
1. Pings via `GET /checkin/users/:id/locations?from=&to=` for the resolved date range
2. Events via the existing events list endpoint, filtering client-side to the same calendar day by comparing instants (not raw string comparison — `occurred_at_client` and the range bounds may use different offset representations, e.g. UTC `Z` vs `+08:00`, so a lexical compare can drop early-morning events)

When pings are present the page SHALL render a Leaflet map with CartoDB Positron tiles; draw a polyline through pings ordered by `occurred_at_client` ascending, **colored by time of day using the shared time-of-day color scale** (defined in `app-personal-trajectory`: domain `06:00`→`22:00` clamped, anchors `#ea580c / #e11d48 / #c026d3 / #7c3aed / #4338ca`, interpolated) — since Leaflet has no native gradient polyline, the path SHALL be drawn as consecutive per-segment polylines each colored by that segment's midpoint time; overlay markers at each event's coordinates (distinct visual style per event type, kept visually distinct from the path colors); display the required `© OpenStreetMap contributors © CARTO` attribution; render a **legend** mapping color to time (`6:00 / 12:00 / 18:00 / 22:00`); and auto-fit the map bounds to encompass all polyline points and event markers.

The day's start SHALL be anchored to the first `clock_in` event: its event marker (event-type colored, not the time scale) marks where the day began, and the page SHALL render the map whenever there is a `clock_in` event with a location, even with zero pings (only the event marker, no line). When there are neither pings nor a `clock_in` event, the page SHALL render the text `該日無軌跡資料` and SHALL NOT instantiate a map.

The `?date=` URL parameter and the date input SHALL stay in sync — selecting a new date in the picker SHALL update the URL and trigger a refetch.

#### Scenario: Default date is today in Org timezone

- **WHEN** an admin navigates to `/checkin/:appUserId/trajectory` with no `?date=`
- **THEN** the page resolves the date to today in the Org timezone and fetches pings for that range

#### Scenario: Date param drives the fetch range

- **WHEN** the URL is `/checkin/:appUserId/trajectory?date=2026-03-01` and Org timezone is `Asia/Taipei` (+08:00)
- **THEN** the page issues `GET /checkin/users/:id/locations?from=2026-03-01T00:00:00+08:00&to=2026-03-02T00:00:00+08:00`

#### Scenario: Path is colored by time of day with a legend

- **WHEN** the day's pings span morning to evening
- **THEN** the path segments transition from the warm (`06:00`) end toward the cool (`22:00`) end following each segment's midpoint time
- **AND** a legend shows the color→time mapping
- **AND** event-type markers remain visually distinct from the path colors

#### Scenario: Clock-in with no pings still renders the map

- **WHEN** the API returns a `clock_in` event with a location but zero pings for the date range
- **THEN** the map renders with the clock-in event marker (anchoring the start) and no path line

#### Scenario: Neither pings nor clock-in hides the map

- **WHEN** the API returns zero pings and no `clock_in` event for the date range
- **THEN** the page shows `該日無軌跡資料` text
- **AND** does not initialize Leaflet

#### Scenario: Polyline ordered chronologically

- **WHEN** the API returns pings out of order (newest-first per the API contract)
- **THEN** the page sorts ascending by `occurred_at_client` before drawing and coloring

#### Scenario: Auto fit-bounds on render

- **WHEN** pings and event markers are rendered
- **THEN** the map's viewport encompasses every plotted coordinate

#### Scenario: Date picker round-trips through URL

- **WHEN** the admin picks a different date in the input
- **THEN** the URL `?date=` is updated to the new value
- **AND** the fetch reruns for the new range

### Requirement: Org settings page exposes location_tracking_enabled toggle

The admin-web Org settings UI on `/` SHALL include a toggle for `location_tracking_enabled` immediately following the existing `transfer_enabled` toggle. The toggle SHALL display the current value from `auth.currentOrg.value.checkin.location_tracking_enabled` and SHALL submit its inverse via `PATCH /orgs/me/settings` when changed. While the request is in flight the toggle SHALL be disabled. When the server responds with `STATE_LOCKED` the UI SHALL display a localized error: `目前有 App 使用者在班，需先全部下班才能調整此設定`.

#### Scenario: Toggle reflects current Org setting

- **WHEN** an admin lands on `/` and `Org.checkin.location_tracking_enabled` is true
- **THEN** the toggle is rendered checked

#### Scenario: Successful toggle update

- **WHEN** an admin clicks the toggle from on to off
- **AND** no AppUser is on shift
- **THEN** the page sends `PATCH /orgs/me/settings { "location_tracking_enabled": false }`
- **AND** the toggle reflects the new value on success

#### Scenario: STATE_LOCKED shows localized error

- **WHEN** an admin clicks the toggle while at least one AppUser is on shift
- **THEN** the API returns `STATE_LOCKED`
- **AND** the page displays `目前有 App 使用者在班，需先全部下班才能調整此設定`

### Requirement: Trajectory page provides xlsx export entry point

The trajectory page SHALL include an export action that opens a date-range selector. After the admin picks `from` and `to` and confirms, the page SHALL trigger a browser download by navigating to `GET /checkin/users/:id/locations/export?from=&to=` (cookie auth carries via the same-origin / SameSite=Lax navigation). The page SHALL pre-validate the range client-side: rejecting empty values, `to < from`, and span > 90 days with localized inline messages so the most common errors do not require a server round-trip.

#### Scenario: Valid export triggers download

- **WHEN** an admin enters a valid `from` / `to` range and confirms
- **THEN** the browser navigates to the export URL with the cookie session
- **AND** the response downloads as `bandao-locations-<username>-<from>-<to>.xlsx`

#### Scenario: Span > 90 days rejected client-side

- **WHEN** the admin selects a `from` / `to` range exceeding 90 days
- **THEN** the page shows an inline error and does NOT issue the export request

#### Scenario: to before from rejected client-side

- **WHEN** the admin selects `to` earlier than `from`
- **THEN** the page shows an inline error and does NOT issue the export request
