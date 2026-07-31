## 1. Recent labels

- [x] 1.1 `recent_labels.dart` — pure `computeRecentLabels()` over the events the app already fetches. Confirmed `CheckinEventDto.location.manualLabel` is already parsed, so no API or model work was needed.
- [x] 1.2 Cached via `SecureStorage` (the app's only key-value layer), JSON-encoded under a new `checkin.recent_labels` key. A fetch that returns an empty list does NOT overwrite the cache — an empty 30-day window must not wipe suggestions the worker relies on.
- [x] 1.3 Six tests: frequency-over-recency ordering, the 6 cap against a 19-label fixture, the 30-day boundary, null/blank labels skipped, whitespace-trimmed duplicates collapsed, empty history.

## 2. Home screen — collecting the label

- [x] 2.1 `CheckinLabelField` widget, inserted directly above `HomeButtons` on the home screen.
- [x] 2.2 Chips render below the field. Verified on device: two chips plus the field leave both action buttons visible without scrolling, so the layout question `design.md` raised is settled.
- [x] 2.3 Gated via `checkinLabelIsValidProvider`, composed with the existing `deniedForever` check rather than replacing it. No dialog added; a test asserts no `Dialog`/`BottomSheet` appears when the label is empty.
- [x] 2.4 `enqueueEvent` clears `checkinLabelProvider` after a successful enqueue; the widget mirrors that into its controller.
- [x] 2.5 Validated on **runes**, not UTF-16 code units, to match the server's `chars().count()`. They agree for CJK but not beyond the BMP, and disagreeing would let the device accept a label the server rejects.
- [x] 2.6 Added to the hand-rolled shim (zh + en) and all three ARB files. No `gen-l10n` step exists in this project.

## 3. Enqueue and submission

- [x] 3.1 One line in `checkin_actions.dart`, plus a defensive `labelMissing` outcome for the case where something bypasses the button gate — refusing rather than silently submitting an unlabelled event.
- [x] 3.2 Verified against prod: 9 app-created events now carry labels (`你家 / 全家 / 公司 / 我家 / 測試`). Server needed no change, as claimed.
- [x] 3.3 Verified on device in airplane mode: label retained through the queue and submitted on reconnect.

## 4. History display

- [x] 4.1 Extracted as `formatLocationLine()` so the rule is testable without mounting the screen.
- [x] 4.2 A local row passes `regionName: null` — the server geocodes on receipt — so it lands in the label-only case and gains the region after sync.
- [x] 4.3 Six tests over `formatLocationLine`, covering all four combinations plus blank-string handling and the pending-row case.

## 5. Verification

- [x] 5.1 `flutter analyze lib test` clean; `flutter test` **249/249**.
- [x] 5.2 Verified. App history shows `地點, 地址` on every row. admin-web needed no change — it already reads `location.manual_label` in three places (checkin board, per-user events, and a label≠region conditional), so app-created labels appear there automatically.
- [x] 5.3 Verified. Chips showed the AppUser's own prior labels. **This is where a real defect surfaced**: chips only appeared after an app restart, because suggestions were derived purely from a server fetch and a just-enqueued event is still in the local queue. Fixed by recording the label on-device at enqueue time — see the `remember()` path and its seven tests.
- [x] 5.4 Verified end to end in airplane mode.
- [x] 5.5 Checked on device and judged acceptable: with a chip present a check-in is two taps against the previous one. The chips are what make that true, which is why they were scoped into this change rather than deferred.
