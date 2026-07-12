## 1. Prerequisite check

- [x] 1.1 Confirmed: `add-org-api-tokens` is applied and archived (`openspec/specs/org-api-tokens/spec.md` exists on `main`), `ApiTokenScope::CheckinRead` exists in `src/domain.rs`. No delta needed.

## 2. Query layer (api)

- [x] 2.1 Added `src/services/utc_offset.rs`: `parse_offset` / `parse_date` / `today_at_offset` / `day_window_utc` — small standalone helpers, 6 unit tests independent of the handler (uses the `time` crate, matching the codebase's existing convention in `handlers::app_checkin::parse_rfc3339` rather than pulling in `chrono`)
- [x] 2.2 Added `list_by_org_in_range_for_export(org_id, day_start, day_end)` to `src/db/checkin_events.rs`: unpaginated, filters `event_type ∈ {ClockIn, ClockOut}` and `occurred_at_client ∈ [day_start, day_end)`, sorted ascending by `occurred_at_client`
- [x] 2.3 `tests/checkin_events_export_query.rs` (seeds directly via `db.checkin_events.create`, bypassing the state machine so arbitrary types/timestamps are easy to set up): transfer_out/transfer_in events in range are excluded; events outside the day window are excluded; events exactly at `day_start` are included, events exactly at `day_end` are excluded (half-open range)

## 3. Endpoint (api)

- [x] 3.1 `src/handlers/checkin_export.rs`: `GET /orgs/me/checkin/events/export` — optional `utc_offset` query param (default `+00:00`), optional `date` query param (`YYYY-MM-DD`, default "today" computed from the server's UTC clock shifted by `utc_offset`); compute `[day_start, day_end)` in UTC for the query
- [x] 3.2 Resolve org from `ApiTokenAuthContext`'s `org_id` (not from a session); require the `checkin:read` scope via `token.require_scope(...)`
- [x] 3.3 Fetch events via 2.2, join `AppUser.display_name` per `app_user_id` (single `list_by_org` batch fetch for the whole Org roster, not one query per event); return JSON: `{ date, utc_offset, events: [{ app_user_display_name, event_type, occurred_at_client }] }`, events sorted ascending by `occurred_at_client`. **No vendor-specific text formatting happens here** — that's the PowerShell client's job (see section 5).
- [x] 3.4 Empty-day case: zero events in range returns `200` with `events: []` (not a 404) — falls out naturally from the query/serialization, no special-casing needed
- [x] 3.5 Malformed `utc_offset` or `date` → `400` validation error (via `services::utc_offset`'s `ApiError::Validation`)
- [x] 3.6 Wired into a new `api_token_protected` router group in `src/handlers/mod.rs`, layered with `auth::api_token::api_token_require_session` — the first real consumer of that middleware/extractor

## 4. api integration tests

- [x] 4.1 `tests/checkin_export_endpoint.rs::default_date_returns_todays_events_at_offset_and_excludes_transfers_and_yesterday` — seeds clock_in/transfer_out/transfer_in ~1h ago and clock_out ~30h ago, calls with no `date` param + `utc_offset=+08:00`, asserts only the single today's clock_in comes back
- [x] 4.2 `utc_offset_boundary_is_half_open` — event at UTC 15:59:59 on 2026-07-10 included in `date=2026-07-10&utc_offset=+08:00`; event at UTC 16:00:00 excluded
- [x] 4.3 `default_offset_is_plain_utc_day` — no `utc_offset` param at all; response echoes `utc_offset: "+00:00"`; UTC 23:59:59 included, UTC 00:00:00 next day excluded
- [x] 4.4 `requires_a_bearer_token` (no header → 401), `token_without_checkin_read_scope_is_forbidden` (scopes cleared at the DB layer, since the create endpoint won't issue a zero-scope token → 403), `dashboard_session_cookie_is_not_accepted` (valid admin session cookie, no bearer → 401)
- [x] 4.5 `malformed_date_and_offset_are_rejected` (both → 400) and `explicit_past_date_returns_that_days_data` (2020-01-15 round-trips correctly)

## 5. PowerShell client (integrations/)

- [x] 5.1 New directory `integrations/zhengdan-checkin-export/` with `export.ps1`: reads a sibling (gitignored) `config.ps1` for `$ApiBaseUrl`, `$ApiToken`, `$TargetFolder`; calls `GET /orgs/me/checkin/events/export?utc_offset=%2B08:00` with `Invoke-RestMethod` using the bearer token
- [x] 5.2 Client-side formatting in `export.ps1`: for each event, `app_user_display_name.PadRight(20)` + `occurred_at_client` converted to `+08:00` via `[DateTimeOffset]::Parse(...).ToOffset(...)` and formatted `yyyyMMddHHmmss` + `上班`/`下班`, joined with `` `r`n `` (CRLF), no trailing CRLF. `$Lines` is wrapped in `@(...)` to dodge the PowerShell gotcha where a zero-iteration `foreach` assigned to a variable yields `$null` instead of an empty array (would have thrown in `[string]::Join`)
- [x] 5.3 Writes via `[System.IO.File]::WriteAllText($OutFile, $Content, (New-Object System.Text.UTF8Encoding($false)))` — BOM-safe idiom, `Out-File -Encoding utf8` / `Set-Content -Encoding UTF8` deliberately not used
- [x] 5.4 `$ErrorActionPreference = 'Stop'` + try/catch around the whole body: any non-2xx / timeout / network error / formatting exception lands in the `catch`, which only logs to `export.log` and exits 1 — the write-file step never runs on that path, so a failed run leaves `TargetFolder` untouched. A successful call with zero events still writes an (empty) file — that's a different, legitimate signal from "export failed"
- [x] 5.5 Target filename is `Get-Date -Format 'yyyyMMddHHmmss'` + `.txt`, computed at write time (local execution time)

## 6. PowerShell client docs

- [x] 6.1 `integrations/zhengdan-checkin-export/README.md`: API token setup/rotation via admin-web, config file setup, Task Scheduler registration (`powershell.exe -ExecutionPolicy Bypass -File export.ps1`, scoped to the one task rather than changing the machine-wide execution policy), the BOM-safe write idiom with the two common-mistake versions shown side by side, and the known 震旦雲-side name-matching-collision limitation called out explicitly
- [x] 6.2 `config.example.ps1` checked in as a template; `integrations/zhengdan-checkin-export/.gitignore` excludes `config.ps1` and `export.log`; README additionally recommends keeping the real `config.ps1` outside the repo checkout entirely on the actual deployment machine, not relying on `.gitignore` alone

## 7. Docs & verification

- [x] 7.1 `cargo test` (full suite) clean; `cargo clippy --all-targets` clean; `cargo fmt` applied
- [x] 7.2 No PowerShell interpreter available in this environment to literally run `export.ps1`, so verified the equivalent by: seeding the real (non-testcontainers) local dev API with two AppUsers/events matching the vendor sample's first two rows exactly (name + local +08:00 timestamp), fetching them via the live `GET /orgs/me/checkin/events/export` endpoint, then reproducing `export.ps1`'s exact algorithm (`PadRight(20)`-equivalent, `+08:00` conversion, CRLF join, UTF-8-no-BOM encode) in a throwaway script. Output was **byte-for-byte identical** to the corresponding lines in `/Users/mosluce/Downloads/20260712094513.txt`, including a confirmed absence of a BOM. Test data cleaned up afterward.
- [ ] 7.3 On-site / customer-coordinated smoke (blocking, needs the actual Windows Server 2016 Datacenter box): install the script, register the Task Scheduler job, confirm a file lands in the target folder within the hour and 震旦雲 successfully imports it without duplicate punches — **do not consider this change done until this smoke has actually run against the real machine**, per the earlier open question about that machine's PowerShell execution policy / antivirus posture
