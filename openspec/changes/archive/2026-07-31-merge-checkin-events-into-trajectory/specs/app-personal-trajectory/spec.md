## ADDED Requirements

### Requirement: Trajectory point merge contract

The system SHALL define a single rule for building the ordered vertex list of a trajectory polyline from one day's `location_pings` and `checkin_events`, shared by the app and admin-web so both render an identical line. This contract sits beside the Time-of-day trajectory color scale and is referenced by `admin-trajectory-dashboard` rather than restated there.

**Eligible points.** The merged series SHALL contain:

- Every `LocationPingDto` returned for the day, contributing `lat` / `lng` / `occurred_at_client`.
- Every `CheckinEventDto` returned for the day whose `source` is NOT `admin_force`, contributing `location.coordinates.lat` / `location.coordinates.lng` / `occurred_at_client`.

Events with `source = admin_force` SHALL be excluded from the merged series. Force-checkout copies its `location` from the AppUser's previous event rather than capturing a position, so including it would draw a leg from the worker's actual last position back to a stale coordinate. Events with `source = app` and `source = legacy_backfill` both carry genuine captured coordinates and SHALL be included.

Excluding an event from the merged series SHALL NOT exclude it from the event markers — an `admin_force` `clock_out` SHALL still render its marker.

**Ordering.** The merged series SHALL be sorted by a three-level total order:

1. `occurred_at_client` ascending, compared as parsed instants — NOT as raw strings. The two endpoints are independent serialisation sites and MUST NOT be assumed to emit the same UTC offset representation.
2. Origin rank ascending, where an event ranks `0` and a ping ranks `1`. An event and a ping at the same instant therefore place the event first.
3. `id` ascending, lexicographic.

Levels 2 and 3 make the order total so that both platforms produce the same vertex sequence for the same input.

**No deduplication.** The merge SHALL NOT drop, collapse, or smooth points that are close in time or space. A ping seconds away from an event at nearly the same coordinate SHALL contribute its own vertex.

**Colouring.** Segment colouring SHALL be unchanged: each segment between consecutive merged points takes the Time-of-day trajectory color scale applied to that segment's midpoint time. Merged points are coloured by the same rule regardless of whether they originated as a ping or an event.

#### Scenario: Pings and events interleave by time

- **GIVEN** the day has pings at `08:04`, `08:12`, `12:33`, `17:52` and events `clock_in 08:00`, `transfer_out 12:30`, `clock_out 17:55`
- **WHEN** the merged series is built
- **THEN** its vertices are ordered `08:00, 08:04, 08:12, 12:30, 12:33, 17:52, 17:55`
- **AND** the first vertex is the `clock_in` coordinate and the last is the `clock_out` coordinate

#### Scenario: admin_force clock_out excluded from the line but not the markers

- **GIVEN** the day's last event is a `clock_out` with `source = admin_force` whose location was copied from an earlier event
- **WHEN** the merged series is built
- **THEN** that event contributes no vertex to the merged series
- **AND** the line does not run back to the copied coordinate
- **AND** a `clock_out` event marker is still drawn at that coordinate

#### Scenario: legacy_backfill events are included

- **GIVEN** the day's events were imported with `source = legacy_backfill` and carry coordinates from the legacy records
- **WHEN** the merged series is built
- **THEN** each such event contributes a vertex

#### Scenario: Equal timestamps break toward the event

- **GIVEN** a `clock_in` event and a ping share the same `occurred_at_client` instant
- **WHEN** the merged series is built
- **THEN** the event's vertex precedes the ping's vertex

#### Scenario: Ordering does not depend on offset representation

- **GIVEN** the pings endpoint returns `occurred_at_client` as UTC `Z` and the events endpoint returns the same instants with a `+08:00` offset
- **WHEN** the merged series is built
- **THEN** the ordering is by instant and matches the ordering that identical representations would produce

#### Scenario: Near-coincident points are both kept

- **GIVEN** a ping three seconds before a `clock_out` event at a coordinate two metres away
- **WHEN** the merged series is built
- **THEN** both contribute vertices
- **AND** the resulting near-zero-length segment is drawn

## MODIFIED Requirements

### Requirement: Trajectory screen SHALL render the AppUser's own daily polyline with summary stats

The `/trajectory` screen SHALL fetch the caller's own pings for the active date via `GET /app/checkin/me/locations?from=&to=` (range = one calendar day in the Org's timezone) and the caller's own events for the same day via `GET /app/checkin/events?from=&to=` using the same range. The screen SHALL NOT over-fetch events and filter them client-side — the range is passed to the server so days beyond the default page's reach still return their events.

The screen SHALL build the day's **merged series** per the Trajectory point merge contract. Rendering SHALL be keyed off the merged point count:

| Merged points | Behaviour |
|---|---|
| `0` | Render the text `該日無軌跡資料`; SHALL NOT instantiate the map |
| `1` | Render the map and the event markers; SHALL NOT draw a line |
| `>= 2` | Render the map, the event markers, and the line |

When the map renders, the screen SHALL render:

- A `flutter_map` map view with OSM/CARTO Positron tiles (matching admin-web's tile choice) and the required `© OpenStreetMap contributors © CARTO` attribution string.
- A polyline drawn through the **merged series** in its contract order, **colored per point by the Time-of-day trajectory color scale** applied to each point's local `occurred_at_client`, interpolated along the line (drawn as consecutive per-segment polylines since a single stroke cannot follow a winding path with a per-point color).
- **Event markers** at each of the day's check-in events (clock in/out, transfer in/out), styled by event type (visually distinct from the time-colored path). Markers SHALL be drawn for every event returned for the day, including events excluded from the merged series.
- A **legend** mapping color to time (a horizontal gradient bar labeled at `6:00 / 12:00 / 18:00 / 22:00`) overlaid on the map.
- Auto-fit map bounds to encompass all rendered points (merged series + event markers) on initial load.
- Three summary stats below the map, all computed over the **merged series** rather than pings alone: **走動距離** (geodesic sum, km to one decimal), **在班時長** (first→last merged point elapsed, `H 小時 M 分`), **位置點** (integer merged point count).

Because the merged series includes the day's checkin coordinates, a day with events and zero pings SHALL draw a line between those event coordinates once there are two or more of them. This supersedes the previous rule that events rendered markers but never a line.

#### Scenario: Path is colored by time of day

- **WHEN** the day's merged points span morning to evening
- **THEN** the polyline transitions from the warm (`06:00`) end of the scale toward the cool (`22:00`) end following each point's local time
- **AND** a legend shows the color→time mapping

#### Scenario: Line passes through the clock-in and clock-out coordinates

- **GIVEN** the day has a `clock_in`, several pings, and a `clock_out`, all with coordinates
- **WHEN** the screen renders
- **THEN** the polyline's first vertex is the `clock_in` coordinate and its last vertex is the `clock_out` coordinate
- **AND** the `clock_in` and `clock_out` markers sit on the line rather than off it

#### Scenario: Event markers are drawn per type

- **WHEN** the day has check-in events (clock in/out, transfer in/out) with locations
- **THEN** a marker is drawn at each event, styled by event type

#### Scenario: Two events and no pings now draw a line

- **WHEN** the server returns a `clock_in` and a `clock_out` for the date and zero pings
- **THEN** the merged point count is `2`
- **AND** the map renders with both markers AND a line between the two coordinates

#### Scenario: A single merged point renders the map without a line

- **WHEN** the server returns exactly one eligible point for the date (for example a `clock_in` and no pings)
- **THEN** the map renders with the event marker
- **AND** no polyline is drawn

#### Scenario: Zero merged points shows text, no map

- **WHEN** the merged point count is `0` for the date
- **THEN** the screen shows the text `該日無軌跡資料`
- **AND** no map widget is instantiated

#### Scenario: Polyline ordered by the merge contract

- **WHEN** the server returns pings newest-first (per the API contract) and events newest-first
- **THEN** the client builds the merged series in contract order before drawing and coloring the polyline

#### Scenario: Auto fit bounds on initial render

- **WHEN** the map renders for the first time
- **THEN** the viewport encompasses every plotted coordinate including the event markers

#### Scenario: Stats include the head and tail legs

- **GIVEN** the day's first ping is 600 metres from the `clock_in` coordinate and the last ping is 200 metres from the `clock_out` coordinate
- **WHEN** the stats are computed
- **THEN** 走動距離 includes both legs
- **AND** 在班時長 is the `clock_in`→`clock_out` elapsed span
- **AND** 位置點 equals the merged point count, not the ping count

### Requirement: Home screen SHALL show a dynamic "我的今天" summary card

The home screen SHALL render a "我的今天" card showing the AppUser's distance walked and elapsed on-shift duration for the current day, computed over the **merged series** for today per the Trajectory point merge contract. The card and the `/trajectory` screen present these figures under identical labels, so they SHALL read the same merged series from a single shared source rather than each assembling their own — agreement SHALL be structural, not maintained by convention. The card SHALL be visible whenever the AppUser has at least one merged point for today OR is currently on shift; it SHALL NOT show on a no-data, off-shift day.

Tapping the card SHALL route the user to `/trajectory` with today selected.

During an active shift, the displayed stats SHALL refresh at most once per 60 seconds (matching the existing ping enqueue throttle); a refresh trigger SHALL also fire on app foreground.

#### Scenario: Card visible on a day with merged points

- **GIVEN** the AppUser has at least one merged point for today
- **WHEN** the user opens `/home`
- **THEN** the "我的今天" card is rendered
- **AND** it shows 走動距離 and 在班時長 values computed from today's merged series

#### Scenario: Card agrees with the trajectory screen

- **GIVEN** the AppUser has pings and checkin events for today
- **WHEN** the user compares the card's 走動距離 and 在班時長 with the `/trajectory` screen's values for today
- **THEN** the values are equal
- **AND** they are equal because both read one shared merged series, not because two computations happen to match

#### Scenario: Card visible on a clock-in-only day

- **GIVEN** the AppUser clocked in today and has zero pings so far
- **WHEN** the user opens `/home`
- **THEN** the "我的今天" card is rendered (the `clock_in` is a merged point)

#### Scenario: Card hidden on a no-data off-shift day

- **GIVEN** the AppUser is off shift and has zero merged points for today
- **WHEN** the user opens `/home`
- **THEN** the "我的今天" card is not rendered

#### Scenario: Card tap routes to trajectory

- **WHEN** the user taps the "我的今天" card
- **THEN** the app routes to `/trajectory` with today selected

#### Scenario: Card refresh throttled during a shift

- **GIVEN** the AppUser is on shift and the card is visible
- **WHEN** more than one ping is enqueued within a 60-second window
- **THEN** the card SHALL NOT issue more than one network refresh in that window
