## Context

`checkin_user_status` is a projection: one row per AppUser holding `status`, `current_shift_started_at`, and `last_event_id`. It is the only thing the status board (`GET /checkin/users`) reads, and the only thing `POST /app/checkin/events` consults to decide whether a transition is legal. The event log is the source of truth; this row is the derived view.

Two writers keep it in sync, and one does not:

- `handlers/app_checkin.rs::submit_event` inserts the event and then calls `checkin_user_status.update_to(...)` conditionally on the prior status. Correct by construction.
- `handlers/checkin.rs` force-checkout does the same. Correct.
- `examples/legacy_backfill.rs` calls `checkin_events.upsert_legacy(...)` and stops there. It never touches the projection.

The gap was a deliberate, documented decision in `add-legacy-backfill-windows-and-pings`: the script was a developer-run cutover tool, the operator would restart the API afterwards, and `startup::repair_checkin_status_drift` would rebuild every row from the event log on boot. The script even prints a reminder to do so. The spec encodes it as a scenario: "Status is reconciled on next API restart, not by the script."

`add-legacy-backfill-windows-and-pings` then turned the script into an unattended LaunchAgent running hourly, 08:00–19:00, with `--since-days 1`. Nobody restarts anything. The projection has not been rebuilt since the API last booted on 2026-07-31, so for every AppUser whose events arrive via the import the board has been frozen for a month: `status = on_site` from that morning's clock-in, `current_shift_started_at` a month stale, `last_event_id` pointing at a long-superseded event. The 722-hour shift durations on the board are that stale timestamp rendered faithfully.

Constraints:

- The example binary can only use `bandao_api`'s public API; whatever it calls has to be exported from the library crate.
- The repair primitive re-seats a row by deleting and re-inserting it before transitioning to the implied state (a conditional `find_one_and_update` cannot move from an unknown prior). That is destructive if it races a live writer.
- Runs must stay idempotent and safe to re-run over an already-imported window — that is what makes the overlapping `--since-days 1` schedule safe.

## Goals / Non-Goals

**Goals:**

- After a real backfill run finishes, `checkin_user_status` agrees with the event log for every AppUser that run touched, without a process restart.
- A run over a window whose events are all already imported still repairs drift — otherwise the first run after deploy would fix nothing, since the currently-stuck users' recent events are already in the collection.
- The single-AppUser repair primitive has one implementation, shared by the startup sweep and the backfill, so the two cannot diverge.
- Dry-run remains strictly read-only.

**Non-Goals:**

- Periodic in-process reconciliation inside the API (a background task on a timer). It would paper over any future writer that forgets the projection instead of fixing it, and it needs its own design for scheduling, jitter, and interaction with live writers.
- Serving the board from the event log directly, dropping the projection. That removes this whole class of bug but changes the read path, the transition guard, and the state-lock guard on `PATCH /orgs/me/settings`. Separate change.
- Improving `current_shift_started_at` accuracy for shifts reconstructed from a `transfer_in` (see Risks). Pre-existing behaviour of the repair primitive; not made worse here.
- Backfilling the projection for AppUsers with no legacy activity in the run window.

## Decisions

### 1. Extract the per-AppUser repair into `services/checkin_status.rs`; keep the sweep in `startup.rs`

`startup::repair_one` already is the reconciliation primitive — it reads the AppUser's latest event, derives the implied status via `imply_status`, and re-seats the row when it disagrees. The change moves `repair_one`, `imply_status`, and `RepairOutcome` into a new `api/src/services/checkin_status.rs` as a public `reconcile_app_user(db, app_user_id, org_id) -> Result<RepairOutcome, mongodb::error::Error>`, and leaves `startup::repair_checkin_status_drift` as the thin whole-collection sweep that iterates `app_users` and calls it.

Why not simply make `repair_one` public where it is: the backfill script importing a per-user repair function out of a module named `startup` is a naming lie, and AGENTS.md puts logic in the service layer rather than in wiring. Keeping the sweep in `startup.rs` means `main.rs` and `api/tests/legacy_backfill_import.rs` keep their current import path, so the move stays contained.

Why not reimplement reconciliation inside the script: the previous design explicitly rejected that ("rather than reimplemented by the script") and the reason still holds — two copies of the implied-status table would drift.

### 2. Reconcile every AppUser the run routed a checkin document to, not only those with new inserts

The import loop accumulates a `HashSet<ObjectId>` of AppUser ids for which `route_action` returned `RoutedAction::Checkin`, regardless of whether `upsert_legacy` reported a new insert or an existing row. After the loop, in non-dry-run mode, the script calls `reconcile_app_user` once per id.

Keying on new inserts only is the tempting narrower choice and it is wrong for the case that motivated this change: after deploying the fix, the stuck users' last-day events are *already* imported, so an inserts-only pass would reconcile nobody and the board would stay frozen until they next generate a genuinely new event. Keying on "appeared in this run's window" repairs them on the first run after deploy.

`RoutedAction::Path` documents are excluded — `location_pings` do not participate in the status machine.

Why not reconcile every AppUser in the org: it is a Mongo round-trip per user per hour for no benefit, and it widens the destructive re-seat (Decision 3) to App-native users who are actively clocking in and out. Scoping to touched users keeps that risk on legacy-sourced users, who by definition are not submitting live events.

### 3. Reconciliation is best-effort and does not fail the run

Per-user errors are printed as warnings, matching how the script already handles per-document upsert failures, and the process still exits 0 when the import itself succeeded. A reconciliation failure means the projection is as stale as it was before — the pre-existing condition — and failing the LaunchAgent run would not improve it while making the job's exit status a poor signal for import health. `reconcile_app_user` returning `Result` (rather than swallowing internally) lets the caller decide; the startup sweep keeps its existing swallow-and-log.

### 4. Summary reports reconciliation; the restart notice goes away

The run summary gains a `reconciled: N` line. Dry-run reports `would reconcile: N` and performs no writes, so an operator can see the blast radius before a real run. The `Restart the bandao-api process now...` line is deleted — it is now false, and under the LaunchAgent nobody reads it anyway. `DEPLOY.md`'s corresponding note is corrected in the same change.

## Risks / Trade-offs

**Re-seating races a live writer** → `reconcile_app_user` deletes and re-inserts the status row before transitioning it. If an App-native user submits an event during that window, the submit path's conditional `update_to` can lose against a row that is missing or freshly `off_duty`, surfacing a spurious `INVALID_TRANSITION` to the user. Mitigated three ways: the no-drift early return (`status == implied && last_event_id == Some(latest.id)`) means the common case never deletes anything; reconciliation is scoped to AppUsers with legacy activity in the window, who are not using the App; and this is the same primitive the startup sweep has always run against the whole collection. Not newly introduced, but the exposure goes from once-per-boot to hourly, which is the honest cost of this fix.

**`current_shift_started_at` is approximated from the latest event** → When the latest event is `transfer_in`, the repair stamps the shift start as the transfer time rather than the original `clock_in`, so the board under-reports that shift's duration. Pre-existing in the startup repair and out of scope here; recorded as an Open Question because the board now depends on the repair far more often than it used to.

**A run reconciles users whose events were all already imported** → Deliberate (Decision 2), but it means the hourly job performs a handful of extra `latest_for_app_user` + `find` reads per run. At KLCC's scale (tens of AppUsers active per day) this is negligible next to the ~978K-document legacy collection scan the run already does.

**The spec previously blessed the broken behaviour** → The delta must MODIFY the existing requirement rather than add a new one, otherwise the archived spec keeps a scenario asserting the script need not update the projection, directly contradicting the new one.

## Migration Plan

1. Merge and deploy the API build. No schema or API surface change; no client coordination needed.
2. Re-run `api/scripts/legacy_backfill_sync.sh` — the LaunchAgent runs a *copy* of the binary in the deploy dir under `$HOME`, so a rebuild that is not synced changes nothing. This is the step most likely to be forgotten.
3. Immediate remediation for the currently-stuck rows: restart `bandao-api` once, which runs the existing startup sweep across all AppUsers. Alternatively wait for the next hourly run, which reconciles everyone with legacy activity in the last day.
4. Verify on the board that the affected AppUsers show plausible statuses and shift durations, and that a user whose last event is a `下班` shows as `下班`.

Rollback: revert the API build. The script reverts to writing events only, and the projection resumes drifting until the next restart — i.e. back to today's behaviour. No data written by this change needs undoing; reconciliation only ever moves the projection toward the event log.

## Open Questions

- Should `reconcile_app_user` walk back to the shift's originating `clock_in` to set `current_shift_started_at` exactly, instead of stamping the latest event's time when that event is a `transfer_in`? It would make the board's shift durations correct for multi-site shifts reconstructed from imported history. Deferred rather than decided — it changes the repair primitive's semantics for the startup sweep too.
- Does any other unattended writer touch `checkin_events` without the projection? None found today; worth a note in the spec so the next importer inherits the obligation.
