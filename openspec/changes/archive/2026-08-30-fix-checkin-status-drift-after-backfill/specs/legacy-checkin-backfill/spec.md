## MODIFIED Requirements

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

## ADDED Requirements

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
