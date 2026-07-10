## ADDED Requirements

### Requirement: Time-of-day trajectory color scale

The system SHALL define a single time-of-day color scale used to color trajectory paths, shared by the app and admin-web so both render identically. The scale maps a local wall-clock time (in the Org's timezone) to a color: the domain is `06:00`→`22:00`, values outside are clamped to the nearest bound. It is a two-pole warm→cool ramp defined by five anchors, linearly interpolated between adjacent anchors:

| Local time | Color |
|------------|-------|
| `06:00` | `#ea580c` (warmest) |
| `10:00` | `#e11d48` |
| `14:00` | `#c026d3` |
| `18:00` | `#7c3aed` |
| `22:00` | `#4338ca` (coolest) |

The ramp SHALL pass through the red–purple side (never green/rainbow) and SHALL remain chromatic throughout (no low-chroma gray midpoint) so the path stays legible on the light CARTO Positron basemap. Both implementations SHALL produce the same color for the same time (the anchors and interpolation are the contract).

#### Scenario: Anchor times map to the defined colors

- **WHEN** a point's local time is exactly `06:00`, `14:00`, or `22:00`
- **THEN** its color is `#ea580c`, `#c026d3`, `#4338ca` respectively

#### Scenario: Times outside the domain clamp

- **WHEN** a point's local time is `05:30` or `23:15`
- **THEN** its color equals the `06:00` anchor or the `22:00` anchor respectively

## MODIFIED Requirements

### Requirement: Trajectory screen SHALL render the AppUser's own daily polyline with summary stats

The `/trajectory` screen SHALL fetch the caller's own pings for the active date via `GET /app/checkin/me/locations?from=&to=` (range = one calendar day in the Org's timezone) and the caller's own events for the same day via `GET /app/checkin/events`. When there is renderable location data the screen SHALL render:

- A `flutter_map` map view with OSM/CARTO Positron tiles (matching admin-web's tile choice) and the required `© OpenStreetMap contributors © CARTO` attribution string.
- A polyline drawn through pings ordered ascending by `occurred_at_client`, **colored per point by the Time-of-day trajectory color scale** applied to each ping's local `occurred_at_client`, interpolated along the line.
- A **start marker anchored to the day's first `clock_in` event location** (not the first ping), colored by that check-in's time via the scale.
- An end marker at the last ping.
- A **legend** mapping color to time (a horizontal gradient bar labeled at `6:00 / 12:00 / 18:00 / 22:00`) overlaid on the map.
- Auto-fit map bounds to encompass all rendered points (pings + the start anchor) on initial load.
- Three summary stats below the map: **走動距離** (geodesic sum, km to one decimal), **在班時長** (first→last ping elapsed, `H 小時 M 分`), **位置點** (integer ping count).

When there is a `clock_in` event for the date but zero pings, the screen SHALL still render the map and draw only the colored start marker (no line). When there are neither pings nor a `clock_in` event, the screen SHALL render the text `該日無軌跡資料` and SHALL NOT instantiate the map.

#### Scenario: Path is colored by time of day

- **WHEN** the day's pings span morning to evening
- **THEN** the polyline transitions from the warm (`06:00`) end of the scale toward the cool (`22:00`) end following each point's local time
- **AND** a legend shows the color→time mapping

#### Scenario: Start marker follows the clock-in, not the first ping

- **WHEN** the day has a `clock_in` event with a location
- **THEN** the start marker is drawn at the clock-in location, colored by the check-in time

#### Scenario: Clock-in with no pings still renders the start

- **WHEN** the server returns a `clock_in` event but zero pings for the date
- **THEN** the map renders with only the colored start marker and no polyline

#### Scenario: Neither pings nor clock-in shows text, no map

- **WHEN** the server returns zero pings and no `clock_in` event for the date
- **THEN** the screen shows the text `該日無軌跡資料`
- **AND** no map widget is instantiated

#### Scenario: Polyline ordered chronologically

- **WHEN** the server returns pings newest-first (per the API contract)
- **THEN** the client sorts ascending by `occurred_at_client` before drawing and coloring the polyline

#### Scenario: Auto fit bounds on initial render

- **WHEN** the map renders for the first time
- **THEN** the viewport encompasses every plotted coordinate including the start anchor
