# legacy-checkin-backfill Specification

## Purpose
TBD - created by archiving change add-legacy-backfill-windows-and-pings. Update Purpose after archive.
## Requirements
### Requirement: Legacy records are routed by action into checkin_events or location_pings

The system SHALL provide a developer-run, offline import script that reads
documents from a customer's legacy MongoDB `checkin_events`-shaped
collection (fields: `action`, `at`, `domain`, `signer.username`, `geo.lat`,
`geo.lng`, `address`, `comment`) and routes each document based on its
`action` field. The collection name SHALL be configurable via
`--legacy-collection` (default `checkin_events`) — the actual name varies
per customer's legacy deployment (e.g. KLCC's is `sbsigns`); querying a
nonexistent collection name returns zero documents without erroring, so
this MUST be verified against the customer's database rather than assumed.

- `action` in `{"上班", "下班", "轉出", "轉入"}` SHALL map to
  `CheckinEventType::{ClockIn, ClockOut, TransferOut, TransferIn}`
  respectively and be written to the `checkin_events` collection with
  `source = EventSource::LegacyBackfill`.
- `action == "路徑"` SHALL be written to the `location_pings` collection.
- Any other `action` value SHALL be skipped (not written, not treated as an
  error) and counted in the run summary.

#### Scenario: Clock-in action maps to checkin_events

- **WHEN** the script processes a legacy document with `action = "上班"`
- **THEN** a `checkin_events` row is inserted (or upserted, see idempotency
  requirement) with `event_type = clock_in` and `source = legacy_backfill`

#### Scenario: Path action maps to location_pings

- **WHEN** the script processes a legacy document with `action = "路徑"`
- **THEN** a `location_pings` row is inserted (or upserted) with `lat`/`lng`
  taken from the document's `geo.lat`/`geo.lng`

#### Scenario: Unrecognized action is skipped

- **WHEN** the script processes a legacy document whose `action` is not one
  of `上班`/`下班`/`轉出`/`轉入`/`路徑`
- **THEN** no row is written to either collection
- **AND** the run summary's skipped-action count increases by one

### Requirement: Imported rows are traceable via legacy_source_id and idempotent to re-run

The system SHALL store the legacy document's `_id` as `legacy_source_id` on
every row the script writes to `checkin_events` or `location_pings`. Both
collections SHALL have a partial unique index on `legacy_source_id` (only
enforced where the field is present). The script SHALL write using an
upsert keyed on `legacy_source_id` such that re-running the script against
the same legacy source data does not create duplicate rows.

#### Scenario: Re-running the script does not duplicate rows

- **GIVEN** the script has already imported a legacy document with
  `_id = X` into `checkin_events`
- **WHEN** the script is run again against the same legacy source data,
  including document `X`
- **THEN** no additional `checkin_events` row is created for `X`
- **AND** the existing row's `legacy_source_id` still equals `X`

#### Scenario: New legacy records since the last run are imported

- **GIVEN** the legacy system has new documents created after the script's
  previous run
- **WHEN** the script is run again with an overlapping or wider time window
- **THEN** the new documents are imported
- **AND** previously-imported documents are left unchanged (no duplicates)

### Requirement: AppUser matching is by username or external_key, unmatched records are skipped

The system SHALL resolve each legacy document's `signer.username` against
an identity map built from every `AppUser` in the target Org (identified by
`--org-id`): each AppUser's identity key is its `username` (internal-auth
AppUsers) or, when `username` is absent, its `external_key` (external-auth
shadow AppUsers, which carry no `username` at all). This matters in
practice, not just in theory — real external-auth Orgs' AppUsers are all
shadow rows with `username = null`; matching on `username` alone would
leave the identity map empty and skip every legacy document. The script
SHALL NOT create new AppUsers. When no matching AppUser exists for a
`signer.username` — including when `signer.username` itself is absent from
the legacy document — the record SHALL be skipped (not written, not
treated as a fatal error) and counted in the run summary.

#### Scenario: Matched username imports the record (internal-auth AppUser)

- **WHEN** a legacy document has `signer.username = "fang"`
- **AND** an internal-auth AppUser with `username = "fang"` exists in the
  target Org
- **THEN** the record is imported and attributed to that AppUser's
  `app_user_id`

#### Scenario: Matched external_key imports the record (external-auth shadow AppUser)

- **WHEN** a legacy document has `signer.username = "1001"`
- **AND** an external-auth AppUser with `username = null` and
  `external_key = "1001"` exists in the target Org
- **THEN** the record is imported and attributed to that AppUser's
  `app_user_id`

#### Scenario: Unmatched username is skipped and counted

- **WHEN** a legacy document's `signer.username` was returned by the query
  (see "Query is scoped to known AppUser identities") but somehow still
  fails to resolve to an AppUser in the identity map
- **THEN** the record is not imported
- **AND** the run summary's unmatched-username count increases by one
- **NOTE**: under normal operation this scenario should not occur — the
  query's `signer.username: { $in: known_identities }` clause means an
  unmatched document is not fetched in the first place. This check is
  defense-in-depth, not the primary filtering mechanism.

#### Scenario: Missing signer.username never reaches the deserializer

- **WHEN** a legacy document's `signer` object has no `username` sub-field
  at all (observed in real data — e.g. system-generated `路徑` pings with no
  resolved identity)
- **THEN** the document is excluded by the query's `$in` clause (a missing
  field cannot match any listed value) and is never fetched
- **AND** it is not counted anywhere in the run summary
- **AND** `LegacyCheckinDoc` deserialization SHALL still tolerate a missing
  `signer.username` as defense-in-depth, for any query that does not filter
  this way

### Requirement: Query is scoped to known AppUser identities

The system SHALL build the identity map (see `build_identity_map`) before
querying the legacy collection, and SHALL include a
`signer.username: { $in: <identity map keys> }` clause in the query filter
alongside the `domain` and `at` bounds — not fetch every document in the
domain+time window and discard unmatched ones client-side. This matters at
real scale: a legacy collection can hold far more documents than exist
AppUsers in bandao for that Org, and this script is meant to be re-run
repeatedly during cutover, so re-fetching and re-discarding the same
unmatched documents on every run is wasteful. When the identity map is
empty (the target Org has no AppUser with a `username` or `external_key`),
the script SHALL exit with an error before querying, rather than silently
running a query that can match nothing.

#### Scenario: Query excludes documents for people with no AppUser

- **GIVEN** a legacy collection with documents for both onboarded and
  not-yet-onboarded people
- **WHEN** the script queries the collection
- **THEN** only documents whose `signer.username` matches a known AppUser
  identity are returned by the query itself

#### Scenario: Empty identity map fails fast

- **WHEN** the target Org (`--org-id`) has zero AppUsers with a `username`
  or `external_key`
- **THEN** the script exits with an error before querying the legacy
  collection
- **AND** does not print a summary implying zero matches were found in the
  legacy data itself

### Requirement: Query window defaults to 365 days and is overridable

The script SHALL, by default, only read legacy documents whose `at` field
is within the last 365 days. The script SHALL accept a parameter to widen
or narrow this window.

#### Scenario: Default run only reads recent history

- **WHEN** the script is run without a window override
- **THEN** only legacy documents with `at >= now - 365 days` are read

#### Scenario: Override widens the window

- **WHEN** the script is run with an explicit window override greater than
  365 days
- **THEN** legacy documents older than 365 days but within the override
  window are also read

### Requirement: Dry-run mode reports without writing

The script SHALL accept a dry-run flag. In dry-run mode, the script SHALL
compute and print the same summary counts (matched/imported by action type,
skipped by unmatched username, skipped by unrecognized action) as a normal
run, but SHALL NOT write any rows to `checkin_events` or `location_pings`,
and SHALL NOT write to `checkin_user_status`.

#### Scenario: Dry-run produces no writes

- **WHEN** the script is run with the dry-run flag
- **THEN** the run summary is printed
- **AND** no rows are inserted or upserted into `checkin_events` or
  `location_pings`
- **AND** no `checkin_user_status` row is created, deleted, or updated

#### Scenario: Dry-run reports the reconciliation it would perform

- **WHEN** the script is run with the dry-run flag
- **THEN** the summary reports how many AppUsers a real run would reconcile

### Requirement: Documents that fail to deserialize are counted, not just logged

The system SHALL count every legacy document that fails to deserialize into
the expected shape (a required field other than `signer.username` is
missing or malformed) as `skipped_malformed_document` in the run summary.
The script SHALL print at most a small, fixed number of individual
deserialize-failure warnings per run (further occurrences are silently
counted, not printed) so a legacy collection with widespread schema drift
cannot flood the terminal with near-duplicate lines.

#### Scenario: Malformed documents are counted and summarized, not silently dropped

- **WHEN** the script encounters legacy documents that fail to deserialize
  for a reason other than a missing `signer.username`
- **THEN** each occurrence increments `skipped_malformed_document`
- **AND** the final run summary reports the total count
- **AND** individual warning lines stop printing after a fixed cap, with a
  single notice that further warnings are suppressed

### Requirement: Imported rows bypass live state-machine and ordering validation

The system SHALL write imported `checkin_events` rows directly at the
repository layer, without invoking the state-machine transition table or
the `OUT_OF_ORDER` strict-ordering check that gate `POST
/app/checkin/events`. After a non-dry-run import completes, the script
SHALL reconcile `AppUser`-level `checkin_user_status` from the event log
for every AppUser it routed at least one checkin document to during the
run, so that the projection agrees with the event log without requiring an
API process restart. The reconciliation SHALL reuse the same single-AppUser
primitive that the `repair_checkin_status_drift` startup routine applies
across the whole collection, rather than reimplementing the implied-status
derivation in the script.

#### Scenario: Historical events import regardless of transition legality

- **WHEN** the script imports a sequence of legacy events that would not be
  a legal state-machine transition sequence if submitted live (e.g. two
  consecutive `clock_in` actions)
- **THEN** all of the events are still written to `checkin_events`
- **AND** the script does not reject or reorder them

#### Scenario: Status is reconciled by the script before it exits

- **WHEN** a non-dry-run import writes checkin events for an AppUser
- **THEN** before the script exits, that AppUser's `checkin_user_status`
  has `status` equal to the status implied by their latest event by client
  time, and `last_event_id` equal to that event's id
- **AND** no API process restart is required for the status board to
  reflect the imported events

#### Scenario: Re-running over an already-imported window still reconciles

- **WHEN** a non-dry-run import processes a window in which every legacy
  checkin document was already imported by an earlier run (every upsert
  reports "already present")
- **THEN** the AppUsers those documents belong to are still reconciled
- **AND** a `checkin_user_status` row that had drifted from the event log
  is brought back in line

#### Scenario: Reconciliation is scoped to AppUsers seen in the run

- **WHEN** a non-dry-run import completes
- **THEN** only AppUsers that the run routed at least one
  `checkin_events`-bound document to are reconciled
- **AND** AppUsers whose only documents in the window routed to
  `location_pings` are not reconciled
- **AND** AppUsers with no documents in the window are left untouched

#### Scenario: A failed reconciliation does not fail the import

- **WHEN** reconciling one AppUser's status fails
- **THEN** the script prints a warning naming that AppUser
- **AND** continues reconciling the remaining AppUsers
- **AND** the import's own exit status is unaffected

### Requirement: Run summary reports reconciliation and does not instruct a restart

The run summary of a non-dry-run import SHALL report the number of AppUsers
whose `checkin_user_status` was reconciled. The script SHALL NOT instruct
the operator to restart the API process to reconcile status, because the
import is driven by an unattended scheduled job with no operator to act on
such an instruction, and reconciliation now happens in-run. Operational
documentation for the scheduled job SHALL NOT state that a restart is
required after an import.

#### Scenario: Summary includes a reconciled count

- **WHEN** a non-dry-run import completes
- **THEN** the printed summary includes the number of AppUsers reconciled

#### Scenario: No restart instruction is printed

- **WHEN** a non-dry-run import completes
- **THEN** the output contains no instruction to restart the API process

