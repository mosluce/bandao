## Why

The Android bundle built on 2026-07-31 (0.4.3+13) shipped to Google Play production with `http://10.0.2.2:9090` — the Android emulator's alias for the build machine's loopback — baked in as its API base URL. On a real device that address resolves to nothing, and because the app declares no cleartext exception, the `http://` request would be blocked by Android's default network security policy even if it did. The release cannot reach the backend at all.

The cause is that `DEPLOY.md`'s Android build step is a bare `flutter build appbundle --release` with no `--dart-define=API_BASE_URL`. `Env.compileTimeDefault()` then silently falls back to the platform dev default. This has been the documented Android procedure since it was written on 2026-05-07; only iOS was ever protected, and only incidentally — `release_ios.sh` happens to pass the dart-define, and `DEPLOY.md` warns about it in the iOS section alone.

A second instance of the same failure affects both platforms: `PRIVACY_URL` is never passed by any script or documented anywhere, so both shipped binaries link the location-consent dialog's privacy policy at `http://localhost:3000/privacy` (iOS) / `http://10.0.2.2:3000/privacy` (Android). Both store listings declare `https://bandao-admin.ccmos.tw/privacy`, so the in-app link contradicts what was declared to app review.

Underneath both is a specification gap: `mobile-release` says nothing about the API base URL or the privacy URL. That a shipped binary must point at the production backend has never been a requirement, so nothing has ever enforced it.

## What Changes

- Add `app/scripts/release_android.sh`, mirroring `release_ios.sh`: `pub get` → codegen → `flutter build appbundle --release` with **both** `--dart-define=API_BASE_URL` and `--dart-define=PRIVACY_URL`, then artifact verification, then optional handoff to `upload_android.sh`.
- Add **post-build artifact verification** to both release scripts: unpack the built `.aab` / `.ipa`, confirm the expected production URLs are present in the Dart snapshot, and fail the build when they are not. This verifies the file that will actually be uploaded rather than the arguments the operator believes they passed — it is the check that detected this incident.
- Pass `PRIVACY_URL` on both platforms, sourced from `app/store_metadata/{ios/privacy_url.txt,android/privacy_policy_url.txt}` so the in-app link cannot drift from the store declaration.
- Replace `DEPLOY.md`'s bare Android build block with the scripted path, and carry the "the dart-define is required" warning that currently exists only in the iOS section.
- Establish in `mobile-release` that a release artifact SHALL carry the production backend and privacy URLs, and that the release procedure SHALL verify the artifact before upload.

Out of scope: the operational hotfix itself (rebuilding and shipping a corrected bundle) and the question of whether previously published Android bundles carried the same defect — both are operator actions this change enables rather than performs.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mobile-release`: currently specifies signing, versioning, permission copy, and store listing, but never states that a shipped artifact must target the production backend. Adds requirements that release builds bake the production API base URL and privacy policy URL, and that the release procedure verifies the built artifact before upload.

## Impact

- `app/scripts/release_android.sh` — new.
- `app/scripts/release_ios.sh` — adds `PRIVACY_URL` and post-build verification.
- `DEPLOY.md` — Android release section rewritten; iOS section updated for the new verification step.
- `openspec/specs/mobile-release/spec.md` — via delta.
- No application code changes. `app/lib/core/env/env.dart` keeps its dev-default fallback: the fallback is correct for `flutter run`, and the defect is that nothing checked release builds, not that the fallback exists.
- Operationally unblocked once this lands: cutting a corrected Android build. Per the shared build counter, that is `0.4.4+14` — Play rejects a reused `versionCode`, and the 0.4.3 train has already shipped.
