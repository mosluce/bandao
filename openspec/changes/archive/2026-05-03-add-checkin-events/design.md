## Context

argus's identity layers are now complete: dashboard users have m:n memberships with org tenancy, and AppUsers are admin-managed mobile-end-user identities scoped 1:1 to an Org. What's missing is what AppUsers actually *do* — clock in / out and tell the system where they are. This change is the first one that makes argus a workplace tool rather than a user-management toolkit.

The product brief (from ROADMAP and explore conversation) is: four event types (上班 / 下班 / 轉出 / 轉入), an Org-level toggle to disable the two transfer events for offices that don't dispatch employees off-site, an admin-driven "force checkout" for end-of-day cleanup, and per-event location capture. Two ergonomic constraints emerged in explore that shape the design more than the headline features:

1. **Multi-site shifts are a first-class workflow.** A construction worker visiting sites A → B → C in one day must not need to fake-return-to-base between visits. `transfer_in` therefore means "arrived at the *next* worksite", not "back at the original primary". The state machine has only three states (`off_duty`, `on_site`, `in_transit`) and 5 legal transitions, with `on_site → in_transit → on_site → in_transit → on_site → off_duty` being the canonical multi-site flow.

2. **The AppUser is sometimes offline.** A mobile app on a construction site may have no signal for hours. Events captured offline must be sortable and faithfully represent when they actually happened, not when the network got around to delivering them. We solve this with two timestamps per event (`occurred_at_client` and `occurred_at_server`), trusting the client clock for ordering / display and the server clock for audit.

This change covers backend (new collections, new endpoints, the `ReverseGeocoder` trait + a Nominatim impl) and admin-web UI (live board, per-user history, transfer toggle, timezone setting). Flutter consumes this contract in the separate `add-app-shell` change; the AppUser-facing surface is exercised here only via integration tests and curl smoke.

## Goals / Non-Goals

**Goals:**

- Capture every clock_in / clock_out / transfer with location and time, in a way that survives offline → online sync without losing ordering.
- Give admins an at-a-glance "who is working right now" board, scoped to the active Org via the existing tenancy rules.
- Allow Orgs that don't use transfer events to opt out cleanly, without the toggle becoming a foot-gun mid-shift.
- Pin the `ReverseGeocoder` shape so future provider swaps (commercial keys, fallback chains) are isolated to one trait impl.
- Establish the dual-timestamp pattern so `add-app-shell` knows exactly what its persistent queue must guarantee.
- Make `org-tenancy`'s timezone field a real, displayed thing — but keep it strictly cosmetic so the data model stays simple.

**Non-Goals:**

- Implementing the Flutter UI, the on-device queue, or background-sync. The queue *contract* is documented; the *code* lives in `add-app-shell`.
- Auto-checkout (heuristics for "this user forgot to clock out"). Admin force-checkout is the manual fallback for now.
- Continuous location tracking / breadcrumbs between events.
- Attendance reports, shift summaries, payroll exports.
- Editing or deleting past events. force-checkout is the only after-the-fact admin path.
- Snapshotting Org timezone per event. Changing `Org.timezone` re-renders historical events under the new TZ; we treat that as acceptable display drift, not a data-correctness bug.
- Validating GPS quality. We accept whatever lat/lng/accuracy the device reports.
- Geocoding caching. Each event hits Nominatim. A cache is a future optimisation if rate becomes an issue.

## Decisions

### State machine has three states, not "n locations"

```
status: off_duty | on_site | in_transit

  off_duty   ─clock_in─────▶ on_site
  on_site    ─clock_out────▶ off_duty
  on_site    ─transfer_out─▶ in_transit
  in_transit ─transfer_in──▶ on_site         (arrives at next worksite, not necessarily original)
  in_transit ─clock_out────▶ off_duty        (forgot to transfer_in, went straight home)
```

`on_site` is "currently performing work duties at any worksite". `in_transit` is "moving between worksites". `transfer_in` does not require the AppUser to be back at the same physical place they `transfer_out`-ed from; the location is captured per-event.

The alternative — chained `transfer_out` events that update location while staying in `in_transit` — would conflate "status flag" with "location ping". Continuous location is the responsibility of `add-location-tracking`, not the four headline events. Keeping the state machine binary (`primary` vs `transit`) keeps the invariant clear: at most one state transition per event, no special "relocation" event.

### Two collections: append-only events + denormalized current state

`checkin_events` is the source of truth: every state change is one row. Indexes on `(app_user_id, occurred_at_client desc)` for personal history and `(org_id, occurred_at_client desc)` for org-wide queries.

`checkin_user_status` is a single row per AppUser holding the current `status`, the `current_shift_started_at`, and `last_event_id`. It's denormalized for two reasons:

- **Live board query**: admin-web's "who is on shift" needs `count() / list()` filtered by status, which would otherwise mean scanning the events table or maintaining a complex aggregation.
- **State machine validation**: every incoming event needs to read the AppUser's current state to decide if the transition is legal. Reading one document is cheap and atomic.

The two writes (insert event + update status) are sequenced in code: insert event first, then update status with a conditional filter on `(app_user_id, status = expected_prior_status)`. If the conditional fails (race), the event row is rolled back via best-effort delete, and the second-arriving request gets `INVALID_TRANSITION`. We don't use Mongo transactions for MVP — the conditional-update pattern matches what the rest of the codebase does and keeps the transactional cost off the hot path.

### Dual timestamps, with `client` as the canonical "when this happened"

Every event carries:

- `occurred_at_client: DateTime` — supplied by the AppUser in the request body. Trusted for sort order, display, and "events on day X" queries.
- `occurred_at_server: DateTime` — set on receipt. Used only by admin-web to render a "skew warning" icon when `|client - server| > 1 hour`, and for forensic audit.

Why two: the AppUser may be offline for hours or days. The event happened *when the AppUser pressed the button*, not when their phone got a signal. Insisting on server time would mean a 3-hour-late sync shows up as 3pm even though the worker actually clocked in at noon. That breaks payroll, accountability, and trust.

Validation: any client time is accepted, including future timestamps from a phone with a misset clock. Out-of-order events (`new.client < last_event.client` for the same AppUser) are rejected with `OUT_OF_ORDER`, on the assumption that the app's persistent queue serializes events strictly (event N waits for the server's `2xx` on event N-1 before sending). This is documented as the queue contract for `add-app-shell`.

Index on `occurred_at_client` (not server) for paging, since clients sort and admin views render by the client time.

### Server-issued `event.id` and the queue happy path

The request includes `occurred_at_client` but not an idempotency key. If a client retries the same event after a transient network failure, two rows can land. The MVP solution: the queue contract requires the client to wait for `2xx` before sending the next event AND to retry the same event (with the same `occurred_at_client`) on transient failures. Server-side `OUT_OF_ORDER` rejection prevents the same `occurred_at_client` from being inserted twice for the same AppUser (we check `<` strictly; equal client times are rejected). Worst case duplicates are observable as two rows with the same `occurred_at_client`, which admin-web will visibly highlight.

Future change can introduce a client-supplied UUID + idempotency table if duplicates become a problem in practice.

### `transfer_enabled` toggle, state-locked while anyone is on shift

`Org.settings.checkin.transfer_enabled: bool` (default `true`). When `false`, the API rejects `transfer_*` events with `403 TRANSFER_DISABLED`. The toggle itself can only flip when the Org has zero AppUsers in `on_site` or `in_transit` state — flipping mid-shift would leave employees in a non-representable state (`in_transit` with `transfer_enabled = false`), so we explicitly forbid it (`409 STATE_LOCKED`, body includes the `on_duty_count` so admin-web can render "目前在班 N 人，需先全部下班").

Timezone changes are *not* state-locked because they only affect display.

### Force-checkout is the only admin write into the event log

`POST /checkin/users/:id/force-checkout` writes a `clock_out` event with `source = admin_force` and `initiated_by = { kind: dashboard_user, id: ctx.user_id }`. The optional `reason` is stored as a free-text field on the event. The endpoint:

- Requires the target's `checkin_user_status.status` to be `on_site` or `in_transit`. If already `off_duty`, returns `409 NOT_ON_DUTY`.
- Generates a server-time `occurred_at_client` equal to `now` (server) since there's no client-supplied time. This is the one place where `occurred_at_server == occurred_at_client`.
- Sets `location` from the **last event's location** with a small adornment (`region_name` carried over, `manual_label` becomes "管理員強制收班"). Force-checkout cannot capture the AppUser's actual location; we record the last-known location to keep the event log queryable on `region_name`.

This is the only admin write into `checkin_events`. There is no admin "edit" or "delete" — events are append-only and the only post-hoc tool is force-checkout.

### `ReverseGeocoder` is a trait with one impl, not zero or two

```rust
#[async_trait]
pub trait ReverseGeocoder: Send + Sync {
    async fn lookup(&self, lat: f64, lng: f64) -> Option<String>;
}
```

One impl: `NominatimGeocoder`, configured with a fixed User-Agent matching argus and a 2-second per-request timeout. Failures (timeout, 4xx, 5xx, parse error) collapse to `None`, which the handler renders as `region_name = null`. No retries — the queue is local to the AppUser, and a `null` region is correctable later by a future "geocode backfill" job (out of scope).

Why a trait at all when there's one impl: the ROADMAP entry "Reverse geocoding provider 抽象" is real and near-term. Designing the trait now is ~10 lines of code and lets the future change be "add a `MapboxGeocoder` impl + config" instead of "redesign all the call sites".

### `Org.timezone` is purely cosmetic; DB stays absolute-time

`Org.timezone: String` is an IANA name (`Asia/Taipei`, `America/Los_Angeles`, etc.) defaulting to `"Asia/Taipei"`. It is **only** used by the rendering layer (admin-web today; future Flutter app may follow). The database stores absolute UTC moments (Mongo's BSON `DateTime` is UTC anyway), and the API never date-math's based on `Org.timezone`. If admin-web wants "today's events", it computes the UTC range from the Org's timezone client-side and passes UTC ISO timestamps as query params.

This decision keeps the server time-handling trivial and avoids the historical-event-renormalisation rabbit hole. The cost is a cosmetic display drift if an Org changes its timezone (events from "yesterday" may render as "today before 8am" or similar). Acceptable.

### Routing layout

```
public:                — none —

bearer-auth (/app/*):
  POST   /app/checkin/events         submit event
  GET    /app/checkin/status         current state
  GET    /app/checkin/events         own history (cursor pagination)

dashboard cookie + admin (/checkin/* and /orgs/me/settings):
  GET    /checkin/users                     live status board
  GET    /checkin/users/:id/events          history of one AppUser
  POST   /checkin/users/:id/force-checkout  force end-of-shift
  PATCH  /orgs/me/settings                  body { transfer_enabled?, timezone? }
```

We use `PATCH /orgs/me/settings` (not separate endpoints per setting) because the settings document is the natural unit and the existing slug / code endpoints are already at `/orgs/me/...`. State-lock behaviour applies only when the patch touches `transfer_enabled`.

### New error variants

Added to `ApiError`, all with the existing `{ code, message }` envelope:

| Variant | HTTP | Code |
|---|---|---|
| `InvalidTransition` | 422 | `INVALID_TRANSITION` |
| `TransferDisabled` | 403 | `TRANSFER_DISABLED` |
| `OutOfOrder` | 409 | `OUT_OF_ORDER` |
| `StateLocked` | 409 | `STATE_LOCKED` |
| `NotOnDuty` | 409 | `NOT_ON_DUTY` |
| `InvalidTimezone` | 400 | `INVALID_TIMEZONE` |

`InvalidTransition` body includes the current state and the attempted event so the client can render a useful message.

## Risks / Trade-offs

- **Mongo race on concurrent same-AppUser events** → conditional `findOneAndUpdate` on `(app_user_id, status = expected)`. Second-arriving event gets `INVALID_TRANSITION` because the prior state is now different. Acceptable; multi-device same-user is rare and can re-derive on retry.
- **Client clock manipulation** → an AppUser could backdate or future-date events. We accept any client time and surface skew warnings in admin-web. Real fraud detection is a separate concern (could compare with location reasonableness etc); MVP only highlights, doesn't block.
- **Nominatim availability / rate** → public Nominatim has usage policy limits (1 req/s avg, attribution, User-Agent). For pilot / dev fine. For real production, swap impl to a paid provider (Mapbox, Google) — that's a config change, not a redesign, thanks to the trait. Document the swap path in `api/README.md`.
- **Timezone display drift on Org TZ change** → cosmetic only; data is absolute time. We document this and don't try to freeze TZ per event.
- **Force-checkout location is "last known"** → admin sees the worker's last reported location, not their actual location at force-checkout time. The alternative (no location) is worse. Manual label "管理員強制收班" makes the synthetic origin clear.
- **`OUT_OF_ORDER` forces strict client-side queueing** → if the queue is wrong, events get rejected and the AppUser may end up in a stuck state until the bad event is dropped. Documented contract; `add-app-shell` test plan should include a failure-recovery scenario.
- **No idempotency key for retries** → a misbehaving client can submit duplicate events with the same client time. We rely on `OUT_OF_ORDER` (strict `<`) to reject true duplicates and let admin-web flag visible repeats. If duplicates are observed in practice, future change adds a UUID + idempotency table.
- **Two-collection consistency** → if the status update fails after the event insert, we have an event without a state update. Mitigation: on next request, the conditional update will find the actual current state (derived from events) rather than the stale denormalized one — but to be safe, we add a startup repair task that scans `checkin_user_status` against the latest event for each AppUser and fixes drift. (A small task, listed in `tasks.md`.)
- **Reverse geocoding latency on the hot path** → Nominatim adds 100-500ms to event submission. Acceptable for MVP. If it becomes a problem, switch to async backfill or add caching.

## Migration Plan

1. Add `checkin_events` collection with indexes `(app_user_id, occurred_at_client desc)` and `(org_id, occurred_at_client desc)`.
2. Add `checkin_user_status` collection with unique index on `app_user_id` and secondary index on `(org_id, status)`.
3. Add `Org.timezone` field with default `"Asia/Taipei"` and a migration step that backfills existing Orgs.
4. Add `Org.settings.checkin` sub-document (default `{ transfer_enabled: true }`); existing Orgs without this sub-document treat it as the default at read time.
5. Wire `ReverseGeocoder` instance into `AppState`, default config = Nominatim with 2s timeout and the argus User-Agent.

No existing event data to migrate. The Org backfill can be written as part of this change's startup hook or a one-shot migration utility — either is fine since the read path tolerates missing fields.

## Open Questions

None blocking implementation. Deferred but worth noting:

- Whether the Nominatim impl should cache results per (lat, lng) rounded to e.g. 5 decimals. Simple in-memory LRU would meaningfully cut traffic for a busy Org. Holding off until we observe rates.
- Whether admin-web's live board should refresh via polling, SSE, or WebSocket. MVP is polling every 30 seconds. Real-time push is a separate UX upgrade.
- Whether `force-checkout` should also be available on the `/app/me/...` surface (i.e. AppUser self-checkout when their button doesn't work). Today no — they always have `clock_out` available; if they're stuck due to a bug, admin force-checkout is the workaround.
- Whether to expose a `since=<ISO>` query param on `/app/checkin/events` for incremental sync after the app has been offline. Probably yes when `add-app-shell` lands; for now cursor pagination by `occurred_at_client` is enough.
