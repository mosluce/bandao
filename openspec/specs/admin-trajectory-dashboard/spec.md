# admin-trajectory-dashboard Specification

## Purpose
TBD - created by archiving change add-location-tracking-dashboard. Update Purpose after archive.
## Requirements
### Requirement: Trajectory page renders one AppUser's daily polyline + event markers

The admin-web SHALL provide a route `/checkin/:appUserId/trajectory` accepting an optional `?date=YYYY-MM-DD` query parameter. When `date` is absent the page SHALL default to the current calendar date in the active Org's timezone. The page SHALL convert the `date` parameter to an RFC3339 range covering that calendar day in the active Org's timezone (`from = <date>T00:00:00<tz_offset>`, `to = <next date>T00:00:00<tz_offset>`) before requesting data.

The page SHALL fetch, for the resolved date range:

1. Pings via `GET /checkin/users/:id/locations?from=&to=`
2. Events via `GET /checkin/users/:id/events?from=&to=` using the same range

The event fetch SHALL be range-scoped server-side. The page SHALL NOT request a newest-first page and filter it client-side: that approach silently returns zero events for any date beyond the page's reach (roughly 25 days at four events per day), which drops both the markers and — now that events contribute vertices — the line's endpoints, and it fails for every `legacy_backfill`-imported day.

The page SHALL build the day's **merged series** per the Trajectory point merge contract defined in `app-personal-trajectory` (eligible points, `admin_force` exclusion, three-level instant-based ordering, no deduplication). Rendering SHALL be keyed off the merged point count:

| Merged points | Behaviour |
|---|---|
| `0` | Render the text `該日無軌跡資料`; SHALL NOT instantiate Leaflet |
| `1` | Render the map and the event markers; SHALL NOT draw a line |
| `>= 2` | Render the map, the event markers, and the line |

When the map renders it SHALL use Leaflet with CartoDB Positron tiles; draw the path through the **merged series** in its contract order, **colored by time of day using the shared time-of-day color scale** (defined in `app-personal-trajectory`: domain `06:00`→`22:00` clamped, anchors `#ea580c / #e11d48 / #c026d3 / #7c3aed / #4338ca`, interpolated) — since Leaflet has no native gradient polyline, the path SHALL be drawn as consecutive per-segment polylines each colored by that segment's midpoint time; overlay markers at each event's coordinates (distinct visual style per event type, kept visually distinct from the path colors), including events excluded from the merged series; display the required `© OpenStreetMap contributors © CARTO` attribution; render a **legend** mapping color to time (`6:00 / 12:00 / 18:00 / 22:00`); and auto-fit the map bounds to encompass all merged points and event markers.

The day's start is the merged series' first vertex, which on a normal day is the first `clock_in` event. The page SHALL NOT maintain a separate "clock-in anchors the day" code path: with events contributing vertices, the clock-in is simply the first merged point. This supersedes the previous rules that a `clock_in` with zero pings rendered a marker but never a line, and that the map was gated on `pings.length > 0 || clockInEvent !== null`.

The `?date=` URL parameter and the date input SHALL stay in sync — selecting a new date in the picker SHALL update the URL and trigger a refetch of both pings and events.

#### Scenario: Default date is today in Org timezone

- **WHEN** an admin navigates to `/checkin/:appUserId/trajectory` with no `?date=`
- **THEN** the page resolves the date to today in the Org timezone and fetches pings and events for that range

#### Scenario: Date param drives both fetch ranges

- **WHEN** the URL is `/checkin/:appUserId/trajectory?date=2026-03-01` and Org timezone is `Asia/Taipei` (+08:00)
- **THEN** the page issues `GET /checkin/users/:id/locations?from=2026-03-01T00:00:00+08:00&to=2026-03-02T00:00:00+08:00`
- **AND** `GET /checkin/users/:id/events?from=2026-03-01T00:00:00+08:00&to=2026-03-02T00:00:00+08:00`

#### Scenario: A date far in the past still returns its events

- **GIVEN** an AppUser with `legacy_backfill` events from 200 days ago and more than 100 events since
- **WHEN** an admin opens the trajectory page for that date
- **THEN** the range-scoped event fetch returns that day's events
- **AND** the markers and the line's endpoints render

#### Scenario: Line passes through the clock-in and clock-out coordinates

- **GIVEN** the day has a `clock_in`, several pings, and a `clock_out`, all with coordinates
- **WHEN** the page renders
- **THEN** the path's first vertex is the `clock_in` coordinate and its last vertex is the `clock_out` coordinate
- **AND** those markers sit on the path rather than off it

#### Scenario: Path is colored by time of day with a legend

- **WHEN** the day's merged points span morning to evening
- **THEN** the path segments transition from the warm (`06:00`) end toward the cool (`22:00`) end following each segment's midpoint time
- **AND** a legend shows the color→time mapping
- **AND** event-type markers remain visually distinct from the path colors

#### Scenario: admin_force clock_out is not a vertex but is still a marker

- **GIVEN** the day's last event is a `clock_out` with `source = admin_force`
- **WHEN** the page renders
- **THEN** the path does not extend to that event's copied coordinate
- **AND** a `clock_out` marker is drawn there

#### Scenario: Clock-in plus clock-out with no pings now draws a line

- **WHEN** the API returns a `clock_in` and a `clock_out` with locations and zero pings for the date
- **THEN** the merged point count is `2`
- **AND** the map renders both markers AND a line between the two coordinates

#### Scenario: A single merged point renders the map without a line

- **WHEN** the API returns a `clock_in` with a location and zero pings for the date
- **THEN** the map renders with the clock-in marker
- **AND** no path line is drawn

#### Scenario: Zero merged points hides the map

- **WHEN** the merged point count is `0` for the date
- **THEN** the page shows `該日無軌跡資料` text
- **AND** does not initialize Leaflet

#### Scenario: Path ordered by the merge contract

- **WHEN** the API returns pings and events newest-first (per the API contract)
- **THEN** the page builds the merged series in contract order before drawing and coloring
- **AND** ordering compares parsed instants, not raw strings

#### Scenario: Auto fit-bounds on render

- **WHEN** merged points and event markers are rendered
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
