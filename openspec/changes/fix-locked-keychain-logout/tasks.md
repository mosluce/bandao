## 0. Pre-flight cleanup

- [x] 0.1 Revert the diagnostic instrumentation changes (these were temporary; the rest of the implementation must be applied on a clean baseline):
  - `app/lib/features/checkin/data/location_ping_processor.dart`: `_countThreshold` back to `30`.
  - `app/lib/features/checkin/data/location_tracking_service.dart`: `distanceFilter` back to `100`, `_throttle` back to `Duration(seconds: 60)`.
  - `app/lib/core/api/api_client.dart`: re-add the `if (kDebugMode)` gate around `dio.interceptors.add(AppLogInterceptor());`.
  - `app/lib/core/api/auth_interceptor.dart`: remove the diagnostic `print('[AUTH] ...')` line.
- [x] 0.2 Confirm `git diff app/` shows no remaining diagnostic-only changes, only the originally-staged `pubspec.yaml` build bump (`0.3.0+5`) plus whatever this change adds in §1.

## 1. Implementation — `SecureStorage` token cache + iOS Keychain accessibility

- [x] 1.1 Update `app/lib/core/storage/secure_storage.dart` constructor to default the underlying `FlutterSecureStorage` with `IOSOptions(accessibility: KeychainAccessibility.first_unlock)` when no storage is injected. Imports may need `import 'package:flutter_secure_storage/flutter_secure_storage.dart';` already present — verify.
- [x] 1.2 Add `String? _cachedToken;` and `bool _tokenLoaded = false;` private fields.
- [x] 1.3 Refactor `readToken()` to short-circuit on `_tokenLoaded` and otherwise read Keychain once, populate the cache, set `_tokenLoaded = true`, and return.
- [x] 1.4 Refactor `writeToken(String)` to write to Keychain first, then set `_cachedToken = token; _tokenLoaded = true;` (atomic ordering: persist before announcing the new value via the cache).
- [x] 1.5 Refactor `clearToken()` to delete from Keychain first, then set `_cachedToken = null; _tokenLoaded = true;` (cached "we know it's null" is a valid cached state).
- [x] 1.6 Add a class-level dartdoc note on `SecureStorage` documenting the invariant: all reads/writes/clears of `auth.bearer_token` MUST go through this wrapper; direct `FlutterSecureStorage` access for that key is forbidden so the cache cannot drift.

## 2. Tests

- [x] 2.1 Add a unit test in `app/test/core/storage/secure_storage_test.dart` (create file if absent) that injects a fake `FlutterSecureStorage`, calls `readToken()` twice, and asserts the underlying fake's read counter is `1` (the second read must hit the cache).
- [x] 2.2 Add a unit test that calls `writeToken('abc')` then `readToken()` and asserts `'abc'` is returned, even when the underlying fake is configured to throw on subsequent reads (proves cache is used).
- [x] 2.3 Add a unit test for the `clearToken()` → `readToken()` sequence: assert `null` is returned, the underlying fake's delete was invoked, and a second `readToken()` does not hit Keychain (cached null).
- [x] 2.4 Add a unit test that constructs `SecureStorage()` with no injected storage and asserts the resulting `FlutterSecureStorage` was configured with `KeychainAccessibility.first_unlock` (use whatever introspection the package allows, or extract the `IOSOptions` into a `@visibleForTesting` constant if cleaner).
- [x] 2.5 Run `cd app && flutter test test/core/storage/secure_storage_test.dart` and confirm all new tests pass.

## 3. Manual verification — TestFlight smoke

- [x] 3.1 Bump `app/pubspec.yaml` build number (current diagnostic build will be `0.3.0+5`; this fix ships as the next bump, e.g. `0.3.0+6`). The `release_ios.sh` script does this automatically — accept its bump.
- [x] 3.2 Run `cd app && ./scripts/release_ios.sh` to build the IPA with prod URL baked in and upload to App Store Connect via altool. Confirm the script reports "UPLOAD SUCCEEDED".
- [x] 3.3 Wait 10–30 minutes for Apple to finish processing the build; confirm it appears under App Store Connect → 班到 → TestFlight → Builds in a "Ready to Test" state.
- [x] 3.4 Install the new build on the same iPhone (iOS 26.x) used for the original repro via TestFlight app.
- [x] 3.5 Smoke the locked-background scenario:
  - Login → 上班 → confirm the location tracking chip is counting up.
  - Lock the device (side button) and leave the app backgrounded for at least 5 minutes; do not move the phone.
  - Unlock and tap the app icon to resume.
  - Verify the user is still on `/` (home), NOT on `/login`.
- [x] 3.6 Repeat the same scenario but this time walk during the locked-background window (≥5 minutes, ≥300m). Verify still on `/` after resume.
- [x] 3.7 Smoke the cold-launch case: force-quit the app (swipe up in App Switcher), lock the device, wait 30+ seconds, unlock, reopen the app — confirm the user is still authenticated (lands on `/` not `/login`).
- [ ] 3.8 _(deferred)_ Sanity-check Android: install a debug build on an Android device (or emulator), repeat the foreground login → background → resume flow, confirm no regression. Android does not exercise the Keychain accessibility path but does exercise the in-memory cache. _Deferred to next time the operator has an Android device to hand; the change does not touch Android-specific paths so regression risk is low._

## 4. Cleanup and documentation

- [x] 4.1 If the diagnostic prints / interceptor gate revert in §0 was committed separately, ensure it is reverted before merge so the shipping diff is just the `SecureStorage` change. (Reverts done in §0; nothing was committed separately, single working-tree change.)
- [x] 4.2 Update `app/CHANGELOG.md` (or root `CHANGELOG.md`, whichever is canonical for app-side entries) with a note under the relevant version: "Fix: bearer token now survives iOS device-lock during background sync; eliminates spurious logout when the user moved during a locked-screen background period." (entered under root `CHANGELOG.md` § App / [0.3.0+7])
- [x] 4.3 Run `openspec validate fix-locked-keychain-logout` and confirm the change is valid before requesting review.
- [ ] 4.4 Open a PR; in the description, link to this OpenSpec change and include the reproduction log evidence (`token=NULL` after `Cmd+L`) as the smoking gun.
