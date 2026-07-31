## 1. Recent labels

- [x] 1.1 `recent_labels.dart` — pure `computeRecentLabels()` over the events the app already fetches. Confirmed `CheckinEventDto.location.manualLabel` is already parsed, so no API or model work was needed.
- [x] 1.2 Cached via `SecureStorage` (the app's only key-value layer), JSON-encoded under a new `checkin.recent_labels` key. A fetch that returns an empty list does NOT overwrite the cache — an empty 30-day window must not wipe suggestions the worker relies on.
- [x] 1.3 Six tests: frequency-over-recency ordering, the 6 cap against a 19-label fixture, the 30-day boundary, null/blank labels skipped, whitespace-trimmed duplicates collapsed, empty history.

## 2. Home screen — collecting the label

- [x] 2.1 `CheckinLabelField` widget, inserted directly above `HomeButtons` on the home screen.
- [ ] 2.2 Render the recent labels as one-tap options that fill the field without submitting. Check on a real device that 6 chips plus the field do not push the action buttons below the fold — that is the layout question `design.md` leaves open.
- [x] 2.3 Gated via `checkinLabelIsValidProvider`, composed with the existing `deniedForever` check rather than replacing it. No dialog added; a test asserts no `Dialog`/`BottomSheet` appears when the label is empty.
- [x] 2.4 `enqueueEvent` clears `checkinLabelProvider` after a successful enqueue; the widget mirrors that into its controller.
- [x] 2.5 Validated on **runes**, not UTF-16 code units, to match the server's `chars().count()`. They agree for CJK but not beyond the BMP, and disagreeing would let the device accept a label the server rejects.
- [x] 2.6 Added to the hand-rolled shim (zh + en) and all three ARB files. No `gen-l10n` step exists in this project.

## 3. Enqueue and submission

- [x] 3.1 One line in `checkin_actions.dart`, plus a defensive `labelMissing` outcome for the case where something bypasses the button gate — refusing rather than silently submitting an unlabelled event.
- [ ] 3.2 Verify end-to-end that the label reaches the server: enqueue, submit, and confirm the stored event carries it. The server needs no change, so this is a wiring check, not a feature.
- [ ] 3.3 Test that a row queued offline retains its label and submits it on reconnect with no further input.

## 4. History display

- [x] 4.1 Extracted as `formatLocationLine()` so the rule is testable without mounting the screen.
- [x] 4.2 A local row passes `regionName: null` — the server geocodes on receipt — so it lands in the label-only case and gains the region after sync.
- [x] 4.3 Six tests over `formatLocationLine`, covering all four combinations plus blank-string handling and the pending-row case.

## 5. Verification

- [x] 5.1 `flutter analyze lib test` clean; `flutter test` **242/242**.
- [ ] 5.2 On a real device: label an event, submit, confirm it appears in the app's history as `label, region` and on admin-web's event history — admin-web reads the same DTO and needs no change, so this also confirms that claim.
- [ ] 5.3 Confirm the chips reflect the AppUser's own history and not another user's, using an account with existing labelled events. KLCC accounts all have imported history, so this is testable immediately.
- [ ] 5.4 Confirm an offline enqueue works end to end: airplane mode, label, submit, restore connectivity, verify the label landed.
- [ ] 5.5 Sanity-check the friction this adds. Time a clock-in via chip against the current one-tap flow. If it is materially worse than the legacy system's flow, say so before shipping — this change puts a required field on the app's primary path and that decision deserves one honest look at the result.
