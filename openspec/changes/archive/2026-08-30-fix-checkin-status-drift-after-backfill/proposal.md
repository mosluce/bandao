## Why

The legacy backfill script writes `checkin_events` rows directly at the repository layer and deliberately leaves `checkin_user_status` reconciliation to the `repair_checkin_status_drift` routine, which only runs once at API process startup. That was an acceptable trade-off while the script was a one-off, developer-run cutover tool. It stopped being acceptable when the import became an unattended hourly LaunchAgent (`add-legacy-backfill-windows-and-pings` → `api/scripts/legacy_backfill_run.sh`): events now arrive every hour, but nothing reconciles the projection, so the status board freezes at whatever snapshot the last API restart produced.

This is live in production. On KLCC's board every AppUser whose events come from the legacy import has been stuck since the last API restart on 2026-07-31 — the board shows them `on_site` with shift durations over 720 hours, while their event history correctly shows they clocked out. The board is the primary operational view of who is on shift, and it has been wrong for a month.

## What Changes

- Extract the per-AppUser half of the startup drift repair (`startup::repair_one`) into a public, reusable reconciliation function that callers can invoke for a single AppUser. `repair_checkin_status_drift` keeps its current whole-collection behaviour by calling into it.
- The legacy backfill script reconciles `checkin_user_status` at the end of a real (non-dry-run) run, for every AppUser that had at least one checkin-routed legacy document in the run's window — not only those with newly-inserted rows, so a re-run over an already-imported window still repairs drift.
- The run summary reports how many AppUsers were reconciled, and the "Restart the bandao-api process now so `repair_checkin_status_drift` reconciles `checkin_user_status`" line is removed: unattended hourly runs have nobody to act on it, and it is no longer true.
- Dry-run stays read-only — it reports what it would reconcile and writes nothing.

Not in scope: making the API reconcile periodically at runtime, or deriving board status from the event log instead of the projection. Both are larger designs; this change closes the hole in the writer that actually broke.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `legacy-checkin-backfill`: the "Imported rows bypass live state-machine and ordering validation" requirement currently mandates that status reconciliation is left to the startup repair routine, with a scenario asserting the script does not necessarily update `checkin_user_status`. That is the defect. The requirement changes to: imported rows still bypass the state machine and ordering checks, but the script SHALL reconcile `checkin_user_status` for the AppUsers it touched before exiting.

## Impact

- `api/src/startup.rs` — `repair_one` / `imply_status` become reusable; the module gains a public single-AppUser entry point. Startup behaviour is unchanged.
- `api/examples/legacy_backfill.rs` — tracks touched AppUser ids during the import loop, runs reconciliation after it, extends the summary, drops the restart notice.
- `api/src/services/legacy_backfill.rs` — `RunSummary` gains a reconciliation counter if the summary is reported through it.
- `api/tests/legacy_backfill_import.rs` — the existing test calls `repair_checkin_status_drift` manually after import; it gains coverage that the script's own reconciliation makes that unnecessary.
- `api/scripts/legacy_backfill_run.sh` and `DEPLOY.md` — no code change required, but the operational note that a restart is needed after import is no longer accurate and is corrected.
- Production remediation: once deployed, the next hourly run reconciles every AppUser with legacy activity in the last day, which covers the currently-stuck KLCC users. An API restart is the immediate stop-gap before then.
- No API surface, DTO, or database schema change. `admin-web` and `app` are untouched.
