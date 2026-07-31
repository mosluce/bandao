## Context

The failure was reproduced on a physical iPhone (iOS 26.5.2) against prod, with a debug build so the dio log interceptor was active. The evidence chain:

```
POST /app/auth/login  → 200          (prod: 4 app_sessions created, 0 failed attempts)
                      ↓
             (complete silence)
                      ↓
relaunch → no GET /app/me            (token was never persisted)
```

Four statements sit between the `200` and a usable UI:

```dart
final res = await repo.login(...);          // 200
await storage.writeToken(res.token);        // unprotected plugin call
await storage.writeLastOrgCode(orgCode);    // unprotected plugin call
state = data(AuthState.authenticated(...));
```

Three code facts make a failure there unrecoverable:

1. `SecureStorage._safeRead` wraps reads in `try / on PlatformException`; **no write or delete has any wrapper**. The read wrapper is the 0.4.2 fix for the Android Keystore invalidation that used to hang the app at startup — the write half was never done.
2. `login()` sets `state = data(AuthState.loading())` before the request. Only `on ApiException` restores it. Anything else leaves the state machine at `loading()` permanently.
3. The router maps `AuthLoading` to `/splash`, so the login screen is unmounted *at the start of the submit*. Its `catch` blocks all begin `if (!mounted) return;`, so on the failing path they return before setting anything — the error is discarded outright, and `_submitting` never resets either.

The relevant UI already exists and is simply never reached: `splash_screen.dart` renders an `_ErrorBlock` with a 重試 button for `AuthError`, and `app-shell`'s auto-login requirement already specifies that behaviour for bootstrap network failures.

## Goals / Non-Goals

**Goals:**

- A storage failure can never leave the app on a spinner with no way forward.
- The auth state machine reaches a terminal state on every path, including exceptions nobody anticipated.
- A device whose Keychain cannot be written to remains **usable**, not bricked.
- Storage failures become observable instead of being silently absorbed.

**Non-Goals:**

- Identifying the underlying `OSStatus` / Keychain error. It changes neither remedy; capture it with Console.app while verifying.
- Upgrading `flutter_secure_storage` (9.2.4 → 10.3.1). Separate change, separate risk. This fix must hold even if the plugin is the culprit.
- Reworking `AuthState` into a different shape, or changing the router's redirect table beyond what the error path needs.
- Persisting the token by some other mechanism when the Keychain fails. A bearer token belongs in the Keychain; falling back to weaker storage would trade a usability bug for a security one.

## Decisions

### Do not flip global auth state to `loading` during a login submit

This is the root cause of the lost error, and removing it fixes that half outright. The login screen already owns a `_submitting` flag driving its own progress indicator; the global `AuthState.loading()` adds nothing except unmounting the screen that knows how to report the failure.

With the screen still mounted, the existing `on ApiException` / `catch (_)` / `finally` block does exactly what it was written to do — show the message, re-enable the button. No new UI.

Considered and rejected: keeping the flip and teaching `/splash` to show login errors. That works (splash already renders `AuthError`), but it routes a *login form* failure onto a screen with no form, and the user's next action — fix the password and retry — has to bounce them back anyway. Also rejected: keeping the flip and having the login screen read `authProvider`'s error instead of local state; that leaves the unmount/remount churn in place and makes the error's lifetime harder to reason about.

`AuthState.loading()` remains for what it was designed for: the bootstrap window in `build()`, where there genuinely is no screen to own the failure.

### Guarantee a terminal state regardless of what throws

Removing the flip is necessary but not sufficient — it fixes *this* path. `login()`, `logout()`, `changePassword()` and `refresh()` all mutate `state` around awaits, and a non-`ApiException` escaping any of them can strand the machine.

Every state-mutating method SHALL end in a terminal state (`authenticated`, `unauthenticated`, or `error`) on all paths. Where a method cannot decide, `error` is correct — the router sends `AuthError` to `/login`, and both `/login` and `/splash` can render it.

This is the belt to the previous decision's braces: it is what stops the *next* unanticipated exception from reproducing this bug somewhere else. The two decisions are deliberately redundant.

### Writes and deletes throw a typed failure; they do not fail silently

The proposal called for "a typed result the caller can act on". Refined after seeing the read path's consequences: **a typed exception**, not a return value.

`_safeRead`'s silent absorption is precisely why this bug read as a login problem for so long — a Keychain that cannot be read looks identical to a Keychain with nothing in it. Repeating that shape on writes would produce a worse bug: a token silently not stored, and the user mysteriously logged out on next launch with nothing anywhere to explain it.

A typed `SecureStorageFailure` (wrapping the platform error and naming the key) makes callers decide. Combined with the terminal-state guarantee above, an unhandled one is contained rather than fatal. A `bool` return would be silently ignorable by the next caller added — the exact failure mode being fixed.

### A token that cannot be persisted does not block the session

When `writeToken` fails, the user **is** authenticated — the server issued a token and the process holds it. The choice is what to do with that fact:

| | Behaviour | Consequence on a permanently-broken Keychain |
|---|---|---|
| Treat login as failed | `unauthenticated` + error | **App is unusable forever.** The user can never get in. |
| **Proceed, warn about persistence** (chosen) | `authenticated`, non-blocking notice | App works; user re-logs in each cold start |

The second is chosen because the first punishes the user for a device-state problem they cannot fix, and discards a session that is otherwise completely valid. The notice must say what will actually happen ("下次開啟 App 需要重新登入") rather than surfacing a platform error code.

`writeLastOrgCode` failing is cosmetic — it only pre-fills a form field — and SHALL never affect the session.

### Read failures stay fail-soft but stop being invisible

`_safeRead` keeps returning `null` on failure: the app must still boot, and that requirement is already specified. What changes is that the failure is recorded rather than discarded, so "Keychain is broken" is diagnosable from the app's own reporting instead of requiring a physical device, a cable, and a debug build — which is what this investigation cost.

The project already has Crashlytics wired (`firebase_crashlytics` in `pubspec.yaml`), so there is an existing sink for this.

## Risks / Trade-offs

- **Removing the global loading flip changes what other listeners observe mid-login** → Nothing currently watches for it: the router is the only consumer of `AuthLoading`, and its purpose there is the bootstrap window, which is untouched. Verified by grepping `AuthLoading` usage before the change lands.

- **"Proceed without persistence" hides a real device problem behind a working app** → Mitigated by the non-blocking notice plus the Crashlytics report. The alternative — blocking the user entirely — is strictly worse for someone who just wants to clock in.

- **The underlying Keychain failure is still unfixed** → Accepted and explicit. This change converts a silent total lockout into a working app with a visible, reported warning. Whether the cause is the plugin version, iOS 26 behaviour, or device state is a separate investigation that this change makes *possible* rather than requiring a tethered debug session.

- **Cannot be verified on a simulator** → The simulator's Keychain is clean and does not reproduce the failure. Verification needs the affected physical device, and a fault-injecting fake for the automated tests.

## Migration Plan

No data or API migration. Client-only.

1. Land the change and cut a build.
2. Verify on the affected physical device: login must reach home, with the persistence notice visible if the Keychain write still fails.
3. Capture the `OSStatus` from Console.app during that verification and file it against the follow-up plugin/Keychain investigation.

Rollback is shipping the previous build; nothing is written that a rollback would strand.

## Open Questions

- **What is the actual Keychain error?** Deliberately deferred (see Non-Goals). The answer decides whether the follow-up is a plugin upgrade, an entitlements change, or device-specific state — none of which change this change's contents.
- **Does the same failure occur on Android?** The write paths are shared and equally unprotected, so the fix covers both, but only iOS has a confirmed reproduction. Worth a check once a build is out, since 0.4.2's read-side fix was prompted by an Android report.
