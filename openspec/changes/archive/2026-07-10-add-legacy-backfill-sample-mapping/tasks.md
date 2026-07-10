## 1. Sample endpoint (api)

- [x] 1.1 Add `POST /orgs/me/legacy-backfill/sample` handler (admin-only) in `handlers/legacy_backfill.rs`: request `{ connection_string?, database, collection, query?: serde_json::Value, limit }`, connection string omitted/blank reuses the stored one (same convention as `build_config`) — via new `resolve_connection_string` helper
- [x] 1.2 Validate `query` (if present) is a JSON object (not array/scalar) before attempting any Mongo connection; reject with a validation error otherwise
- [x] 1.3 Extract/reuse the provider's connection-setup logic (from `services/legacy_backfill/provider.rs`) so `sample` and `preview`/`run_backfill` share the same connect-timeout/client-setup code path — extracted `connect_collection`, `fetch_and_map` now calls it too
- [x] 1.4 Implement the sample query: convert the validated JSON filter to a `bson::Document` (empty document when `query` is absent), run `collection.find(filter).limit(N)`, and return the raw matched documents converted to JSON (no field mapping applied) — `provider::sample_raw_documents`, dates/ObjectIds rendered as plain strings (not `$date`/`$oid` wrappers) via `bson_to_display_json` so scalar fields stay leaves when the frontend flattens paths
- [x] 1.5 Response shape mirrors `preview`'s connected/error reporting: `{ connected: bool, documents: Vec<serde_json::Value>, error: Option<String> }`

## 2. Field-flattening + drag-and-drop UI (admin-web)

- [x] 2.1 Add `sample()` to `useLegacyBackfill.ts` composable calling the new endpoint; extend `types/api.ts` with the request/response types
- [x] 2.2 Add a "採樣" button + optional query textarea/input to `pages/settings/legacy-backfill.vue`, positioned after the connection-info fields and before the field-mapping inputs
- [x] 2.3 Implement a client-side recursive flattener: given the returned raw documents, produce a unioned list of `{ path: string, value: unknown }` across all sampled documents (nested objects flattened with dot-paths; arrays treated as opaque leaf values per design D1's known limitation)
- [x] 2.4 Render the flattened fields as draggable chips (native HTML5 `draggable`/`dragstart`), each chip labeled `path · sample value`
- [x] 2.5 Make the identity/timestamp/lat/lng/region-name/manual-label/action-field inputs valid drop targets (`@dragover.prevent`/`@drop`): dropping a chip sets that input's `v-model` to the chip's dot-path, without removing the existing manual-typing capability — also added a `dragOverField` ring highlight for drop-target affordance
- [x] 2.6 Handle the connection-failure / empty-sample cases in the UI (mirroring the existing preview section's error display)

## 3. Docs & verification

- [x] 3.1 Confirmed `legacy-checkin-backfill` spec deltas (this change's `specs/legacy-checkin-backfill/spec.md`) accurately match the implemented endpoint/UI behavior — no drift found
- [x] 3.2 `cargo build` + `cargo fmt --all -- --check` + `cargo clippy --all-targets --all-features` clean; admin-web `nuxt typecheck` clean; full `cargo test --all-features --no-fail-fast` re-run after all changes passed with 0 failures across the entire suite
- [x] 3.3 Manual smoke via curl against a locally running api + a seeded fake legacy collection (`legacy_hr_sample.sbsigns`, matching the customer's real schema incl. a `"路徑"` ping doc): sampling with no query filter returned all 3 raw unmapped documents with dates/ObjectIds rendered as plain strings (not `$date`/`$oid` wrappers, confirming `bson_to_display_json`); sampling with `{"action": "上班"}` correctly narrowed to 1 matching document; submitting a non-object `query` (a JSON array) was rejected with `400 VALIDATION` before any connection attempt; an unreachable connection string correctly returned `connected: false` with a diagnostic `error`, no crash. All test data (fake DB, test Org/admin account, sessions touched during cleanup) removed afterward.
- [x] 3.4 Manual smoke: drag a sampled field onto each of the 7 mapping inputs and confirm the dot-path fills in correctly; confirm manually typing into a mapping input still works after a sample has been loaded — verified by the user directly in the browser at `/settings/legacy-backfill` against the real `klcc` Org's legacy MongoDB.
