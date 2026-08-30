## Why

The server-configuration screen turns the compile-time default into a permanent per-device override on a save that changes nothing.

`_seed()` pre-fills the override input with `Env.compileTimeDefault()` whenever no override is stored, and `_save()` / `_savePrivacy()` write whatever is in the field unconditionally. So a user who opens the screen, looks around, and taps 儲存 without editing a character has silently pinned that build's default into secure storage. From then on the override wins over the compile-time value forever: every future URL change shipped in an app update is invisible to that device, with nothing in the UI indicating why.

This is the same failure shape as the incident that produced `fix-mobile-release-target-urls` — a device quietly serving a stale URL while the binary carries the correct one — except here the pinning survives reinstalls, because iOS Keychain entries outlive app deletion.

Two adjacent defects on the same screen:

- The helper text reads 留空即使用官方預設, but a blank field cannot be saved: `validateBaseUrlOverride("")` parses to a URI with no scheme and is rejected as `malformed`. And `_seed()` never leaves the field blank in the first place. The affordance the UI advertises does not exist.
- The privacy-URL section is visible in release builds — the `kDebugMode` gate sits below it — yet `_savePrivacy()` validates only scheme + authority, with no `https` requirement. A release user can pin an `http://` privacy URL, while the API field in the same screen rejects exactly that.

## What Changes

- The override inputs SHALL start **empty** when no override is stored. The screen already displays the effective URL above each field, so the current value stays visible without occupying the input. This makes the existing 留空即使用官方預設 helper text true.
- Saving a **blank** field SHALL clear the stored override rather than fail validation.
- A typed value SHALL be stored as-is, including one that equals the compile-time default. The default is a fixed property of the build that no user action redefines; the override sets the *current* value, and the screen does not second-guess what the user entered.
- The privacy-URL override SHALL be validated by the same build-mode rules as the API base URL: `https` required in release, any scheme with a host accepted in debug.
- **BREAKING (per-device, recoverable):** devices that already carry an override equal to the compile-time default keep it until the user clears it. Existing overrides are not migrated — see design.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `app-shell`: the "Server configuration screen is reachable in all builds" requirement gains rules about what the input is seeded with, what a blank save means, and that saving the default is not a way to pin it. The base-URL override requirement gains the parity rule for the privacy-URL override's validation.

## Impact

- `app/lib/features/auth/presentation/server_config_screen.dart` — seeding, both save handlers, the clear handlers.
- `app/lib/core/storage/api_base_url.dart` — `validateBaseUrlOverride` gains a documented empty-string meaning, or the caller handles blank before validating (design decides).
- `app/lib/core/storage/privacy_url.dart` — privacy override validation reuses the base-URL rules.
- `app/test/` — existing server-config and validation tests; new cases for blank save, default-equals save, and release-mode privacy validation.
- No API, admin-web, or release-tooling change.
