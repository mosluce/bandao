## 1. Domain model changes

- [x] 1.1 Add `EventSource::LegacyBackfill` variant to `api/src/domain.rs`
- [x] 1.2 Add `legacy_source_id: Option<ObjectId>` field to `CheckinEvent` (serde default, skip if none)
- [x] 1.3 Add `legacy_source_id: Option<ObjectId>` field to `LocationPing` (serde default, skip if none)

## 2. Database changes

- [x] 2.1 Remove the `location_pings_ttl` TTL index creation in `api/src/db/mod.rs`
- [x] 2.2 Add a partial unique index on `checkin_events.legacy_source_id` (`partialFilterExpression: { legacy_source_id: { $exists: true } }`)
- [x] 2.3 Add a partial unique index on `location_pings.legacy_source_id` (same partial filter pattern)
- [x] 2.4 Add repository methods for upserting a `CheckinEvent` / `LocationPing` keyed on `legacy_source_id` (`find_one_and_update` + `$setOnInsert` + `upsert: true`)

## 3. Legacy import script

- [x] 3.1 Scaffold `api/examples/legacy_backfill.rs` with CLI args: `--org-id`, `--legacy-uri`, `--legacy-domain`, `--since-days` (default 365), `--dry-run`
- [x] 3.2 Connect read-only to the legacy MongoDB and query the legacy `checkin_events`-shaped collection filtered by `domain` and `at >= now - since_days`
- [x] 3.3 Parse each legacy document's `action`, `at`, `signer.username`, `geo.lat`, `geo.lng`, `address`, `comment` fields
- [x] 3.4 Load all `AppUser` rows for `--org-id` and build a `username -> app_user_id` lookup
- [x] 3.5 Implement action routing: `上班/下班/轉出/轉入` → `CheckinEventType` + `checkin_events` upsert (`source = LegacyBackfill`, location built from `geo`/`address`/`comment`); `路徑` → `location_pings` upsert; anything else → skip
- [x] 3.6 Skip records whose `signer.username` has no matching AppUser; do not create AppUsers
- [x] 3.7 In dry-run mode, compute counts without performing any upsert
- [x] 3.8 Print a run summary: imported counts per event type, imported ping count, skipped-by-unmatched-username count, skipped-by-unrecognized-action count

## 4. Verification

- [x] 4.1 Unit test the action → target-collection routing logic (all 5 known action strings + one unknown string) — `src/services/legacy_backfill.rs` test module
- [x] 4.2 Unit test that re-processing the same legacy `_id` twice results in exactly one upserted row (idempotency) — `tests/legacy_backfill_import.rs::real_run_routes_skips_and_reruns_are_idempotent`
- [x] 4.3 Unit test the `since_days` window filter (record just inside vs. just outside the window) — `tests/legacy_backfill_import.rs::since_days_window_excludes_records_older_than_the_cutoff`
- [x] 4.4 Run the script in `--dry-run` against a seeded local legacy MongoDB fixture and confirm summary counts match the fixture — `tests/legacy_backfill_import.rs::dry_run_computes_summary_without_writing` (drives the same library functions the example binary calls, against a testcontainers Mongo fixture; `examples/` binaries aren't directly invokable from `cargo test`)
- [x] 4.5 Run the script for real against the fixture, restart the API locally, and confirm `repair_checkin_status_drift` produces the expected `checkin_user_status` for a seeded AppUser — `tests/legacy_backfill_import.rs::real_run_routes_skips_and_reruns_are_idempotent` (calls `repair_checkin_status_drift` directly rather than an actual process restart)
- [x] 4.6 Confirm `location_pings` documents older than 90 days are no longer deleted (TTL index absent) via `db.location_pings.getIndexes()` — `tests/location_tracking_ttl.rs` (also covers dropping a pre-existing TTL index from older deployments)
- [ ] 4.7 Manually spot-check imported data for a couple of AppUsers via the existing `/checkin` status board and `/checkin/[appUserId]/trajectory` admin-web pages — **requires a real legacy MongoDB connection and running admin-web instance; not automatable, left for the operator to do at actual cutover time**

## 5. Post-review fixes (found during live KLCC dry-run)

- [x] 5.1 Add `--legacy-collection` CLI flag (default `checkin_events`) — KLCC's actual legacy collection is named `sbsigns`, not `checkin_events`; querying a nonexistent collection silently returns zero documents (no error), which is exactly what an all-zero dry-run summary looked like. The design's "hardcode the field *shape*" decision didn't account for the collection *name* also varying per customer.
- [x] 5.2 Support `LEGACY_URI` / `LEGACY_DOMAIN` / `LEGACY_COLLECTION` env vars (via `.env`) as fallbacks when the corresponding CLI flag is omitted, so a customer's legacy connection string doesn't have to be typed on the command line / land in shell history
- [x] 5.3 Make `LegacySigner.username` `Option<String>` instead of required — real KLCC `sbsigns` documents (e.g. system-generated `路徑` pings) sometimes have `signer` present but no `username` sub-field at all. This previously failed the whole document's deserialization (flooding stderr with one warning line per document, uncounted); a missing username is now treated the same as "no matching AppUser". Added `RunSummary.skipped_malformed_document` (capped to the first `MAX_MALFORMED_WARNINGS` printed lines) so any *other* future deserialize failures stay visible in the summary instead of silently scrolling past — type-level tolerance covered by `services::legacy_backfill::tests::deserialize_tolerates_missing_signer_username` (superseded end-to-end coverage: see 5.5, such documents are now excluded before deserialization is even reached)
- [x] 5.4 Add `services::legacy_backfill::build_identity_map` and use it in place of matching on `AppUser.username` alone — KLCC's Org uses external-database auth, so all of its real AppUsers are shadow rows with `username = null` and only `external_key` set (the ERP `USERNO`, which is what the legacy system's `signer.username` was itself populated from). Matching on `username` alone left the identity map empty and skipped all 58,267 real documents as "unmatched username" on the first live dry-run against KLCC. The map now keys on `username` (internal-auth) falling back to `external_key` (external-auth shadow users). Spec updated (`specs/legacy-checkin-backfill/spec.md`, "AppUser matching is by username or external_key..."); covered by `tests/legacy_backfill_import.rs::external_auth_shadow_users_match_by_external_key` and three new unit tests on `build_identity_map`
- [x] 5.5 Push identity filtering into the MongoDB query itself (`legacy_query_filter` gained a `signer.username: { $in: known_identities }` clause) instead of fetching every document in the domain+time window and discarding unmatched ones client-side — KLCC's legacy collection has ~978K documents, the overwhelming majority belonging to people never onboarded into bandao; re-fetching and re-discarding all of them on every re-run (this script is meant to be re-run repeatedly during cutover) was wasteful. The script now also exits early with a clear error if the target Org has no AppUsers with a `username`/`external_key` to match against, rather than silently importing nothing. Client-side unmatched-username counting is kept as a defense-in-depth check (now expected to normally read 0, since the query already scopes to known identities) rather than removed. Spec and tests updated accordingly (`documents_missing_signer_username_are_excluded_by_the_identity_scoped_query`, `legacy_query_filter_shape`)
