## Why

An AppUser on a real iOS device cannot get past login. The app shows an indefinite spinner with no error and no way forward. Observed on iOS 26.5.2 with both the TestFlight `0.4.3+1` release build and a locally-built debug build; deleting and reinstalling the app does not help.

The device console shows exactly one thing and then silence:

```
🐛 -> POST https://bandao-api.ccmos.tw/app/auth/login
🐛 <- 200 POST https://bandao-api.ccmos.tw/app/auth/login
```

Server-side, the login **succeeds**: four `app_sessions` rows were created for this AppUser in three hours, with zero failed-login attempts and no lockout.

**The root cause is a client/server DTO mismatch.** `AppUserDto` marks both `username` and `external_key` `skip_serializing_if = "Option::is_none"`, so an external shadow user's payload carries `external_key` and **omits `username` entirely**. The Flutter model hard-casts it:

```dart
username: json['username'] as String,   // TypeError on every external user
```

Every AppUser in an external-auth Org is a shadow user, so that Org could never log in at all. The model predates `external-db-auth` and was never updated for it — the same drift that produced today's `EventSource`-missing-`legacy_backfill` crash, in the same hand-rolled DTO layer.

The parse throws a `TypeError`, not an `ApiException`. Two independent structural defects turn that into a total lockout rather than an error message:

1. **`SecureStorage` hardens reads but not writes.** `_safeRead` catches `PlatformException`, drops the corrupted entry and returns `null` — that is the fix shipped in 0.4.2 for the Android Keystore invalidation that used to make the app "卡死在啟動流程無法開啟". Every write and delete (`writeToken`, `writeLastOrgCode`, `writeApiBaseUrlOverride`, `markBackgroundSyncTipSeen`, `writeLocationTrackingLastCleanStop`, and all `clear*`) calls the plugin raw. The 0.4.2 fix also **hides the root cause**: a failing read is now indistinguishable from "no value stored", so the only symptom that surfaces is the unprotected write.

2. **`AuthState.loading()` has no path back.** `login()` sets `state = data(AuthState.loading())` before the request — which makes the router replace `/login` with `/splash` immediately. Only `on ApiException` restores the state; any other throw leaves it at `loading()` forever, and `/splash` has no error UI or retry. The login screen's `catch (_)` does fire and sets an error message, but on a `State` object the router unmounted at the very start of the submit, so the message is never seen.

Those two are why the failure was *silent and unrecoverable* rather than a visible error. They are worth fixing on their own terms: without them, any future non-`ApiException` anywhere on the login path reproduces the same dead end.

**A correction, recorded because it cost most of the investigation.** The first diagnosis attributed this to `writeToken` failing against a broken iOS Keychain — `writeToken` sits at exactly that point in the sequence, and 0.4.2 had already fixed the Android Keystore analogue on the *read* path. That was wrong. Device verification showed the Keychain writing fine and no persistence notice firing; the parse was always the culprit. The evidence was available from the start — the KLCC `app_users` documents have no `username` field — and was not weighted properly against a more familiar-looking hypothesis.

Also ruled out and recorded so nobody re-walks them: external MSSQL auth latency (login returned 200), a stale per-device API base URL override (requests reach prod), an Apple team-prefix change invalidating Keychain items (`DEVELOPMENT_TEAM` last changed in #12, before any release build), and the `merge-checkin-events-into-trajectory` change (it touched none of the login path).

## What Changes

- **Accept both AppUser identity shapes.** `username` becomes optional and `external_key` is added, matching what the server actually serialises. This is the root-cause fix: it is what unblocks external-auth Orgs. Where the UI shows a machine-readable identity it prefers `username`, falls back to `external_key`, and renders nothing when neither is present.
- **Harden `SecureStorage` writes and deletes** to the same standard as `_safeRead`. A platform failure MUST NOT propagate as an unhandled exception. Writes report failure through a typed result the caller can act on rather than throwing, so the auth layer can distinguish "token stored" from "token not stored" and behave accordingly.
- **Stop masking storage failures.** `_safeRead` currently swallows `PlatformException` silently, which is why this bug looked like a login problem rather than a storage problem. Reads keep their fail-soft behaviour — the app must still boot — but the failure becomes observable rather than invisible.
- **Give `AuthState.loading()` a guaranteed exit.** `login()` SHALL leave the state machine in a terminal state (`authenticated` or `unauthenticated`) on every path, including unexpected exceptions. This is the defect that turns any post-response failure into a permanent spinner, and it is fixed independently of what threw.
- **Surface the failure where the user is.** An error raised after the router has parked on `/splash` must reach a screen the user is actually looking at. Today the message is written to an unmounted login screen and lost.
- **Decide what a failed token write means for the session.** A successful login whose token cannot be persisted is a real state: the user is authenticated for this process but will be logged out on next launch. The change picks one behaviour and specifies it rather than leaving it to whichever exception fires first.

The storage hardening stays in scope even though it turned out not to be the cause. Writes really were the only unguarded platform calls left after 0.4.2, and the silent read path really did hide the problem — during the investigation a broken Keychain was indistinguishable from an empty one. It is genuine hardening, and it is what makes the *next* keystore failure diagnosable from telemetry instead of from a tethered debug session.

**Not in scope**: auditing the remaining hand-rolled DTOs for the same client/server drift. Two instances surfaced in one day (`EventSource` missing `legacy_backfill`, `AppUser.username` required) and the model file itself notes that codegen was deferred — but a full audit, or adopting OpenAPI codegen, is its own change. Also out of scope: upgrading `flutter_secure_storage` (9.2.4, with 10.3.1 available).

## Capabilities

### New Capabilities

(none — all changes extend existing capabilities)

### Modified Capabilities

- `app-shell`: a new requirement makes both AppUser identity shapes (`username` / `external_key`) acceptable to the client — the root-cause fix; a new requirement covers the auth state machine always reaching a terminal state with a user-visible error; a new requirement covers secure-storage writes failing loudly but non-fatally; the login requirement gains a specified outcome for a token write that fails; and the "Bearer-token reads tolerate iOS device-lock" requirement gains a write-side counterpart.

## Impact

- **app (`app/`)**: `lib/core/api/models/app_user.dart` (optional `username`, new `external_key`, `identityLabel` fallback — the root-cause fix), `lib/features/auth/presentation/home_screen.dart` (the one place that renders the identity), `lib/core/storage/secure_storage.dart` (write/delete hardening, read failures made observable), `lib/core/telemetry/error_reporter.dart` (new injectable sink over the existing Crashlytics integration), `lib/features/auth/state/auth_provider.dart` (terminal-state guarantee, best-effort token clear).
- **No API, admin-web, or database change.** This is entirely client-side.
- **Every AppUser of an external-auth Org** is currently unable to log in at all. The identity fix unblocks them. The state-machine fix independently guarantees that a future failure on this path shows an error rather than an endless spinner — confirmed in the field during this investigation, where it turned the silent hang into a visible, retryable failure and is what made the real cause findable.
- **Release**: needs a new build to reach the affected device. iOS is the confirmed platform; the Android write paths share the same unprotected code and are fixed by the same change.
