## ADDED Requirements

### Requirement: Org can configure a legacy check-in data source

The system SHALL allow an admin to configure `settings.legacy_backfill` for their Org via `POST /orgs/me/legacy-backfill`: a MongoDB connection string (stored encrypted, write-only — never returned in any response), database, collection, an identity field (dot-path, matched against `AppUser.username`), a timestamp field (dot-path), latitude/longitude fields (dot-path), optional region-name and manual-label fields (dot-path), an action field (dot-path), and an action-value-to-`CheckinEventType` mapping table. The system SHALL reject saving a config where the identity/timestamp/lat/lng/action fields are empty or where any `action_map` value is not a valid event type.

#### Scenario: Valid config saves

- **WHEN** an admin submits a `legacy_backfill` config with all required fields non-empty and valid `action_map` values
- **THEN** the config is persisted to `settings.legacy_backfill`
- **AND** the connection string is stored encrypted, never returned in plaintext

#### Scenario: Missing required field is rejected

- **WHEN** an admin submits a config with an empty `identity_field`
- **THEN** the save is rejected with a validation error
- **AND** the config is not persisted

#### Scenario: Invalid action_map value is rejected

- **WHEN** an admin submits an `action_map` entry whose value is not one of the four event types
- **THEN** the save is rejected with a validation error

### Requirement: Admin can preview the legacy backfill mapping without writing data

The system SHALL provide `POST /orgs/me/legacy-backfill/preview` (admin-only) that connects to the legacy database using the submitted (possibly unsaved) configuration, fetches a small sample of documents, applies the field mapping and action mapping, and returns the resulting neutral-shape preview. This endpoint SHALL NOT write to `checkin_events`, SHALL NOT create or modify any `AppUser`, and SHALL NOT set `legacy_backfill_done_at` on any AppUser.

#### Scenario: Preview shows mapped sample without writing

- **WHEN** an admin submits a candidate config to the preview endpoint
- **THEN** the response contains a sample of documents mapped into the neutral event shape
- **AND** no `checkin_events` rows are created
- **AND** no AppUser's `legacy_backfill_done_at` changes

#### Scenario: Preview surfaces connection failures

- **WHEN** the submitted connection string cannot reach the legacy database
- **THEN** the preview response indicates the connection failed with a diagnostic
- **AND** no partial data is written

### Requirement: First successful login enqueues a one-time backfill job

When an Org has a `legacy_backfill` configuration and an authenticating AppUser's `legacy_backfill_done_at` is unset, the system SHALL, after the login response has been prepared (session issued, response body assembled), enqueue a `pending` job in `legacy_backfill_jobs` for that AppUser, keyed uniquely by `app_user_id` so a duplicate enqueue is a no-op. Enqueuing SHALL be a single fast write and SHALL NOT delay or alter the login response.

#### Scenario: First login enqueues a job

- **WHEN** an AppUser whose Org has `legacy_backfill` configured logs in for the first time (`legacy_backfill_done_at` is unset)
- **THEN** the login response is returned without waiting for any backfill work
- **AND** a `pending` job for that AppUser is created in `legacy_backfill_jobs`

#### Scenario: Subsequent logins do not enqueue a duplicate job

- **WHEN** an AppUser whose `legacy_backfill_done_at` is already set logs in again
- **THEN** no job is enqueued

#### Scenario: A login while a job is already pending or active does not enqueue another

- **WHEN** an AppUser logs in again while their existing job is still `pending` or `active`
- **THEN** no additional job is created for that AppUser

#### Scenario: Org without legacy_backfill configured never enqueues

- **WHEN** an AppUser logs in and their Org has no `legacy_backfill` configuration
- **THEN** no job is enqueued

### Requirement: A background worker processes backfill jobs with capped retry

The system SHALL run a persistent background worker (started once at process boot) that periodically claims one due `pending` job at a time via an atomic conditional update (matching only `pending` jobs whose `next_attempt_at` has elapsed, setting `status: active`), so concurrent workers cannot claim the same job. For a claimed job, the worker SHALL: connect to the Org's configured legacy database; query documents whose identity field matches the job's AppUser's `username`; map each document's action value via `action_map`, skipping (and counting) values with no mapping; build `CheckinEvent` rows using the timestamp field value as-is (`occurred_at_client`) and, when configured, the region-name/manual-label field values directly (no reverse-geocode call); insert the resulting rows into `checkin_events` without enforcing the live event-submission state-machine or ordering checks, logging (not blocking on) any sequence anomalies; and derive `checkin_user_status` from the AppUser's latest event using the same reconciliation used by the startup drift-repair task.

On success, the worker SHALL set the job's `status` to `done` and set `AppUser.legacy_backfill_done_at`. On failure, the worker SHALL increment the job's `attempts`, compute a backoff-delayed `next_attempt_at`, and set `status` back to `pending` — unless `attempts` has reached a fixed cap, in which case `status` SHALL be set to `failed` and the job SHALL NOT be retried automatically. A job left `active` past a staleness threshold (e.g. the worker process crashed mid-job) SHALL be treated as abandoned and reset to `pending` on a subsequent tick.

#### Scenario: A pending job is processed and marked done

- **WHEN** the worker claims a `pending` job whose legacy data is reachable and well-formed
- **THEN** the matched AppUser's legacy check-in history is inserted into `checkin_events`
- **AND** `checkin_user_status` is updated to reflect the latest imported event
- **AND** the job's `status` becomes `done` and `AppUser.legacy_backfill_done_at` is set

#### Scenario: A failed attempt retries with backoff, independent of login

- **WHEN** a claimed job's backfill attempt fails (e.g. cannot reach the legacy database)
- **THEN** the job's `attempts` is incremented and `status` returns to `pending` with a later `next_attempt_at`
- **AND** the retry happens on the worker's own schedule, not tied to the AppUser logging in again

#### Scenario: Exceeding the retry cap marks the job failed

- **WHEN** a job's `attempts` reaches the configured cap after repeated failures
- **THEN** the job's `status` becomes `failed`
- **AND** the worker does not attempt it again automatically

#### Scenario: An abandoned active job is recovered

- **WHEN** a job has been `active` for longer than the staleness threshold (e.g. the worker crashed mid-job)
- **THEN** a subsequent worker tick resets that job to `pending` so it can be retried

#### Scenario: Unmapped action values are skipped, not imported

- **WHEN** a legacy document's action value has no entry in `action_map`
- **THEN** that document is skipped and counted
- **AND** the rest of the AppUser's history is still processed

#### Scenario: Sequence anomalies do not block the backfill

- **WHEN** the legacy history contains an event sequence that would violate the live check-in state machine (e.g. two consecutive clock-ins)
- **THEN** the anomaly is logged
- **AND** the events are still inserted and the rest of the backfill continues

### Requirement: Admin can view legacy backfill job status

The system SHALL provide `GET /orgs/me/legacy-backfill/jobs` (admin-only, read-only) listing the current Org's `legacy_backfill_jobs` with their status, associated AppUser, attempt count, last error, and timestamps. This endpoint SHALL NOT provide any mutation (e.g. manual retry) in this iteration.

#### Scenario: Admin views job statuses

- **WHEN** an admin requests `GET /orgs/me/legacy-backfill/jobs`
- **THEN** the response lists every job for `current_org`, including `failed` jobs with their `last_error`

#### Scenario: Jobs from other Orgs are not visible

- **WHEN** an admin requests the jobs list
- **THEN** only jobs whose `org_id == current_org_id` are returned
