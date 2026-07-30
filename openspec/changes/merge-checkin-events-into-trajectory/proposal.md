## Why

The trajectory polyline is drawn exclusively from `location_pings`, while checkin events (clock in/out, transfer in/out) are drawn only as standalone markers. The two coordinate sets never intersect, so every event marker floats off the path by construction — not a bug, a consequence of the data model.

The gap is structural and lands in three fixed places:

- **Head**: the location tracker only starts after the server confirms `on_site`, and the first ping additionally requires the `100m AND 60s` sampling filter to clear. The path therefore begins some distance and minutes after the clock-in point. Offline clock-in is far worse — the event sits in the queue, the tracker never starts, and hours of the shift produce zero pings.
- **Tail**: tapping 下班 stops the tracker optimistically, before the `clock_out` event is confirmed. The path always ends short of the clock-out point.
- **Transfers**: the tracker keeps running, but the 100m/60s sampling never lands exactly on the transfer coordinate.

We fix this on the read side: both renderers already fetch pings *and* events for the day, so merging the event coordinates into the polyline's vertex list closes all three gaps at once — retroactively, for every day already on file, including the KLCC records imported by `legacy-checkin-backfill`. No new data is written and no new collection is introduced.

## What Changes

- Define a shared **trajectory point merge contract** (alongside the existing shared time-of-day color scale, in `app-personal-trajectory`): the polyline's vertices are `location_pings` plus the day's checkin events, ordered by `occurred_at_client`, with an explicit tie-break so both platforms draw an identical line.
- Events with `source = admin_force` are **excluded** from the merge. Force-checkout copies its location from the AppUser's *previous* event rather than capturing a real position, so merging it would draw a fabricated leg from the worker's actual last position back to a stale coordinate. `source = app` and `source = legacy_backfill` both carry genuine coordinates and are included.
- **BREAKING (spec-level)**: the "checkin events render markers but never a line" rule is replaced, not extended. Map rendering now keys off the merged point count — `>= 1` renders the map, `>= 2` draws the line, `0` shows `該日無軌跡資料`. A day with events and zero pings therefore now draws a line between the event coordinates. This is accepted deliberately: a segment bridging two event points is no more interpolated than a segment bridging two pings 30 minutes apart, and consistency beats special-casing. Orgs with `location_tracking_enabled = false` will consequently see their trajectory page change from isolated markers to markers joined by a line — derived entirely from event coordinates the system already stores, so no new data is collected.
- Add `from` / `to` range filters to the event history endpoints (`GET /checkin/users/:id/events` and `GET /app/checkin/events`), reusing the `INVALID_RANGE` validation already specified for `GET /checkin/users/:id/locations`. **This is required, not optional polish**: both trajectory pages currently fetch events with `limit: 100` newest-first and no range, which reaches back roughly 25 days at 4 events/day. Every legacy-imported day — the primary reason to prefer a read-side fix — falls outside that window and returns zero events, so the merge would silently do nothing there. It also repairs a latent defect: event markers already vanish today on dates beyond that window.
- Redefine the two trajectory stats that the merge makes more accurate, plus the point count:
  - **走動距離** is summed over the merged series, so the head and tail legs are finally included.
  - **在班時長** becomes the merged first→last span, which for a normal day is the clock-in→clock-out span — the precise answer that `trajectory_stats.dart` currently documents as out of scope.
  - **位置點** counts merged points rather than pings, so the number matches what is plotted. Same-day figures will read 2–4 higher than before this change.

## Capabilities

### New Capabilities

(none — all changes extend existing capabilities)

### Modified Capabilities

- `app-personal-trajectory`: gains the shared trajectory point merge contract (source filtering, ordering, tie-break); the polyline requirement switches its vertex source from pings to merged points; the events-with-no-pings scenario is rewritten around merged point count; the three summary stats are redefined over the merged series; the day's event fetch becomes range-scoped.
- `admin-trajectory-dashboard`: the polyline requirement switches its vertex source to merged points and references the merge contract; the clock-in-with-no-pings and neither-pings-nor-clock-in scenarios are rewritten around merged point count; the event fetch becomes range-scoped instead of client-side filtered.
- `checkin-events`: the AppUser history endpoint (`GET /app/checkin/events`) and the Org-member event history endpoint (`GET /checkin/users/:id/events`) accept optional `from` / `to` RFC3339 filters with `INVALID_RANGE` validation matching the location endpoints.

## Impact

- **API code (`api/`)**: `handlers/checkin_dto.rs` (`EventsCursorQuery` gains `from` / `to`), `handlers/checkin.rs` (`list_user_events`), `handlers/app_checkin.rs` (`list_events`), `db/checkin_events.rs` (paginated query gains range predicates). Range validation is lifted from the existing location-pings path so both share one rule. No schema change, no migration, no new index (`checkin_events` is already indexed on `(app_user_id, occurred_at_client)` for the cursor).
- **admin-web**: `pages/checkin/[appUserId]/trajectory.vue` — event fetch passes the day range, client-side day filtering is dropped, polyline is built from merged points, `hasData` is replaced by merged point count. `test/pages/trajectory.test.ts` covers merge ordering, `admin_force` exclusion, and the new empty-state thresholds.
- **app (`app/`)**: `features/trajectory/state/trajectory_controller.dart` (range-scoped event fetch, merge), `features/trajectory/data/trajectory_stats.dart` (all three stats over merged points), `features/trajectory/presentation/trajectory_screen.dart` (polyline vertices, empty state), `features/trajectory/presentation/today_summary_card.dart` (consumes the redefined stats). `core/api/models/checkin_event.dart` already exposes `source`, so no model regeneration is needed.
- **Not fixed by this change**: `GET /checkin/users/:id/locations/export` remains a ping-only dump — checkin coordinates will not appear in the xlsx. Anyone reconciling against the export will still see the old numbers.
- **No data written**: no derived pings, no backfill script, nothing to roll back beyond redeploying the previous clients.
