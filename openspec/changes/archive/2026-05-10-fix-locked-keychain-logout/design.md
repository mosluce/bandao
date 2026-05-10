## Context

The bandao app keeps an authenticated AppUser logged in by storing a bearer token in iOS Keychain via `flutter_secure_storage` (key: `auth.bearer_token`). Every authenticated HTTP call (`/app/*`) goes through `AuthInterceptor.onRequest`, which awaits `SecureStorage.readToken()` to attach `Authorization: Bearer <token>`. The location-tracking pipeline (`GeolocatorTrackingService` + `LocationPingProcessor`) writes pings into a drift queue and flushes batches to `POST /app/checkin/locations` whenever drift change streams or a 1Hz timer fire — including while iOS is keeping the app process alive in the background due to the `location` background mode.

Reproduced on iPhone running iOS 26.3.1 with a release build pointing at production (`https://bandao-api.ccmos.tw`):

```
flutter: [AUTH] /app/checkin/locations token=set      # foreground
flutter: [AUTH] /app/checkin/locations token=NULL     # immediately after Cmd+L lock
flutter: [AUTH] /app/auth/logout token=NULL           # processor's onAuthExpired → logout
```

`flutter_secure_storage`'s default iOS accessibility is `KeychainAccessibility.unlocked` (≈ `kSecAttrAccessibleWhenUnlocked`) — the Keychain item is unreadable while the device is locked. Because `AuthInterceptor` re-reads Keychain on every request, every background ping issued during a locked screen ships without `Authorization`. The server returns 401, `LocationPingProcessor._drainOnce` interprets that as session-expired and calls `authProvider.logout()`, which clears the local token and flips state to `unauthenticated`. Resume → `redirect()` lands the user on `/login`.

Server-side session TTL (14d sliding) and multi-device session handling are not at fault — confirmed by the absence of bug occurrence when the user does NOT move (no pings → no flush → no 401).

## Goals / Non-Goals

**Goals:**
- Keep authenticated state durable across iOS device-lock + background-sync windows.
- Stop using Keychain as a hot path for bearer-token reads — token is read once at process start.
- Maintain interface compatibility for `SecureStorage`'s public methods so all existing callers continue to work unchanged.
- Preserve the security posture of "token is encrypted at rest, scoped to this app's Keychain access group".

**Non-Goals:**
- Redesigning when location pings are sent (background-vs-resume) — orthogonal concern, see "Alternatives Considered".
- Changing server-side session semantics or 401 handling logic in `LocationPingProcessor` — out of scope; this fix removes the false-positive 401 trigger so the existing handler only fires for real auth failures.
- Adding a token-refresh mechanism — current sessions slide on every authenticated request server-side, no client refresh needed.
- Migrating to a different secure-storage library.

## Decisions

### 1. In-memory token cache inside `SecureStorage`

`SecureStorage` will hold a `String? _cachedToken` plus a `_tokenLoaded` flag. `readToken()`:

- If `_tokenLoaded == true` → return `_cachedToken` (no Keychain I/O).
- Else → read from Keychain, populate `_cachedToken` + flip `_tokenLoaded` to true, return value.

`writeToken(String t)` writes to Keychain THEN sets `_cachedToken = t; _tokenLoaded = true;`.
`clearToken()` deletes from Keychain THEN sets `_cachedToken = null; _tokenLoaded = true;` — `_tokenLoaded` stays true because "we know the answer is null" is itself a valid cached state.

**Why inside `SecureStorage` rather than `AuthInterceptor`?** Multiple call sites read the token (`AuthNotifier._bootstrap`, `AuthNotifier.refreshMe`, `AuthInterceptor.onRequest`, future call sites). Caching at the wrapper keeps the cache canonical for all of them with one implementation.

**Why not a Riverpod-level `Provider<String?>`?** Riverpod state would need to be plumbed into the dio interceptor layer, complicating the testing story and creating a circular bootstrap (interceptor needs token before auth state is built). The wrapper-level cache is simpler and call-site-transparent.

### 2. Keychain accessibility = `first_unlock`

Pass `IOSOptions(accessibility: KeychainAccessibility.first_unlock)` to `FlutterSecureStorage`. This maps to `kSecAttrAccessibleAfterFirstUnlock`: the item is unreadable until the user unlocks the device for the first time after reboot, but stays readable thereafter — including while the screen is locked.

This covers the cold-launch-while-locked corner case (iOS launches the killed app process to handle a background location event before the user has done their first post-reboot unlock). With cache-only on a freshly killed process, `_bootstrap()` would still hit Keychain once, and that one read would return null without `first_unlock`.

**Why not a stricter `first_unlock_this_device` (no iCloud Keychain sync)?** The bearer token is server-issued and AppUser-scoped — there is no benefit to syncing it via iCloud Keychain. We could choose either; `first_unlock` is the documented default for "data this app needs after first unlock". Decision: pick `first_unlock` for simplicity; both work for the bug.

### 3. Constructor signature: pass options through

`SecureStorage` currently has `SecureStorage([FlutterSecureStorage? storage])`. Change to construct the underlying storage with iOS options when none is injected:

```dart
SecureStorage([FlutterSecureStorage? storage])
    : _storage = storage ??
        const FlutterSecureStorage(
          iOptions: IOSOptions(accessibility: KeychainAccessibility.first_unlock),
        );
```

Tests that inject their own fake `FlutterSecureStorage` are unaffected — their fakes don't read iOS options.

### 4. Cache invalidation on logout from `_onAuthExpired`

`AuthNotifier.logout()` calls `storage.clearToken()`. Because `clearToken()` resets the cache, all subsequent in-flight 401-triggered logout calls (from queued pings still draining when the first 401 arrives) will see `_cachedToken = null` and not loop. With this fix the practical concern disappears — once `first_unlock` is in place, the original 401 doesn't happen — but the cache invalidation contract still has to hold for any other path that might 401 in the future.

## Alternatives Considered

- **Resume-only sync.** Gate `LocationPingProcessor` triggers on `AppLifecycleState.resumed`, drain only when foreground. Rejected as primary fix: bigger refactor, hides a class of bugs rather than fixing root cause, and admin-web trajectory display becomes user-resume-bound. Worth revisiting independently if battery telemetry suggests background HTTP is wasteful, but unrelated to this change.
- **Don't logout on a single 401 from `LocationPingProcessor`.** Reroute through `refreshMe()` and let `/app/me` arbitrate. Adds complexity and still leaves the underlying Keychain-locked-token issue intact. Defense-in-depth worth considering as a follow-up but not the right primary fix.
- **Store the token in `SharedPreferences` / `UserDefaults` instead of Keychain.** No Keychain accessibility constraints, but loses hardware-backed encryption. Rejected on security grounds.

## Risks / Trade-offs

- **[Risk] Cache drift if a non-`SecureStorage` path mutates the Keychain item directly.** → Mitigation: there is no such path today; `SecureStorageKeys.bearerToken` is referenced only inside `SecureStorage`. Document this invariant in the wrapper's class-level comment as part of the implementation.
- **[Risk] `first_unlock` slightly weaker than `WhenUnlocked` (token readable while screen is locked, post-first-unlock).** → Mitigation: this is the standard posture for any persistent-login mobile app; the token's blast radius is one AppUser session, server-side TTL bounds exposure to 14 days, and admin retains force-kick. Acceptable for this app.
- **[Risk] The cache survives a second AppUser logging in on the same device when handover wipe runs but before re-login completes.** → Mitigation: `clearToken()` is called from `logout()` and from `_fetchMe()` on 401, both of which already write `null` into the cache. Initial login via `writeToken()` overwrites the cache with the new value. The handover-wipe code path doesn't touch the bearer token directly.
- **[Trade-off] Cold-launch-while-locked still hits Keychain once.** With `first_unlock` it succeeds; without it, it returns null. We accept this single read on cold launch as the necessary persistence boundary.

## Migration Plan

1. Patch `SecureStorage` (constructor + cache + read/write/clear paths).
2. Bump `pubspec.yaml` build number, run `release_ios.sh` to ship a TestFlight build.
3. Manual smoke on iPhone (iOS 26.x): login → 上班 → lock screen → wait 5+ minutes → unlock and verify still on home (NOT on `/login`).
4. Verify same flow on Android (no Keychain semantics, but the in-memory cache change applies — sanity check no regression).
5. No data migration. Existing logged-in users keep working: their existing Keychain item remains readable after `first_unlock` is applied at next app launch.

**Rollback:** revert the `SecureStorage` patch and ship a new build. The Keychain item itself does not change format, only the access-policy attribute on writes after the patch — old items keep their original attribute and continue to work either way.

## Open Questions

- None blocking implementation. Future telemetry consideration: keep `LocationPingProcessor`'s 401-→-logout path instrumented (e.g. `Crashlytics.recordError(non-fatal)`) so we can observe whether any post-fix 401s slip through; this would belong in a separate small change and is not required for this fix.
