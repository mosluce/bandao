## MODIFIED Requirements

### Requirement: Trajectory page renders one AppUser's daily polyline + event markers

The admin-web SHALL provide a route `/checkin/:appUserId/trajectory` accepting an optional `?date=YYYY-MM-DD` query parameter. When `date` is absent the page SHALL default to the current calendar date in the active Org's timezone. The page SHALL convert the `date` parameter to an RFC3339 range covering that calendar day in the active Org's timezone (`from = <date>T00:00:00<tz_offset>`, `to = <next date>T00:00:00<tz_offset>`) before requesting data.

The page SHALL fetch, for the resolved date range:

1. Pings via `GET /checkin/users/:id/locations?from=&to=`
2. Events via `GET /checkin/users/:id/events?from=&to=` using the same range

The event fetch SHALL be range-scoped server-side. The page SHALL NOT request a newest-first page and filter it client-side: that approach silently returns zero events for any date beyond the page's reach (roughly 25 days at four events per day), which drops both the markers and — now that events contribute vertices — the line's endpoints, and it fails for every `legacy_backfill`-imported day.

The page SHALL build the day's **merged series** per the Trajectory point merge contract defined in `app-personal-trajectory` (eligible points, `admin_force` exclusion, three-level instant-based ordering, no deduplication). Rendering SHALL be keyed off the merged point count:

| Merged points | Behaviour |
|---|---|
| `0` | Clear any previously drawn layers and display the empty overlay `該日無軌跡資料`; SHALL NOT instantiate Leaflet if it has not already been instantiated |
| `1` | Render the map and the event markers; SHALL NOT draw a line |
| `>= 2` | Render the map, the event markers, and the line |

The map container element SHALL remain mounted for the page's lifetime. The loading, error, and empty states SHALL be rendered as overlays positioned over that container, and SHALL NOT unmount or replace it. Unmounting the container while a Leaflet instance is bound to it orphans that instance against a detached DOM node: subsequent draws land on the detached node while the freshly created container stays empty, which is what made a date change blank the map until a page reload.

Drawing SHALL be a function of the current merged series and event list, not of a transition in whether the day has data. Whenever either changes the page SHALL, when the merged point count is non-zero, ensure a Leaflet instance exists for the mounted container and redraw the path, markers, and bounds. A date change between two days that both have data SHALL redraw exactly as a change from a day without data to a day with data does.

Leaflet SHALL be instantiated lazily, on the first render that has a non-zero merged point count. Once instantiated the map SHALL be retained until the page unmounts; a later date with zero merged points SHALL clear the drawn layers rather than destroy the map.

When the map renders it SHALL use Leaflet with CartoDB Positron tiles; draw the path through the **merged series** in its contract order, **colored by time of day using the shared time-of-day color scale** (defined in `app-personal-trajectory`: domain `06:00`→`22:00` clamped, anchors `#ea580c / #e11d48 / #c026d3 / #7c3aed / #4338ca`, interpolated) — since Leaflet has no native gradient polyline, the path SHALL be drawn as consecutive per-segment polylines each colored by that segment's midpoint time; overlay markers at each event's coordinates (distinct visual style per event type, kept visually distinct from the path colors), including events excluded from the merged series; display the required `© OpenStreetMap contributors © CARTO` attribution; render a **legend** mapping color to time (`6:00 / 12:00 / 18:00 / 22:00`); and auto-fit the map bounds to encompass all merged points and event markers. Fit-bounds SHALL run on every render, including after a date change, so the viewport always frames the displayed day rather than retaining the previous day's framing.

The legend SHALL be displayed only when the day has a non-zero merged point count and no error is showing, so that it never floats over an empty or errored map surface.

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

#### Scenario: Zero merged points shows the empty overlay without initializing Leaflet

- **WHEN** the merged point count is `0` for the date and Leaflet has not yet been instantiated
- **THEN** the page shows the `該日無軌跡資料` overlay
- **AND** does not initialize Leaflet
- **AND** does not render the legend

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

#### Scenario: Switching between two days that both have data redraws the map

- **GIVEN** the trajectory page is showing a date whose merged point count is non-zero
- **WHEN** the admin selects another date whose merged point count is also non-zero
- **THEN** the map displays the newly selected day's path and markers without a page reload
- **AND** the viewport is re-fit to the newly selected day's coordinates

#### Scenario: Stepping through an empty day and back keeps the map working

- **GIVEN** the trajectory page is showing a date with data
- **WHEN** the admin selects a date with zero merged points
- **THEN** the previously drawn path and markers are cleared and the empty overlay is shown
- **WHEN** the admin then selects another date with data
- **THEN** that day's path and markers render

#### Scenario: Loading state overlays the map instead of unmounting it

- **WHEN** a date change starts a fetch
- **THEN** a loading indicator is shown over the map container
- **AND** the map container remains mounted throughout the fetch

#### Scenario: Recovering from a failed fetch

- **GIVEN** a fetch for a date failed and the error overlay is displayed
- **WHEN** the admin selects a date whose fetch succeeds with a non-zero merged point count
- **THEN** the error overlay is dismissed and that day's path and markers render
