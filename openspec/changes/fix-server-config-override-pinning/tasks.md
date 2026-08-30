## 1. Seeding

- [x] 1.1 Change `_seed()` so each input is set to the stored override when one exists and left empty when none does — drop the `Env.compileTimeDefault()` / `Env.privacyUrlCompileTimeDefault()` fallbacks from the text controllers
- [x] 1.2 Confirm the effective-URL labels above each field still render (they read from `effectiveBaseUrlProvider` / `effectivePrivacyUrlProvider`, not from the controllers), so nothing is hidden by the empty input
- [x] 1.3 Check the privacy field's helper/label copy — the base-URL field already says 留空即使用官方預設, and the privacy field needs equivalent wording now that blank is meaningful there too

## 2. Save and clear semantics

- [x] 2.1 In `_save()`, detect a blank trimmed input before validating and route it to the existing clear path
- [x] 2.2 Do NOT compare the input against `Env.compileTimeDefault()` — a typed value is stored as entered, even when it equals the default. The blank path from 2.1 is the only protection needed, and it is sufficient because an unedited field is blank
- [x] 2.3 Keep the bearer-token comparison in `_save()` against the current **effective** URL, and make sure the blank/clear path from 2.1 runs through it too — clearing an active override is a server change and must drop the session
- [x] 2.4 Apply 2.1 to `_savePrivacy()`
- [x] 2.5 Verify `_clear()` and `_clearPrivacy()` leave the inputs empty afterwards rather than re-seeding them with the compile-time default

## 3. Privacy override validation parity

- [x] 3.1 Replace `_savePrivacy()`'s inline scheme+authority check with `validateBaseUrlOverride`, mapping both error cases to the same messages the base-URL field shows
- [x] 3.2 Confirm `validateBaseUrlOverride` still rejects `""` as malformed and gains no empty-string special case — blank is handled by the callers from group 2

## 4. Tests

- [x] 4.1 `server_config_screen_test.dart`: input renders empty when no override is stored, and renders the stored value when one is
- [x] 4.2 Open-and-save-without-editing writes no override — the regression this change exists to prevent
- [x] 4.3 Blank save clears an existing override, for both the base URL and the privacy URL
- [x] 4.4 Typing the exact compile-time default and saving DOES persist it, and reopening the screen shows it in the field — the override is visible, not silently discarded
- [x] 4.5 Saving a genuinely different URL still persists it and still clears the bearer token
- [x] 4.6 `api_base_url_test.dart`: assert `validateBaseUrlOverride("")` is malformed, pinning the D3 decision
- [x] 4.7 Privacy override rejects `http://` in release mode and accepts it in debug, mirroring the existing base-URL cases

## 5. Verification

- [x] 5.1 `flutter analyze`
- [x] 5.2 `flutter test`
- [x] 5.3 Run the app and confirm: fields start empty, saving untouched changes nothing, entering a custom URL still works, clearing returns to the default
- [x] 5.4 On a device carrying a pre-existing default-equal override, confirm the field now shows that value (making the pin visible) and that clearing it takes one tap
