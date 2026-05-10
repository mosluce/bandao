## Why

On iOS, when the device is locked while the app is backgrounded, the location-pings batch processor fires `POST /app/checkin/locations` but `flutter_secure_storage`'s default Keychain accessibility (`kSecAttrAccessibleWhenUnlocked`) makes the bearer token unreadable. The request goes out without an `Authorization` header, the server returns 401, and the processor's `_onAuthExpired` handler calls `authProvider.logout()` — silently logging the user out. When the user picks the phone up again, they land on `/login`, losing whatever in-flight queue context they had.

This was reproduced end-to-end on a real iPhone (iOS 26.3.1, release build, prod backend): rolling diagnostic prints in `AuthInterceptor.onRequest` show `token=set` while the screen is unlocked and `token=NULL` for every `/app/checkin/locations` request issued after lock, followed by chained 401s and `logout()` calls. The bug therefore has two compounding causes that this change addresses together.

## What Changes

- Cache the bearer token in process memory after the first successful Keychain read on startup; serve all hot-path reads (`AuthInterceptor`, `_bootstrap`, `refreshMe`, etc.) from that cache. Writes (`writeToken`, `clearToken`) update memory and Keychain together so the cache cannot drift from persistent state.
- Configure `flutter_secure_storage` on iOS with `IOSOptions(accessibility: KeychainAccessibility.first_unlock)` so even the cold-launch-while-locked path (rare; iOS launches the app process for a location event before the user has unlocked since reboot) can still read the token from Keychain.
- No changes to existing call sites — the `SecureStorage` wrapper keeps its current public surface.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities

- `app-shell`: tighten the secure-storage requirements so bearer-token availability survives iOS device lock and the AuthInterceptor does not depend on per-request Keychain I/O.

## Impact

- **Code**: `app/lib/core/storage/secure_storage.dart` (in-memory cache + iOS accessibility override). No call-site changes elsewhere.
- **Behavior**: removes the silent background logout path; once logged in, users stay logged in across screen lock + background sync as long as the session is server-side valid.
- **Security**: Keychain item moves from `WhenUnlocked` to `AfterFirstUnlock`. Token is readable after the first post-reboot unlock and remains readable until power-off — the standard posture for keep-me-logged-in mobile apps. The token is still hardware-protected and tied to the app's Keychain access group.
- **Tests**: existing `SecureStorage` fakes/tests keep working since the interface is unchanged. New unit coverage SHOULD verify that cached reads do not hit the underlying `FlutterSecureStorage` after the first read.
- **Dependencies**: none (uses options already exposed by the existing `flutter_secure_storage` dependency).
- **No server changes** required.
