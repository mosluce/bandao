## 1. Extract the reconciliation primitive

- [x] 1.1 Create `api/src/services/checkin_status.rs` and register it in `api/src/services/mod.rs`.
- [x] 1.2 Move `repair_one`, `imply_status`, and `RepairOutcome` out of `api/src/startup.rs` into the new module, exposing `pub async fn reconcile_app_user(db: &Db, app_user_id: ObjectId, org_id: ObjectId) -> Result<RepairOutcome, mongodb::error::Error>` and making `RepairOutcome` public.
- [x] 1.3 Move the `imply_status` unit tests in `startup.rs`'s `mod tests` across with the code they cover.
- [x] 1.4 Reduce `startup::repair_checkin_status_drift` to the whole-collection sweep that iterates `app_users` and calls `reconcile_app_user`, keeping its swallow-and-log error handling and the `fixed` / `initialised` tally. `main.rs`'s call site and `api/tests/legacy_backfill_import.rs`'s import path must not need changing.
- [x] 1.5 `cargo build` and `cargo test` — startup behaviour is unchanged, so the existing suite must pass untouched at this point.

## 2. Reconcile inside the backfill run

- [x] 2.1 In `api/examples/legacy_backfill.rs`, accumulate a `HashSet<ObjectId>` of AppUser ids for every document that `route_action` resolves to `RoutedAction::Checkin`, regardless of whether `upsert_legacy` reported a new insert or an already-present row. Do not add ids for `RoutedAction::Path`.
- [x] 2.2 After the import loop, when not in dry-run, call `reconcile_app_user` once per collected id and count the successes into a `reconciled` tally.
- [x] 2.3 On a per-AppUser reconciliation error, print a warning naming the AppUser id and continue with the remaining ids; do not change the process exit status.
- [x] 2.4 In dry-run, perform no reconciliation and report the number of AppUsers a real run would have reconciled.

## 3. Summary and operator-facing output

- [x] 3.1 Add the reconciled count to the printed run summary (`reconciled: N` for a real run, `would reconcile: N` for a dry run).
- [x] 3.2 Delete the `"\nRestart the bandao-api process now so repair_checkin_status_drift reconciles checkin_user_status."` line at the end of a real run.
- [x] 3.3 Update the module doc comment at the top of `api/examples/legacy_backfill.rs` so it describes in-run reconciliation instead of deferring to a restart.
- [x] 3.4 Add a sentence to `DEPLOY.md`'s legacy-backfill section stating that each run reconciles `checkin_user_status` for the AppUsers it touched, so no API restart is needed after an import.

## 4. Tests

- [x] 4.1 Update the existing assertion block in `api/tests/legacy_backfill_import.rs` that reads `status_before_repair` and then calls `repair_checkin_status_drift` — after the import the status must already be `on_site`, with no manual repair call.
- [x] 4.2 Add an integration test: an AppUser whose `checkin_user_status` has drifted (row says `on_site` while the latest imported event is a `clock_out`) is corrected to `off_duty` with `current_shift_started_at = null` by a run in which every document is already present.
- [x] 4.3 Add an integration test: an AppUser whose only documents in the window route to `location_pings` is not reconciled, and an AppUser with no documents in the window is left untouched.
- [x] 4.4 Add an integration test: a dry-run over documents that would reconcile leaves `checkin_user_status` byte-for-byte unchanged.
- [x] 4.5 Assert `last_event_id` points at the latest imported event, not only that `status` matches, in at least one of the above.

## 5. Verification

- [x] 5.1 `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` all clean.
- [x] 5.2 Run the example against a scratch database with `--dry-run` and confirm the summary reports a would-reconcile count and writes nothing.
- [x] 5.3 Record in the change notes that deployment requires re-running `api/scripts/legacy_backfill_sync.sh`, since the LaunchAgent executes the copy in the deploy dir rather than the repo build.
