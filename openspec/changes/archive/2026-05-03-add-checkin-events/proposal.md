## Why

The dashboard and AppUser identity layers are now in place, but argus's reason for existing — letting employees clock in / out and letting admins see who's currently working — has zero implementation. Without it, AppUsers can log in but have nothing to do, admins can manage employees but can't see them in action, and downstream features (auto-checkout, location tracking trajectory, attendance reports) all stall.

This change delivers the core checkin/checkout state machine plus the supporting org-level toggles, admin force-checkout, and reverse geocoding. AppUser-side endpoints are exercised via `curl` in this change; the Flutter UI that consumes them is the separate `add-app-shell` work item, which can land in any order relative to this one.

## What Changes

- **Four event types** with a strict state machine for each AppUser:
  - `clock_in` (`off_duty` → `on_site`)
  - `clock_out` (`on_site` or `in_transit` → `off_duty`)
  - `transfer_out` (`on_site` → `in_transit` — leaving a worksite)
  - `transfer_in` (`in_transit` → `on_site` — arriving at the next worksite)
  - `transfer_in` represents arriving at the *next* worksite (not necessarily the original); cycles `on_site → transit → on_site → transit → on_site` are normal for multi-site days. Illegal transitions return `422 INVALID_TRANSITION`.

- New collections:
  - `checkin_events` — append-only log. Each row holds the AppUser, the event type, server-issued and client-claimed timestamps, the source (`app` or `admin_force`), the initiator identity, and a location (lat/lng + optional accuracy + optional reverse-geocoded `region_name` + optional manual label).
  - `checkin_user_status` — denormalized current state per AppUser, with `current_shift_started_at` and `last_event_id`. Updated atomically with each event so admin queries for "who is on shift" are a single index hit.

- New `Org.settings` fields under a `checkin` sub-document:
  - `transfer_enabled: bool` (default `true`). Toggle is **state-locked**: admin cannot flip it while any AppUser in the Org is `on_site` or `in_transit` (responds `409 STATE_LOCKED` with the on-duty count). Disabling transfer rejects `transfer_*` events with `403 TRANSFER_DISABLED`.

- New `Org.timezone: string` (IANA, default `"Asia/Taipei"`). Display-only — DB stores absolute time without timezone information. Admin-web uses this for rendering all timestamps; future Flutter app may use it or device locale.

- New `/app/checkin/*` endpoints (Bearer auth, AppUser):
  - `POST /app/checkin/events` — submit a new event with `{ type, lat, lng, accuracy?, manual_label?, occurred_at_client }`.
  - `GET /app/checkin/status` — current state + last event.
  - `GET /app/checkin/events` — own event history (cursor pagination).

- New `/checkin/*` endpoints (dashboard cookie + admin):
  - `GET /checkin/users` — list AppUsers with current status (live "who is working" board).
  - `GET /checkin/users/:id/events` — single AppUser's history.
  - `POST /checkin/users/:id/force-checkout` — force `clock_out` with optional free-text reason.

- New `PATCH /orgs/me/settings` (admin) accepting `{ transfer_enabled?, timezone? }`. Rejects `transfer_enabled` flips when state-locked; `timezone` is always changeable.

- **Reverse geocoding abstraction**: introduces a `ReverseGeocoder` trait with one Nominatim-backed implementation. Failures are fail-soft (`region_name = null`, event still recorded). Future provider swaps are a configuration / impl change.

- **Dual timestamps with offline tolerance**:
  - `occurred_at_client` (required in the request body) is what the AppUser device records when the event happened — used for ordering, display, and "today's events" queries.
  - `occurred_at_server` is set by the server on receipt — used by admin-web to flag suspicious skew (`|client - server| > 1 hour` shows a warning icon next to the event).
  - Any client time is accepted (including offline-sync events from days ago); only `OUT_OF_ORDER` (a new event whose `client` time is earlier than that AppUser's most recent stored event's `client` time) is rejected, on the assumption that the app's local queue serializes events strictly.

- **App-side queue contract** (consumed by `add-app-shell`): events are persisted locally before being sent and the app must wait for `2xx` on event N before sending event N+1. This is **not** code in this change — it's the contract that makes `OUT_OF_ORDER` rejection a non-issue in practice.

Out of scope:

- Flutter app implementation of the queue / checkin UI (`add-app-shell`).
- Auto-checkout (separate ROADMAP item).
- Continuous location tracking / trajectory map (separate ROADMAP items: `add-location-tracking`, `軌跡視覺化`).
- Editing or deleting past events. MVP only allows `force-checkout` as a corrective.
- Attendance reports / payroll integration.
- Historical timezone snapshotting (changing `Org.timezone` re-renders past events under the new TZ; we accept the slight cosmetic shift as the right MVP trade-off).

## Capabilities

### New Capabilities

- `checkin-events`: state machine, the two new collections, AppUser + admin endpoints, force-checkout, transfer-enabled toggle with state-lock semantics, reverse-geocoding contract.

### Modified Capabilities

- `org-tenancy`: extends the existing `Org has a settings container` concept with a concrete `checkin` sub-document and adds `Org.timezone`. The `settings` container requirement gets a small clarification + new requirement for `Org has a configurable timezone`.

## Impact

- **Schema**: two new collections (`checkin_events`, `checkin_user_status`) plus indexes. `Org` document gains `timezone: string` and a structured `settings.checkin` sub-document.
- **API code**: new `api/src/handlers/checkin.rs` (admin) and `api/src/handlers/app_checkin.rs` (mobile). New repos `api/src/db/checkin_events.rs` and `api/src/db/checkin_user_status.rs`. New `api/src/services/reverse_geocoder.rs` defining the trait + `nominatim.rs` impl. New error variants (`InvalidTransition`, `TransferDisabled`, `OutOfOrder`, `StateLocked`, `NotOnDuty`).
- **API tests**: integration tests for every legal/illegal transition, transfer toggle behaviour, state-lock enforcement, force-checkout, dual timestamp + skew handling, OUT_OF_ORDER rejection, geocoding fail-soft, and AppUser scoping.
- **admin-web**: new `pages/checkin/index.vue` (live board: who is on_site / in_transit / off_duty + current shift duration), `pages/checkin/[appUserId].vue` (single user's event history with skew warnings), and a transfer-toggle + timezone field on `pages/index.vue`. New types in `types/api.ts`, new `composables/useCheckin.ts`.
- **Docs**: `api/README.md` gains a checkin section (state machine table, error codes, ReverseGeocoder trait). `admin-web/README.md` describes the checkin board and timezone setting. ROADMAP entry for `add-checkin-events` moves to "delivered" once landed; `add-location-tracking` becomes the next-in-chain.
- **Reverse geocoding network**: API gains an outbound dependency on Nominatim by default. Document the User-Agent and rate considerations in `api/README.md`.
- **No Flutter changes** in this change.
