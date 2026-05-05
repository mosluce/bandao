## Context

The previous client-side change (`add-app-checkin-polish`) shipped the
device-local queue + workmanager + GPS scaffolding with the explicit
intent of consuming a continuous location stream next. That stream needs
somewhere to land. `add-org-privacy-policy` already disclosed the
collection (90-day retention, conditional on Org enabling) so workers
who consent on the upcoming app-side change won't see surprise
disclosures. This change is the API plumbing that ties the two ends
together.

Three orthogonal concerns drive the design:

1. **Volume vs cost** — at peak (one mid-size org, 100 workers, full
   workday) the server might receive ~480 raw pings/worker, dropping to
   ~150 after the 100m client-side distance filter. Across 100 workers
   that's 15K pings per workday landing in batches. Existing
   reverse-geocoding pipeline cannot keep up at this rate (Nominatim
   2-second timeout × 15K calls = 8+ hours of work per day; rate-limit
   issues), so pings skip geocoding entirely.

2. **Privacy boundary** — pings are more sensitive than checkin events.
   The 90-day TTL is enforced by the database (not the application),
   so it's not bypassable via a forgotten code path. The admin export
   path only emits xlsx files (vs raw API access) so we have a clear
   audit point for "data left the system".

3. **Two-toggle settings UX** — the existing `transfer_enabled` toggle
   is already state-locked (an admin can't flip it mid-shift). Adding
   a second toggle (`location_tracking_enabled`) raises the question
   of whether they should share one lock or each have their own. We
   pick the unified lock for both UX coherence and code simplicity.

## Goals / Non-Goals

**Goals:**

- Land the API endpoints needed for `add-location-tracking-app` to
  ship next, with a stable contract that the app can hand-roll DTOs
  against.
- Enforce the 90-day retention at the database level via TTL —
  privacy commitment is not subject to "did we remember to call the
  cleanup job?" reasoning.
- Make per-ping validation cheap enough that a 100-ping batch from a
  long-offline phone returns in milliseconds. Reject obviously-bad
  pings (future timestamps, >30d old, out-of-range coordinates) at
  the boundary; let valid pings into the partial-accept set.
- Preserve the "single source of state-lock" property of the settings
  endpoint — adding a toggle shouldn't add a code path that could
  diverge from the existing one.
- Provide an xlsx export path that a non-technical admin can open in
  Excel without translation steps.

**Non-Goals:**

- Server-side enforcement that pings fall within a worker's `on_duty`
  windows. The client already guards submission via its home-screen
  status; server-side enforcement would require historical event
  scanning per ping (~30 db reads × 100 pings = 3000 reads/batch),
  not worth it for the marginal correctness gain.
- Reverse-geocoding pings. Reasoning above (Nominatim throughput).
- CSV / JSON / Parquet export formats. Single format keeps the surface
  small; xlsx is the format Taiwanese SMB admins are most comfortable
  with.
- Bulk org-wide export endpoint. Per-AppUser only; multi-user export
  needs a background-job design that's out of MVP scope.
- Admin "delete this user's pings" (D-1) endpoint from explore. MVP:
  90-day TTL covers most §11 erasure cases; explicit deletion routes
  through `mongoexport` + admin DBA action. Add the endpoint later if
  real requests emerge.
- Pings authored by `admin_force` source. Pings are purely
  AppUser-initiated (no admin equivalent of force-checkout's bypass).

## Decisions

### TTL field is `occurred_at_server`, not `occurred_at_client`

A worker's phone with a forward-jumped clock (rare but real — bad
NTP, time-zone change) submits a ping with `occurred_at_client` set
to "next month". If TTL ran on `occurred_at_client`, that ping would
live for 90 days *from the bogus future date* — i.e. far longer than
intended. Conversely a backward-jumped clock would prematurely
delete recent pings. Using `occurred_at_server` (UTC, monotonic on
our side) avoids both edges.

The trade-off: a ping's "logical time" (`occurred_at_client`, which
admins read on the map) differs from its "retention reference"
(`occurred_at_server`). For typical clients with correct clocks
they're within seconds — 90-day boundary effectively the same. For
pathological clients the data still gets deleted "90 days after we
got it", which is the right semantic.

### Insert path uses `insert_many(ordered: false)` for partial accept

A batch arriving with one bad ping shouldn't reject the other 99.
MongoDB's `insert_many(ordered: false)` does exactly this:

- Continues past errors instead of stopping at the first.
- Returns a `BulkWriteError` with `write_errors` listing the failed
  indices and reasons.
- Successful inserts are committed.

Handler flow:

```
1. Validate Org toggle on; toggle off → 403 entire batch.
2. Validate batch size 1..=100; out of range → 400 INVALID_BATCH.
3. Iterate pings, separate into (valid, rejected_with_reason). Range
   checks (lat/lng/accuracy/timestamp) happen in pure-Rust code
   before any db roundtrip.
4. Build domain LocationPing structs for all valid pings, with
   `occurred_at_server = now()`.
5. Call `repo.insert_many(valid)`. The repo wraps
   `insert_many(ordered: false)` and converts BulkWriteError into a
   list of "successfully inserted" indices (we map back to original
   batch indices).
6. Anything still failing at db level (extremely unusual — only
   schema / connectivity errors get here, since we pre-validated)
   gets folded into the `rejected[]` response with a generic
   `code = INSERT_FAILED`.
7. Response 201: { accepted_count, rejected: [...] }.
```

The 201 is correct even when `accepted_count == 0` (e.g. all 100
pings rejected for bad timestamps): the request was processed, the
client gets actionable per-index feedback, and treating "all
rejected" as 422 instead would force the client to handle two
different success/failure shapes.

### State-lock unification

The existing `update_settings` handler already calls
`count_on_duty_in_org` only when `transfer_enabled` is being
patched. The neat extension is "lock when **either** toggle is in
the patch":

```rust
if req.transfer_enabled.is_some() || req.location_tracking_enabled.is_some() {
    let on_duty_count = state.db.checkin_user_status
        .count_on_duty_in_org(active.org_id).await?;
    if on_duty_count > 0 {
        return Err(ApiError::StateLocked { on_duty_count: ... });
    }
}
```

One condition, both toggles share the same `STATE_LOCKED` error
code. This is the cleanest path because the lock's *reason* is the
same: "settings flips during a shift cause data inconsistency". It
also matches admin mental model: settings page shows "locked while
workers are on shift" once, not per-toggle.

The alternative (separate locks per toggle) would let an admin flip
`transfer_enabled` while `location_tracking_enabled` was locked —
but in practice both flips have the same data-consistency concern,
so the differentiation is illusory.

### Range checks live in handler, not domain

Per-ping validation (`lat ∈ [-90, 90]`, `occurred_at_client` not
future / not >30d old) is purely a request-boundary concern: the
domain `LocationPing` struct doesn't need invariants beyond "well
typed". This matches existing `CheckinEvent`'s pattern — domain
types stay simple data carriers, request-time validation lives in
the handler module.

### Export endpoint streams xlsx via in-memory buffer

`rust_xlsxwriter` builds an in-memory xlsx via its `Workbook` API,
then we serve the bytes back with the right `Content-Type` and
`Content-Disposition` headers. Per-AppUser, ≤ 90-day range cap
keeps this comfortably under 50K rows even at peak (a single worker
with 90 days of dense data ≈ 100 × 8 × 22 × 3 × 0.3 ≈ 50K). xlsx
file size at this scale: < 5 MB.

For 50K rows xlsx-builder takes < 1 second on modern hardware. The
endpoint isn't latency-sensitive (admin export is a once-per-month
operation); a 1-second response time is fine. If the volume ever
forces streaming we can switch to `XlsxStreamWriter` later, but
that's a YAGNI call for v1.

The export query reads from the existing `(app_user_id,
occurred_at_client)` index. We sort ascending by
`occurred_at_client` so the spreadsheet is naturally
chronological.

### `Org.checkin_location_tracking_enabled` defaults to `false`

The flag is stored under `Org.settings.checkin.location_tracking_enabled`
(parallel to `transfer_enabled`). A missing field reads as `false`,
matching the existing `checkin_transfer_enabled` helper's pattern of
defaulting via `unwrap_or(true)`. The default-to-`false` choice
matches privacy convention (opt-in, not opt-out): an existing Org
that didn't explicitly set the flag gets no tracking.

### Endpoint location: new `handlers::location_tracking` module

`handlers::checkin.rs` already exceeds 300 lines with the events
surface; adding 3 more endpoint functions plus xlsx-builder code
would push it past comfortable. We create a sibling module
`handlers::location_tracking.rs`. Routes wire up next to the
existing `/app/checkin/*` and `/checkin/*` paths.

The settings endpoint stays in `handlers::checkin.rs` since the
toggle change is a one-line addition to the existing handler — no
need to move it.

### DTO addition pattern

`OrgCheckinSettingsDto` (admin-side org settings response):

```rust
#[derive(Debug, Serialize)]
pub struct OrgCheckinSettingsDto {
    pub transfer_enabled: bool,
    pub location_tracking_enabled: bool,  // new
}
```

`UpdateOrgSettingsRequest`:

```rust
#[derive(Debug, Deserialize, Default)]
pub struct UpdateOrgSettingsRequest {
    pub transfer_enabled: Option<bool>,
    pub timezone: Option<String>,
    pub location_tracking_enabled: Option<bool>,  // new
}
```

The `OrgCheckinDto` exposed via `/me` (dashboard) and `/app/me`
(mobile) lives in `handlers::auth` / `handlers::app_dto`; both gain
the same field. The DTOs are re-cut at the bottom of the response
chain (they read from `Org` directly), so adding one field
propagates without touching call sites.

## Risks / Trade-offs

- **TTL drift**: Mongo's TTL monitor sweeps approximately every 60s,
  so retention is "90 days ± a minute". Mitigation: spec language
  says "approximately 90 days"; document operational reality in
  `api/README.md`.
- **Insert-many partial-success error mapping**: The Rust mongo
  driver's `BulkWriteError` carries `write_errors: Vec<WriteError>`
  with per-index info. We map `write_errors[i].index` (an offset
  into our valid-pings sub-array) back to the original batch index.
  Edge: if our valid sub-array is `[0, 2, 5, 7]` (skipping
  pre-rejected) and write_errors has `index: 1`, that maps to
  original index 2. Slightly fiddly but correct.
- **xlsx in-memory buffer at peak**: 50K rows ≈ 5 MB file ≈ 30-50 MB
  RAM during construction (Workbook buffers more than the final
  output). One concurrent admin export is fine; 10 concurrent
  exports of full 90-day ranges is ~500 MB. Mitigation: in practice
  admins don't run concurrent exports. If observed, add semaphore.
- **Toggle off mid-batch**: Admin flips `location_tracking_enabled`
  to `false` while a batch is in flight. State-lock prevents this in
  most cases (state-lock requires nobody on shift; if anyone is
  actively pinging they're on shift). The pathological case is a
  ping that arrives milliseconds after the toggle flip but before
  state propagates — the request will hit `LOCATION_TRACKING_DISABLED`
  on the toggle check, which is the correct behavior.
- **`occurred_at_server` is set at db-write time, not request-receive
  time**: A request sitting in a queue for seconds before the
  handler runs would have `occurred_at_server` skewed by the queue
  delay. Mitigation: we use `DateTime::now()` at the moment we build
  the LocationPing struct (just before insert_many), giving
  sub-millisecond accuracy in normal operation. Queue delays are
  ms-scale anyway.
- **`accuracy_meters` semantics across iOS / Android**: iOS reports
  CoreLocation horizontal accuracy in meters; Android reports
  fused-provider accuracy in meters. Both are 68% confidence
  radius. We trust both as-is — semantics close enough for a "is
  this point trustworthy" UI affordance, no normalization needed.
- **xlsx column types**: We type `lat`, `lng` as numbers (Excel
  interprets correctly), `occurred_at_client` and `occurred_at_server`
  as ISO8601 strings (avoid Excel's date auto-detection mangling
  zone offsets). `accuracy_meters` as number with empty cells where
  null. Row 1 frozen, columns auto-sized.

## Migration Plan

No data migration. New collection, new endpoints, new field in
existing `Org.settings.checkin` sub-document (defaults to `false`,
no read errors for existing orgs).

For developers:

1. Pull, run `cargo build`. New `rust_xlsxwriter` dep pulled.
2. `cargo test` — integration tests use testcontainers Mongo, no
   manual DB setup.

For operators:

- Existing orgs continue with no change in behavior
  (`location_tracking_enabled` reads as `false` for any org without
  the field, identical to a fresh org).
- The TTL index is created on `cargo run` startup (idempotent;
  re-running is safe).
- 90-day TTL applies from the moment the index lands; backfilling
  isn't relevant since the collection starts empty.

For end-users:

- No-op until `add-location-tracking-app` ships. The mobile client
  doesn't know about the new endpoints yet; `Org.checkin
  .location_tracking_enabled` is delivered to clients but they
  don't act on it.

No rollback concerns pre-launch.

## Open Questions

- **Should we surface `LOCATION_TRACKING_DISABLED` differently from
  `TRANSFER_DISABLED`?** Both are "Org has disabled this feature"
  rejections; the codes differ but the structural shape is the
  same. Keeping them distinct lets clients show feature-specific
  error UI, which is the right call.
- **Should the xlsx filename include the AppUser's username or just
  the id?** Username is more readable in download dialogs; id is
  more stable. We include username in the suggested filename
  (`Content-Disposition`), but it's a hint, not authoritative.
- **Should `accuracy_meters` clamp at some upper bound (e.g. reject
  pings claiming 50km accuracy)?** Probably not in this change —
  let bad pings through and let the dashboard map filter visually.
  If real noise emerges, add a bound in a follow-up.
- **Should the export endpoint also optionally include `region_name`
  via lookups?** No — pings don't have geocoded region names by
  design (volume), and adding lookups in the export path would
  re-introduce the Nominatim rate-limit problem at a new place.
