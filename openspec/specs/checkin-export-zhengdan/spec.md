# checkin-export-zhengdan Specification

## Purpose
TBD - created by archiving change add-zhengdan-checkin-export. Update Purpose after archive.
## Requirements
### Requirement: A generic checkin-events export endpoint returns one day's clock_in/clock_out events as JSON

The system SHALL expose `GET /orgs/me/checkin/events/export`, resolving the Org from the authenticated API token's bound `org_id`. The system SHALL return every `clock_in` and `clock_out` event for that Org whose `occurred_at_client` falls within the requested day window as a JSON body containing `date`, the resolved `utc_offset`, and an `events` array of `{ app_user_display_name, event_type, occurred_at_client }`, ordered ascending by `occurred_at_client`. The system SHALL NOT include `transfer_out` or `transfer_in` events in this export, regardless of the day window. The response format SHALL NOT be specific to any downstream vendor's text format — vendor-specific formatting is the responsibility of the client consuming this endpoint.

#### Scenario: Default call returns today's clock events in UTC

- **WHEN** a request is made with no `date` and no `utc_offset` query parameter
- **THEN** the response includes every `clock_in`/`clock_out` event for the Org whose `occurred_at_client` falls within the current UTC calendar day (`[00:00, 24:00)` UTC)

#### Scenario: Transfer events are excluded regardless of date

- **WHEN** an AppUser has `transfer_out` and `transfer_in` events within the requested day window
- **THEN** neither event appears in the export output

#### Scenario: A day with no matching events returns an empty, successful response

- **WHEN** the requested day has zero `clock_in`/`clock_out` events for the Org
- **THEN** the response is `200 OK` with `events: []`, not an error

### Requirement: The day window is computed from a caller-supplied UTC offset, defaulting to UTC

The system SHALL accept an optional `utc_offset` query parameter in `+HH:MM`/`-HH:MM` format, defaulting to `+00:00` when omitted. The "day" boundary SHALL be computed as the caller's local `[00:00, 24:00)` at that offset, translated to the equivalent UTC instant range. The system SHALL accept an optional `date` (`YYYY-MM-DD`) query parameter selecting which calendar day to export; when omitted, the system SHALL compute "today" using its own current UTC clock shifted by the supplied `utc_offset` — it SHALL NOT trust a caller-supplied notion of the current date. A malformed `utc_offset` or `date` value SHALL be rejected with a validation error.

#### Scenario: A +08:00 offset shifts the day window relative to UTC

- **WHEN** a request includes `utc_offset=+08:00`
- **THEN** the returned day window corresponds to UTC `16:00` of the previous day through UTC `15:59:59` of the current day (i.e. local `00:00`–`23:59:59` at `+08:00`)

#### Scenario: Boundary event just inside the offset window is included

- **WHEN** an event's `occurred_at_client` is exactly UTC `15:59:59` on day D and the request uses `utc_offset=+08:00`
- **THEN** that event is included in the export for local day D (at `+08:00`)

#### Scenario: Boundary event just outside the offset window is excluded

- **WHEN** an event's `occurred_at_client` is exactly UTC `16:00:00` on day D and the request uses `utc_offset=+08:00`
- **THEN** that event is excluded from the export for local day D and instead belongs to local day D+1

#### Scenario: Explicit date parameter overrides the default, still respecting the offset

- **WHEN** a request includes `date=2026-07-10` and `utc_offset=+08:00`
- **THEN** the response includes only events whose `occurred_at_client` falls within local 2026-07-10 at `+08:00`, regardless of the current date

#### Scenario: Malformed offset is rejected

- **WHEN** a request includes a `utc_offset` value that is not a valid `+HH:MM`/`-HH:MM` string
- **THEN** the response is a `400` validation error

### Requirement: The export endpoint requires API-token authentication with the checkin:read scope

The system SHALL require a valid, active API token presenting the `checkin:read` scope to access this endpoint. The system SHALL NOT accept dashboard session-cookie authentication on this endpoint. A request without a valid `checkin:read`-scoped token SHALL be rejected.

#### Scenario: Request without a token is rejected

- **WHEN** a request to the export endpoint carries no `Authorization` header
- **THEN** the response is `401 Unauthorized`

#### Scenario: Dashboard session cookie alone does not grant access

- **WHEN** a request carries a valid dashboard admin session cookie but no API token
- **THEN** the response is `401 Unauthorized`

#### Scenario: Token without the checkin:read scope is rejected

- **WHEN** a request presents a valid, active API token that does not carry the `checkin:read` scope
- **THEN** the response is `403 Forbidden`

### Requirement: The accompanying Zhengdan export script renders the fixed-width text format client-side

The Zhengdan PowerShell client (delivered with this change, not part of `api/`) SHALL, for each event returned by the export endpoint, render one line consisting of: the event's `app_user_display_name` right-padded with spaces to a fixed width of 20 characters, immediately followed by `occurred_at_client` converted to `+08:00` and formatted as `YYYYMMDDHHmmss`, immediately followed by `上班` (for `clock_in`) or `下班` (for `clock_out`), with no separator between any of the three segments. The assembled document SHALL be written as UTF-8 without a byte-order mark, with `CRLF` line endings between rows and no trailing line terminator after the final row, preserving the ascending time order returned by the API.

#### Scenario: A clock_in event is rendered with correct padding and suffix

- **WHEN** the export API returns an event with `app_user_display_name = "郭文賓"`, `event_type = "clock_in"`, and `occurred_at_client` corresponding to `2026-07-07 06:47:44 +08:00`
- **THEN** the script's output line is `郭文賓` followed by 17 spaces (20 characters total for the name field), then `20260707064744`, then `上班`, with no other characters between these three segments

#### Scenario: Output file has no BOM and no trailing newline

- **WHEN** the script writes the assembled document to disk
- **THEN** the file's first bytes are not a UTF-8 byte-order mark
- **AND** the file ends immediately after the last row's content with no additional `CRLF` afterward

