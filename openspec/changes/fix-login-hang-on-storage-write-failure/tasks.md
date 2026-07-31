## 1. Storage layer

- [x] 1.1 Add a typed `SecureStorageFailure` in `app/lib/core/storage/secure_storage.dart` carrying the key name and the underlying platform error, so callers can act on it and reports identify which key failed.
- [x] 1.2 Wrap every write in `SecureStorage` (`writeToken`, `writeLastOrgCode`, `writeApiBaseUrlOverride`, `markBackgroundSyncTipSeen`, `writeLocationTrackingLastCleanStop`, and the location-consent writer) so a `PlatformException` becomes a `SecureStorageFailure`. No raw plugin exception may escape the wrapper.
- [x] 1.3 Wrap every delete (`clearToken`, `clearLastOrgCode`, `clearApiBaseUrlOverride`, `clearLocationTrackingLastCleanStop`, and any consent clear) the same way. Deletes must not be able to break logout — the in-memory cache is cleared regardless of whether the platform delete succeeded.
- [x] 1.4 Keep `_safeRead` returning `null` on failure (the app must still boot) but stop discarding the exception — route it to the reporting sink added in 1.5.
- [x] 1.5 Report every secure-storage failure, read and write, through the existing `firebase_crashlytics` integration as a non-fatal, including the key name and platform error code. This is what makes a broken keystore diagnosable without a tethered debug build — the thing this investigation lacked.
- [x] 1.6 Unit-test the wrapper with a fake that throws on write / delete / read: writes surface `SecureStorageFailure`, deletes still clear the in-memory cache, reads still resolve absent, and every case reaches the reporting sink.

## 2. Auth state machine

- [x] 2.1 Removed the `state = data(AuthState.loading())` flip from `login()`. **Grep result**: the only consumer of `AuthLoading` is `router.dart:124` (the bootstrap window); the only other setters were `retry()` (bootstrap re-run — kept) and `login()` itself. Safe to remove.
- [x] 2.2 A failed `writeToken` no longer fails the login: the session becomes `authenticated` and `pendingSessionNotPersistedProvider` is raised for the UI.
- [x] 2.3 A failed `writeLastOrgCode` is caught and ignored for the session (reported by the storage layer, no user-facing notice).
- [x] 2.4 Added a catch-all to `login()` that resolves to `unauthenticated` and rethrows, guarded so it cannot clobber an already-established authenticated session.
- [x] 2.5 Audited the rest. **Needed a change**: `logout()`, `_fetchMe()` and `refreshMe()` each called `clearToken()`, which now throws — all three routed through a new `_clearTokenBestEffort()` so a keystore failure cannot break logout or strand the bootstrap. **Already terminal, unchanged**: `build()`/`_bootstrap()` (via `_fetchMe`'s catch-all returning `AuthState.error`), `retry()` (`AsyncValue.guard`), `changePassword()` (never enters `loading`; rethrows to its screen).
- [x] 2.6 Controller tests against a throwing storage fake: failed token write ends `authenticated` with the notice raised; failed org_code write is invisible; an unexpected `StateError` mid-login terminates as `unauthenticated` and never `loading`; `AuthLoading` is never observed during a login; logout survives a rejected delete. Widened the repo fake's `loginThrow` to `Object?` so non-`ApiException` types — the class that used to strand the machine — are actually injectable.

## 3. UI

- [x] 3.1 Verified: with the flip gone the login screen stays mounted, so its existing `on ApiException` / `catch (_)` / `finally` block now actually runs. No change needed to that block.
- [x] 3.2 Home shows the notice via `pendingSessionNotPersistedProvider`. It carries a **flag, not a string**, so the wording is localized where a `BuildContext` exists — the neighbouring handover notice hardcodes Chinese in the notifier and shows Chinese to English users, which is not a pattern worth copying.
- [x] 3.3 Added `sessionNotPersistedNotice` to the hand-rolled `app_localizations.dart` shim (zh + en) and to all three ARB files. **No regeneration step exists** — the project uses a hand-written shim, not `flutter gen-l10n` (see the header comment in that file). Noted in passing: the ARBs carry 34 keys against the shim's ~102, so they are already well behind; not fixed here.
- [x] 3.4 Widget tests: an unexpected failure now renders on the login screen with the submit button re-enabled (it was previously discarded by `if (!mounted) return;`); home shows the notice when the flag is set before it mounts, and stays on home rather than bouncing to `/login`; no notice on a normal login.

## 4. Verification

- [x] 4.1 `flutter analyze lib test` clean; `flutter test` **216/216** green, including every pre-existing auth test — removing the loading flip broke nothing.
- [ ] 4.2 **Reproduce on the affected physical device** (iPhone 17, iOS 26.5.2 — the simulator's Keychain is clean and cannot reproduce this). Login must reach home. If the Keychain write still fails, the persistence notice must appear.
- [ ] 4.3 While on that device, capture the actual Keychain `OSStatus` from Console.app (filter process `Runner`) and record it here. This is the input to the follow-up investigation — plugin upgrade vs entitlements vs device state — which is deliberately out of this change's scope.
- [ ] 4.4 Confirm the fix on a healthy device too: a normal login is unchanged, no spurious notice, and `org_code` still pre-fills on the next visit to `/login`.
- [ ] 4.5 Check whether Android reproduces the write failure. The write paths are shared and were equally unprotected; 0.4.2's read-side fix came from an Android report, so it is the likelier second platform.
