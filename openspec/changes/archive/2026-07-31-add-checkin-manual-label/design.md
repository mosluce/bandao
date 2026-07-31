## Context

`manual_label` is the one part of this feature that does not need building. It has been specified, accepted, validated and persisted since the beginning:

```
home UI          →  ✗ nothing sets it        ← the entire gap
pending_events   →  ✓ manualLabel column
background_sync  →  ✓ passes it through
SubmitDto        →  ✓ manual_label
POST /app/checkin/events → ✓ validated 1–120 chars, stored on location
```

What the field means in practice comes from the imported data, not from a design document:

| | |
|---|---|
| Fill rate on 10,239 legacy records | **100%** |
| Distinct labels org-wide | 105 |
| Distinct labels per AppUser | median **7**, max 19 |
| Share held by the single most-used label | ~40% of all records |
| Days where every event shares one label | **83.3%** (3,898 of 4,677) |

They are customer and site names. They are also visibly inconsistent — one AppUser's list contains three different spellings of the same place. That drift is what free text costs, and it is a cost the operator has chosen to keep (see Decisions).

Two existing requirements constrain the shape of any solution:

- `app-checkin`, tap→enqueue: *"The system SHALL NOT show a confirmation dialog, modal, or bottom sheet between the tap and the enqueue."*
- `app-checkin`, home buttons: the visible set is derived from effective checkin status, and buttons are disabled while location permission is `deniedForever`.

## Goals / Non-Goals

**Goals:**

- Every event the app creates carries a label, matching the system it replaced.
- The tap that submits a check-in stays instantaneous, with nothing between it and the enqueue.
- Re-entering a label is normally one tap, not typing.
- History shows what the worker wrote, alongside where the server thinks they were.

**Non-Goals:**

- Any server-side change. The endpoint, validation and storage already exist.
- An admin-managed list of sites. That is a behaviour change from the legacy system and needs tooling nobody has asked for.
- Normalising the existing label drift — the same place spelled several ways. Rewriting imported history is a separate, riskier proposition.
- Editing a label after the fact. `checkin_events` is append-only with no update endpoint; adding one is its own change.

## Decisions

### Gate the buttons instead of interrupting the tap

The no-dialog requirement is not incidental — clocking out at the end of a shift should be one press. Any "tap → sheet → type → confirm" flow breaks it.

Instead the label is collected **before** the tap, in a field above the buttons, and the buttons are disabled while it is empty. The tap→enqueue path is untouched: nothing appears between them, so that requirement needs no modification at all. This is the same gating the login form already applies to its three fields.

Rejected: a bottom sheet after the tap. It directly contradicts a shipped requirement, and it makes the slowest moment of the day (clock-out) slower still.

Rejected: making the label optional and prompting only when empty. That is a dialog by another name, and it would reproduce the very drift the operator is used to living with while adding an interruption.

### Clear the field after every submission

The operator's instruction was that every check-in is filled in afresh. The data does not obviously demand it — 83.3% of days use a single label throughout, so a sticky value would be correct most of the time — but the failure mode of stickiness is silent and expensive: a worker who moves to a different site and forgets to change the field produces confidently wrong records, and nothing in the UI would flag it.

Clearing makes the cost visible and bounded (one extra tap per event, usually on a chip) and makes every label a deliberate act. The 16.7% of days that visit two sites are exactly the ones stickiness would corrupt.

### Recent labels come from the worker's own history

No new endpoint: `GET /app/checkin/events` already returns `location.manual_label`, and the history screen already fetches it.

The list is the AppUser's own labels from the last 30 days, ordered by frequency, capped at 6. Frequency rather than recency so the dominant site stays in a stable position and builds muscle memory; a cap of 6 because the median worker has only 7 distinct labels ever, so 6 covers nearly everything without turning into a scrolling list.

The chips are a shortcut, never a constraint — free text remains available, matching the legacy system. A worker at a brand-new site must be able to type its name.

### Cache the recent labels for offline use

Offline check-in is a core scenario of this app — the whole queue exists for it. A worker underground or out of signal must still be able to label an event, and the recent list is derived from a network fetch.

The computed list is therefore persisted locally and used whenever the fetch is unavailable or stale. A cold install with no history and no signal falls back to free text only, which is the same position the legacy system's users started from.

### Display as `manual_label, region_name`

Two independent fields, either of which may be absent:

| `manual_label` | `region_name` | Shown |
|---|---|---|
| `甲工地` | `高雄市鳳山區頂庄路` | `甲工地, 高雄市鳳山區頂庄路` |
| `甲工地` | — | `甲工地` |
| — | `高雄市鳳山區頂庄路` | `高雄市鳳山區頂庄路` |
| — | — | `22.6116, 120.3006` |

The last row is the existing fallback and stays. It is also what a not-yet-synced local queue row shows, because the server has not reverse-geocoded it yet — the label the worker just typed is available locally, so a pending row can show `甲工地` immediately and gain the region once it syncs.

`region_name` is 98.8% populated in practice (imported records take it from the legacy system's own `address`; app-created ones from reverse geocoding, which is fail-soft), so the two-part form is what workers will almost always see.

## Risks / Trade-offs

- **A required field sits on the primary path.** A worker who cannot supply a label cannot clock in — a real cost on the app's most important action. Mitigated by the chips making the normal case one tap, and evidenced by the 100% legacy fill rate showing this workforce already works this way. Called out in the proposal rather than buried.

- **Existing internal-auth Orgs get new friction.** They have never had this field and did not ask for it. This change makes it required for everyone. If that proves wrong, the natural retreat is an Org-level toggle mirroring `transfer_enabled` — deliberately not built now, because a toggle nobody has asked for is its own liability.

- **The chips will entrench the existing drift.** Surfacing a near-duplicate spelling as a one-tap option makes it more likely to be reused, not less. Accepted: normalisation is out of scope and would mean rewriting imported history. Worth revisiting if the operator later wants a managed list.

- **Offline cold start has no chips.** A brand-new install with no signal offers only free text. Acceptable — it matches where the legacy system's users began, and it self-resolves after the first sync.

## Migration Plan

No data or API migration. `pending_events.manual_label` already exists, so no drift schema change and no queue rows to backfill.

Rows already queued when the new build lands carry `manualLabel: null`; they submit exactly as they do today, since the server treats the field as optional. There is no in-flight breakage.

Rollback is shipping the previous build; nothing written is rendered unreadable by it.

## Open Questions

- **Should transfers reuse the label of the shift they belong to?** The operator asked for transfers to carry their own label, which this design does by clearing the field for every event. Whether a transfer should *default* to the current shift's label — still editable, but pre-filled — is a refinement the field data cannot settle: the 16.7% multi-site days say the site genuinely changes at transfers, but they do not say how often it stays the same.
- **Is 6 chips the right cap on a small phone?** Median usage is 7 distinct labels, so 6 was chosen to cover nearly everything. Whether 6 fits above the action buttons without pushing them below the fold is a layout question to settle on a device, not on paper.
