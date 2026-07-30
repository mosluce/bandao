## Context

Two renderers draw the same picture from the same two sources, independently:

- `admin-web/pages/checkin/[appUserId]/trajectory.vue`
- `app/lib/features/trajectory/presentation/trajectory_screen.dart`

Both follow an identical shape today: fetch the day's pings and the day's events → sort pings ascending by `occurred_at_client` → emit one short polyline per consecutive ping pair coloured by that segment's midpoint time → overlay event markers → `fitBounds` over pings + markers. The polyline's vertex list is pings only; events contribute markers and nothing else.

The precedent for cross-platform agreement already exists: `app-personal-trajectory` defines the "Time-of-day trajectory color scale" as a normative contract (anchors + interpolation), and `admin-trajectory-dashboard` references it by name rather than restating it. This change follows the same pattern for the merge rule.

Relevant constraints discovered while scoping:

- `CheckinEventDto` already exposes `source` (`api/src/handlers/checkin_dto.rs:59`) and a non-optional `location` — no API change is needed to filter by origin.
- Both `CheckinEventDto.id` and `LocationPingDto.id` are ObjectId hex strings, so they are available on both platforms as a stable final tie-break.
- `EventsCursorQuery` (`api/src/handlers/checkin_dto.rs:229`) carries only `before` and `limit`. There is no range filter on either event history endpoint.
- `validate_range` is currently a private helper in `api/src/handlers/location_tracking.rs:391`, used by the ping list and export paths.

## Goals / Non-Goals

**Goals:**

- The polyline passes through every genuine checkin coordinate, closing the head, tail, and transfer gaps described in the proposal.
- One normative merge rule, implemented twice, producing byte-identical vertex ordering on both platforms.
- Retroactive: fixes days already on file, including `legacy_backfill`-imported records, with no data migration.
- Summary stats describe the series that is actually plotted.

**Non-Goals:**

- Writing derived `location_pings` rows (server-side or client-side). Rejected below.
- Changing the xlsx export. It stays a ping-only dump.
- Reverse-geocoding, deduplication, or smoothing of the merged series.
- Changing when the location tracker starts or stops. The head and tail gaps are closed at render time; the underlying sampling behaviour is untouched.
- Backfilling or repairing legacy coordinate quality.

## Decisions

### Merge on read rather than materialise derived pings

Three shapes were considered:

| | Client dual-write | Server-derived ping | **Read-side merge (chosen)** |
|---|---|---|---|
| App release required | Yes; old installs never fixed | No | Both clients change |
| Fixes existing days | No | Needs a backfill script | **Yes, inherently** |
| Fixes `legacy_backfill` days | No | Needs a backfill script | **Yes, inherently** |
| Failure modes | Two independent queues; a stray `403 LOCATION_TRACKING_DISABLED` deletes rows *and* stops the tracker | Single write, must sequence after the status update to stay rollback-safe | **No writes** |
| Couples to `clock_out` flush ordering | Yes — must enqueue before `flushFinal()` | No | No |
| Data duplication | Yes | Yes | **No** |
| Export includes checkin points | Yes | Yes | No |

Read-side merge wins on the axis that motivated the request: the existing days, including the customer's imported history, are fixed by deploying a renderer rather than by running a script over production data. The cost is that the rule lives in two codebases and that the export keeps its old shape.

Note that the client dual-write option would also have needed the org toggle consulted client-side, because enqueuing a ping while `location_tracking_enabled = false` drives the ingest endpoint's `403` path, which deletes in-flight rows and signals the tracker to stop. Read-side merge sidesteps the toggle entirely — it reads coordinates the system already stores.

### Exclude `source = admin_force`, include `legacy_backfill`

Force-checkout inserts a `clock_out` whose `location` is **copied from the AppUser's last event** (`checkin-events`, "Admin can force checkout an AppUser on shift"). That coordinate is a placeholder, not a position. Merging it draws a leg from wherever the worker actually was back to a stale point — a fabricated movement, and the one case where merging makes the picture actively wrong rather than merely coarse.

`legacy_backfill` events carry the legacy document's real `geo.lat` / `geo.lng`, so they are genuine positions and are merged. (The legacy `路徑` action already routes to `location_pings` separately; that is a different record type and unaffected.)

Filtering is by `source`, not by `initiated_by_kind`, because `source` is the field that distinguishes a captured coordinate from a copied one.

### Total ordering, compared as instants

The merge key is a three-level total order:

1. `occurred_at_client` **as a parsed instant**, ascending.
2. Origin rank: event (`0`) before ping (`1`).
3. `id` ascending, lexicographic.

Levels 2 and 3 exist so the order is total. Without them, a clock-in and a ping sharing a millisecond would order differently on the two platforms and the drawn lines would diverge — exactly the class of drift the color-scale contract was written to prevent.

Instant comparison is normative, not incidental. Both endpoints currently serialise `occurred_at_client` via bson's `try_to_rfc3339_string()`, so both emit UTC `Z` today and a lexical compare would happen to work — but `admin-trajectory-dashboard` already documents that offset representations differ across comparison sites (`Z` vs `+08:00`), and the current `pings.sort(localeCompare)` in `trajectory.vue:86` is only correct by that coincidence. The merge parses before comparing so the ordering does not depend on serialisation staying uniform across two endpoints.

No deduplication: a ping three seconds from a `clock_out` at nearly the same spot contributes a near-zero-length segment, which costs nothing and keeps the rule free of a distance or time threshold that both platforms would have to match.

### Range-scope the event fetch (API change)

Both pages fetch events with `limit: 100`, newest-first, then filter to the day in the client. At roughly four events per day that reaches back about 25 days; beyond that the request returns nothing for the requested date. The Flutter screen offers only today plus seven days so it is safe by accident, but admin-web has a free date picker, and every `legacy_backfill` day sits outside the window.

Since fixing imported history is the main reason to prefer this approach, the merge is worthless without a range-scoped fetch. So `from` / `to` are added to both event history endpoints, and the clients pass the day range instead of over-fetching and filtering.

Validation is deliberately identical to the ping list endpoint — parse failure, `to < from`, or a span over 90 days yields `INVALID_RANGE`; `from` being far in the past is not itself a rejection, because imported events predate any retention window. `validate_range` is lifted out of `location_tracking.rs` into a shared location so there is one rule rather than two that drift.

The parameters are optional and additive, so a client that omits them sees exactly today's behaviour. That keeps API and client deploys independent.

This also repairs a latent defect: event markers already disappear today on dates beyond the `limit: 100` reach, silently and with no error.

### Render thresholds keyed off merged point count

The existing rule — pings draw a line, events draw markers, events alone never draw a line — is replaced rather than extended:

| Merged points | Behaviour |
|---|---|
| `0` | `該日無軌跡資料`, map not instantiated |
| `1` | Map renders, marker(s) only, no line |
| `>= 2` | Map renders, line drawn |

`hasData` and the `clockInEvent` anchor computation in `trajectory.vue` collapse into this one count, as does the equivalent branch in `trajectory_screen.dart`. The clock-in event no longer needs a special role as "the thing that anchors the day" — it is simply the first merged point on a normal day.

The visible consequence, accepted per the proposal: a day with events and no pings now draws a line between event coordinates, so orgs running with `location_tracking_enabled = false` see markers joined up where they previously saw markers alone. No new data is collected — these are coordinates already stored on the events — and a segment bridging two events is no more interpolated than a segment bridging two pings half an hour apart.

### Stats computed over the merged series

`computeTrajectoryStats` takes merged points instead of pings:

- **走動距離** — geodesic sum over the merged series, so the head and tail legs count.
- **在班時長** — merged first→last span. On a normal day this is the clock-in→clock-out span, which is the precise figure the current implementation comment names as out of scope.
- **位置點** — merged point count, matching what is plotted. Reads 2–4 higher per day than before.

`today_summary_card.dart` shows 走動距離 and 在班時長 with the same labels as the trajectory screen, so it must consume the same merged series or the two surfaces will disagree on screen.

**Correction found during implementation**: this needs no extra fetch. The card already watches `trajectoryProvider` — the same provider the trajectory screen renders from — so it picks up the merged stats structurally, and the two surfaces cannot drift apart by construction. An earlier draft of this design assumed the card would issue its own events request and reasoned about fitting that inside the 60-second throttle; that was wrong, and no such request exists.

There is also a rename: `TrajectoryStats.pingCount` becomes `pointCount`, since the field no longer counts pings.

## Risks / Trade-offs

- **Two implementations drift** → The merge rule is specified normatively (source filter, three-level ordering, instant comparison, thresholds) in `app-personal-trajectory` and referenced by `admin-trajectory-dashboard`, following the color-scale precedent. Both sides get tests over the same fixture: mixed pings and events, an equal-timestamp tie, and an `admin_force` event that must be excluded.

- **`位置點` jumps for the same historical day** → Unavoidable and intended: the old number never matched the plotted points. No user-facing migration note is planned; flag it if anyone reconciles day-over-day.

- **The xlsx export and the map now disagree** → Export stays ping-only, so a day's exported rows are fewer than the plotted points and the exported distance, if anyone recomputes it, is short by the head and tail legs. Left out of scope rather than solved silently; called out in the proposal's Impact.

- **Legacy coordinate quality warps the line** → A `legacy_backfill` event with `geo: {lat: 0, lng: 0}` (device with no fix) would become a vertex, dragging the line to the Gulf of Guinea. Merging widens the blast radius from `fitBounds` alone (which already includes marker coordinates) to the line's shape. **Checked against KLCC production: zero such rows**, so this is not mitigated by design rather than by oversight — see the resolved Open Question. The exposure remains for any *future* import of unfixed coordinates.

- **Range-scoped fetch changes an endpoint two other surfaces use** → `from` / `to` are optional and default to current behaviour, and the history screens pass neither, so their cursor pagination is untouched.

## Migration Plan

1. Ship the API change first. `from` / `to` are optional; no client sends them yet; behaviour is unchanged.
2. Ship admin-web and the Flutter app in either order. Each starts passing the day range and merging as it deploys; a client on the old build keeps rendering the old picture correctly.
3. No data migration, no index, no backfill script. Rollback is redeploying the previous client build — there is nothing written to undo.

## Open Questions

- ~~**Do KLCC's imported events contain `(0, 0)` or otherwise unfixed coordinates?**~~ **Resolved 2026-07-30: none found.** `countDocuments({ source: "legacy_backfill", lat: 0, lng: 0 })` against KLCC production returned `0`. The merge therefore ships with **no coordinate validation**, which is now an evidence-backed decision rather than an omission: adding a `(0, 0)` filter would have been inventing a rule for data that does not exist. If a future import introduces unfixed coordinates, the mitigation is the one sketched above — drop exactly-`(0, 0)` points in `buildMergedSeries` on both platforms and add the rule to the merge contract requirement.
- **Should the range-scoped fetch replace the history screens' cursor pagination too?** Out of scope here — those screens page backwards through all events and have no date to scope to — but if the endpoints grow range support, a future change could give the history screens a date filter for free.
