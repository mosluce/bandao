## Why

An AppUser on a real iOS device cannot get past login. The app shows an indefinite spinner with no error and no way forward. Observed on iOS 26.5.2 with both the TestFlight `0.4.3+1` release build and a locally-built debug build; deleting and reinstalling the app does not help.

The device console shows exactly one thing and then silence:

```
🐛 -> POST https://bandao-api.ccmos.tw/app/auth/login
🐛 <- 200 POST https://bandao-api.ccmos.tw/app/auth/login
```

Server-side, the login **succeeds**: four `app_sessions` rows were created for this AppUser in three hours, with zero failed-login attempts and no lockout. Every one of those tokens was then lost — relaunching the app issues no `GET /app/me`, so `auth.bearer_token` was never persisted.

Between the `200` and the UI updating there are exactly three statements:

```dart
final res = await repo.login(...);          // ✅ 200, token in hand
await storage.writeToken(res.token);        // ← dies here
await storage.writeLastOrgCode(orgCode);
state = data(AuthState.authenticated(...)); // ← never runs
```

Two independent defects turn a storage failure into a total lockout:

1. **`SecureStorage` hardens reads but not writes.** `_safeRead` catches `PlatformException`, drops the corrupted entry and returns `null` — that is the fix shipped in 0.4.2 for the Android Keystore invalidation that used to make the app "卡死在啟動流程無法開啟". Every write and delete (`writeToken`, `writeLastOrgCode`, `writeApiBaseUrlOverride`, `markBackgroundSyncTipSeen`, `writeLocationTrackingLastCleanStop`, and all `clear*`) calls the plugin raw. The 0.4.2 fix also **hides the root cause**: a failing read is now indistinguishable from "no value stored", so the only symptom that surfaces is the unprotected write.

2. **`AuthState.loading()` has no path back.** `login()` sets `state = data(AuthState.loading())` before the request — which makes the router replace `/login` with `/splash` immediately. Only `on ApiException` restores the state; any other throw leaves it at `loading()` forever, and `/splash` has no error UI or retry. The login screen's `catch (_)` does fire and sets an error message, but on a `State` object the router unmounted at the very start of the submit, so the message is never seen.

The second defect is the one that makes the app unusable: even with storage fixed, any other non-`ApiException` failure on that path (response parsing, the drift handover wipe) produces the identical dead end.

Ruled out during investigation, recorded so nobody re-walks them: external MSSQL auth latency (login returned 200), a stale per-device API base URL override (requests reach prod), an Apple team-prefix change invalidating Keychain items (`DEVELOPMENT_TEAM` last changed in #12, before any release build), and the `merge-checkin-events-into-trajectory` change (it touched none of the login path).

## What Changes

- **Harden `SecureStorage` writes and deletes** to the same standard as `_safeRead`. A platform failure MUST NOT propagate as an unhandled exception. Writes report failure through a typed result the caller can act on rather than throwing, so the auth layer can distinguish "token stored" from "token not stored" and behave accordingly.
- **Stop masking storage failures.** `_safeRead` currently swallows `PlatformException` silently, which is why this bug looked like a login problem rather than a storage problem. Reads keep their fail-soft behaviour — the app must still boot — but the failure becomes observable rather than invisible.
- **Give `AuthState.loading()` a guaranteed exit.** `login()` SHALL leave the state machine in a terminal state (`authenticated` or `unauthenticated`) on every path, including unexpected exceptions. This is the defect that turns any post-response failure into a permanent spinner, and it is fixed independently of what threw.
- **Surface the failure where the user is.** An error raised after the router has parked on `/splash` must reach a screen the user is actually looking at. Today the message is written to an unmounted login screen and lost.
- **Decide what a failed token write means for the session.** A successful login whose token cannot be persisted is a real state: the user is authenticated for this process but will be logged out on next launch. The change picks one behaviour and specifies it rather than leaving it to whichever exception fires first.

**Not in scope**: identifying the underlying iOS Keychain error code. The `OSStatus` would sharpen the storage fix but changes neither defect's remedy — writes need the same wrapper regardless, and the state-machine hole is storage-agnostic. It can be captured later with Console.app while verifying the fix. Also out of scope: upgrading `flutter_secure_storage` (9.2.4, with 10.3.1 available) — a plugin bump is a separate change with its own migration risk, and this change must hold even if the plugin is at fault.

## Capabilities

### New Capabilities

(none — all changes extend existing capabilities)

### Modified Capabilities

- `app-shell`: the login requirement gains a specified outcome for a token write that fails; the "Bearer-token reads tolerate iOS device-lock" requirement gains a write-side counterpart so storage failures are non-fatal in both directions; and a new requirement covers the auth state machine always reaching a terminal state with a user-visible error.

## Impact

- **app (`app/`)**: `lib/core/storage/secure_storage.dart` (write/delete hardening, read failures made observable), `lib/features/auth/state/auth_provider.dart` (terminal-state guarantee on `login`, and the same audit for `logout` / `changePassword` / `refresh` which share the pattern), `lib/features/auth/presentation/login_screen.dart` and/or the splash screen (error surface), `lib/app/router.dart` if splash needs a failure state.
- **No API, admin-web, or database change.** This is entirely client-side.
- **Users currently locked out** are unblocked by the state-machine fix even if their Keychain remains broken: they will see a real error instead of an endless spinner, and the app remains usable up to the point that actually fails.
- **Release**: needs a new build to reach the affected device. iOS is the confirmed platform; the Android write paths share the same unprotected code and are fixed by the same change.
