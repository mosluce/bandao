## 1. Recent labels

- [ ] 1.1 Add a source of recent labels for the current AppUser: last 30 days, ordered by frequency of use, capped at 6, derived from the events the app already fetches via `GET /app/checkin/events`. No new endpoint — confirm the existing fetch carries `location.manual_label` before writing any client code.
- [ ] 1.2 Persist the computed list on the device and serve it when the fetch fails or has not run. Offline check-in is a core scenario; a worker out of signal must still get their chips. Reuse the existing storage layer rather than introducing a new one.
- [ ] 1.3 Unit-test the derivation: frequency ordering (not recency), the 6 cap against a 19-label fixture, the 30-day window boundary, events with a null label excluded, and the cached list served when the source is unavailable.

## 2. Home screen — collecting the label

- [ ] 2.1 Add the label field above the action buttons in `app/lib/features/auth/presentation/home_screen.dart` (or a dedicated widget beside `home_buttons.dart`, whichever keeps the home screen readable — it is already long).
- [ ] 2.2 Render the recent labels as one-tap options that fill the field without submitting. Check on a real device that 6 chips plus the field do not push the action buttons below the fold — that is the layout question `design.md` leaves open.
- [ ] 2.3 Gate the action buttons on a non-empty label, composed with the existing status and `deniedForever` conditions rather than replacing them. Do NOT add any dialog, sheet or prompt — the tap→enqueue requirement is unmodified by this change and must stay true.
- [ ] 2.4 Clear the field after a successful enqueue.
- [ ] 2.5 Enforce 1–120 characters client-side to match the server's existing validation, so an over-long label fails on the device rather than at submit time.
- [ ] 2.6 Add the localized strings (field label, empty-state hint) to the hand-rolled `app_localizations.dart` shim in **both** zh and en, and to the ARB files. There is no `flutter gen-l10n` step in this project — see the header comment in that file.

## 3. Enqueue and submission

- [ ] 3.1 Populate `manualLabel` on the queue row in `checkin_actions.dart`. The column, the background-sync passthrough and the submit DTO all already exist and are already wired — this task is the one line that was missing.
- [ ] 3.2 Verify end-to-end that the label reaches the server: enqueue, submit, and confirm the stored event carries it. The server needs no change, so this is a wiring check, not a feature.
- [ ] 3.3 Test that a row queued offline retains its label and submits it on reconnect with no further input.

## 4. History display

- [ ] 4.1 Replace the location line in `history_screen.dart` with the `manual_label, region_name` rule, covering all four combinations including the existing coordinate fallback.
- [ ] 4.2 Show a local queue row's label immediately (it was supplied on this device) with the region arriving after sync.
- [ ] 4.3 Widget-test all four combinations plus the pending-row case.

## 5. Verification

- [ ] 5.1 `flutter analyze` and `flutter test` clean.
- [ ] 5.2 On a real device: label an event, submit, confirm it appears in the app's history as `label, region` and on admin-web's event history — admin-web reads the same DTO and needs no change, so this also confirms that claim.
- [ ] 5.3 Confirm the chips reflect the AppUser's own history and not another user's, using an account with existing labelled events. KLCC accounts all have imported history, so this is testable immediately.
- [ ] 5.4 Confirm an offline enqueue works end to end: airplane mode, label, submit, restore connectivity, verify the label landed.
- [ ] 5.5 Sanity-check the friction this adds. Time a clock-in via chip against the current one-tap flow. If it is materially worse than the legacy system's flow, say so before shipping — this change puts a required field on the app's primary path and that decision deserves one honest look at the result.
