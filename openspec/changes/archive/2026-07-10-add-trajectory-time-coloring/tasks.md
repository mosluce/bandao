## 1. Shared time-of-day color scale (contract)

- [x] 1.1 Define the scale once as the source of truth (in the spec + mirrored in code): domain `06:00`→`22:00` clamped, 5 anchors `#ea580c / #e11d48 / #c026d3 / #7c3aed / #4338ca`, linear interpolation between anchors. Document the interpolation space (RGB vs HSL) so both platforms match

## 2. app (Flutter)

- [x] 2.1 Add a `timeOfDayColor(DateTime local)` util (returns `Color` from the scale); unit-test the 5 anchors + a clamp case + a midpoint interpolation
- [x] 2.2 `trajectory_controller` / `TrajectoryDayState`: also fetch the active day's events via `GET /app/checkin/events` and expose the first `clock_in` event's location (+ time) as a start anchor
- [x] 2.3 `trajectory_screen.dart`: colour the `Polyline` per-point via `gradientColors` mapped from each ping's `occurred_at_client`. **Verify the installed flutter_map `gradientColors` interpolates per-vertex; if it distributes by length, fall back to segmented Polylines**
- [x] 2.4 Draw event markers (clock in/out, transfer in/out) styled by event type (matches admin colors); the clock-in marker anchors the start. Controller keeps the day's events in state (`GET /app/checkin/events`, instant-filtered to the day). Replaces the earlier time-colored start/end dots
- [x] 2.5 Relax the empty state: `0 pings` **with** any check-in event → still build the map and draw the event markers (no line); `0 pings` **and** no events → keep `該日無軌跡資料`
- [x] 2.6 Add the "color → time" legend (horizontal gradient bar labelled 6:00 / 12:00 / 18:00 / 22:00) overlaid on the map

## 3. admin-web (Nuxt / Leaflet)

- [x] 3.1 Add the matching `timeOfDayColor(date)` util (same anchors/domain/interp); unit-test the anchors
- [x] 3.2 Replace the single `L.polyline` with per-segment polylines coloured by each segment's midpoint time; consider `preferCanvas` for a day's worth of segments
- [x] 3.3 Keep the event-type markers (clock-in marker anchors the start); fix the event day-filter to compare instants (not strings) so early-morning events near the day boundary are not dropped
- [x] 3.4 Add the "color → time" legend on the map

## 4. Spec, verification & viz check

- [x] 4.1 Update `app-personal-trajectory` + `admin-trajectory-dashboard` spec deltas: polyline is time-of-day coloured (define the shared scale), legend present, start anchored to clock-in with the relaxed empty state
- [x] 4.2 dataviz validator on the final anchors (`--mode light`) → ALL CHECKS PASS (`#ea580c,#e11d48,#c026d3,#7c3aed,#4338ca`). Screenshot eyeball of the live maps folded into the manual smoke (4.4)
- [x] 4.3 `flutter analyze` + `flutter test` clean; admin-web `nuxt typecheck` + `pnpm test` + build clean
- [x] 4.4 **DONE (visual smoke, both surfaces)**: admin-web + iOS-sim app both render the warm→cool gradient matching the legend + event-type markers (clock-in green/start, transfer amber, clock-out slate); verified on seeded 2026-07-10 data. Original wording: a day with a clock-in but no pings shows the map with just the start (app: colored start dot; admin: clock-in event marker); a full day shows the warm→cool gradient matching the legend on the light basemap
