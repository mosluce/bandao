## ADDED Requirements

### Requirement: Admin can sample raw legacy documents before mapping fields

The system SHALL provide `POST /orgs/me/legacy-backfill/sample` (admin-only) that connects to the legacy database using only connection information (connection string, database, collection, and an optional raw MongoDB query filter) — with no field-mapping configuration required — and returns a small set of raw, unmapped documents from the collection. This endpoint SHALL NOT apply any field mapping, SHALL NOT require `identity_field`/`timestamp_field`/`lat_field`/`lng_field`/`action_field` to be set, and SHALL NOT write to `checkin_events` or mutate any AppUser.

#### Scenario: Sampling with connection info only returns raw documents

- **WHEN** an admin submits a connection string, database, and collection to the sample endpoint with no query filter and no field-mapping values
- **THEN** the response contains up to the requested limit of raw documents from that collection, unmodified by any field mapping
- **AND** no `checkin_events` rows are created and no AppUser is changed

#### Scenario: An optional query filter narrows the sample

- **WHEN** an admin submits a valid MongoDB query filter (e.g. matching a known person's identifying field) along with the connection info
- **THEN** the returned sample only contains documents matching that filter

#### Scenario: An invalid query filter is rejected

- **WHEN** an admin submits a query filter that is not a valid JSON object (e.g. a JSON array or scalar)
- **THEN** the request is rejected with a validation error and no connection attempt to the legacy database is made for that filter

#### Scenario: Sampling surfaces connection failures

- **WHEN** the submitted connection string cannot reach the legacy database
- **THEN** the sample response indicates the connection failed with a diagnostic, matching the existing preview endpoint's failure-reporting shape

### Requirement: Admin can assign field mapping by dragging sampled fields

The settings page SHALL let an admin populate the identity/timestamp/lat/lng/region-name/manual-label/action field inputs either by typing a dot-path directly or by dragging a field discovered from a sample onto the corresponding input, which fills that input with the field's dot-path. Sample-derived fields are computed client-side from the raw documents returned by the sample endpoint (flattening each document into dot-path/value pairs, unioned across all sampled documents to account for sparse fields). Manual typing SHALL remain available at all times, including for fields absent from the current sample.

#### Scenario: Dragging a sampled field fills a mapping input

- **WHEN** an admin drags a field chip derived from a sample (e.g. `signer.username`) onto the identity-field input
- **THEN** that input's value becomes the dragged field's dot-path

#### Scenario: Manual typing still works after sampling

- **WHEN** an admin has sampled documents and the field they need does not appear in the sample (e.g. a sparse optional field)
- **THEN** the admin can still type the dot-path directly into the corresponding input
