## Why

KLCC's workers labelled every single check-in in the system they came from. Of the 10,239 records imported by `legacy-backfill`, **10,239 carry a `manual_label`** — a 100% fill rate, which is only explicable as a required field there. The labels are site and customer names, typed by the workers themselves.

The Bandao app cannot produce one. The plumbing is entirely in place and has been since the beginning — `pending_events.manual_label` column, the background-sync passthrough, `SubmitCheckinEventRequest.manual_label`, server validation at 1–120 characters — but **nothing in the UI ever sets it**. Every event the app has created carries `manual_label: null`.

So the app is a regression against the system it replaced, on a field these workers fill every day. Their history reads `甲工地, 高雄市鳳山區頂庄路`; anything they record through the app reads as coordinates.

The history view has the matching gap: it renders `region_name` when present and falls back to coordinates, and never shows the label at all — so even the imported records display only half of what the worker wrote.

## What Changes

- **Collect a location label on every check-in.** A required field above the action buttons on the home screen, applying to whichever of `[上班] [下班] [轉出] [轉入]` is pressed. Cleared after each submission, so every event is labelled deliberately rather than inheriting a stale value.
- **Gate the action buttons on it rather than interrupting the tap.** The existing requirement that nothing may appear between the tap and the enqueue stays intact and unmodified — the buttons are simply disabled until a label exists, the same pattern the login form already uses for its three fields. There is no dialog, sheet, or confirmation step anywhere in the flow.
- **Offer the worker's own recent labels as one-tap chips.** Derived from their own history, which the API already returns. The median AppUser has used 7 distinct labels ever, and a handful dominate, so the common case becomes one tap instead of typing Chinese on a phone. Free text is always available — the legacy system had no managed list and this change does not introduce one.
- **Show the label in the history timeline**, as `manual_label, region_name`, degrading cleanly when either is absent.

**Explicitly not in scope**: any server change. `manual_label` is already specified, accepted, validated and stored; `POST /app/checkin/events` needs no modification. Also out of scope: an admin-managed list of sites. The legacy system used free text, so requiring workers to pick from a predefined set would be a behaviour change, not a port — and it would need admin tooling that nothing has asked for.

**A known consequence, accepted deliberately**: this puts a required field on the primary path. A worker who cannot supply a label cannot clock in. That is a real cost, and it is the reason the recent-label chips are part of this change rather than a follow-up — without them the requirement would be punitive. The 100% legacy fill rate is the evidence that the workforce already works this way.

## Capabilities

### New Capabilities

(none — all changes extend existing capabilities)

### Modified Capabilities

- `app-checkin`: the home action buttons gain the label as a second gating condition alongside location permission and checkin status; a new requirement covers collecting the label and offering recent values; the enqueue requirement records that the label travels with the queue row; and the history requirement gains the `manual_label, region_name` display rule.

## Impact

- **app (`app/`)**: the home screen (new field + chips above the buttons), `checkin_actions.dart` (populate the queue row's existing `manualLabel` column), a source of recent labels derived from the events the app already fetches, and `history_screen.dart` (the location line). Localized strings for the field label and its empty state.
- **No API, admin-web, or database change.** `pending_events.manual_label` already exists, so there is no drift migration either.
- **admin-web already displays what it needs** — the event history and trajectory markers read `location.manual_label` from the same DTO. Labels created by the app start appearing there with no work.
- **Behavioural change for existing app users**: clocking in becomes two taps rather than one (pick a chip, then press). For KLCC this restores the flow they had; for any internal-auth Org using the app today it is new friction.
