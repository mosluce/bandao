## 1. Shared verification logic

- [x] 1.1 Write the artifact-verification routine: given an archive, a list of internal snapshot paths, and the URLs that must be present, `grep -a -q` each URL in each snapshot and exit non-zero naming the missing URL
- [x] 1.2 Make an unlocatable snapshot path a failure, not a pass — a layout change must stop the release rather than silently skip the check
- [x] 1.3 Comment why the check asserts presence only. Measured during implementation: whether a loopback literal survives is *inconsistent* — a correctly built bundle drops `10.0.2.2:3000/privacy` (the define makes that branch dead code) but keeps `10.0.2.2:9090`, so an absence check would red-flag good builds on one URL and never fire on the other

## 2. `release_android.sh`

- [x] 2.1 Create `app/scripts/release_android.sh` following `release_ios.sh`'s shape: header comment explaining the one-time setup and common invocations, `set -euo pipefail`, `SCRIPT_DIR`/`APP_DIR` resolution, `usage()`, flag parsing
- [x] 2.2 Default `API_URL` to `${BANDAO_API_URL:-https://bandao-api.ccmos.tw}`, matching `release_ios.sh`; read `PRIVACY_URL` from `app/store_metadata/android/privacy_policy_url.txt`
- [x] 2.3 Do NOT bump `pubspec.yaml` — echo the version being built and leave the shared build counter alone, so an Android cut cannot desync the pair from an iOS cut
- [x] 2.4 Run `flutter pub get`, `dart run build_runner build --delete-conflicting-outputs`, then `flutter build appbundle --release` with both `--dart-define`s
- [x] 2.5 Locate the produced `.aab` and fail clearly if the build reported success but no bundle is on disk
- [x] 2.6 Verify every `base/lib/*/libapp.so` in the bundle carries both URLs, via the routine from group 1
- [x] 2.7 Hand off to `upload_android.sh` (or print the exact command under `--no-upload`), keeping that script's `internal`-track default and its refusal to script promotion
- [x] 2.8 `chmod +x` and confirm `./scripts/release_android.sh --help` prints usage without side effects

## 3. `release_ios.sh`

- [x] 3.1 Add `--dart-define=PRIVACY_URL`, read from `app/store_metadata/ios/privacy_url.txt`
- [x] 3.2 Add the verification step against `Payload/Runner.app/Frameworks/App.framework/App` after the build and before `xcrun altool --upload-app`
- [x] 3.3 Update the script's header comment — it currently explains why `API_BASE_URL` is passed; extend it to cover `PRIVACY_URL` and the post-build check

## 4. Documentation

- [x] 4.1 Replace `DEPLOY.md`'s bare `flutter build appbundle --release` block with the `release_android.sh` path
- [x] 4.2 Carry the iOS section's "the dart-define is **required** — without it login silently fails" warning into the Android section, noting the `10.0.2.2` fallback and that Android also blocks the resulting cleartext request
- [x] 4.3 Keep a manual fallback sequence documented, with both `--dart-define`s inline so copying it cannot reproduce the defect
- [x] 4.4 Document the verification step in both platforms' sections, including how to run the same check by hand against any downloaded artifact

## 5. Verification

- [x] 5.1 Run `./scripts/release_android.sh --no-upload` and confirm the produced `.aab` passes verification and contains `https://bandao-api.ccmos.tw`
- [x] 5.2 Deliberately build a bundle without the defines and confirm the verification routine fails, names the missing URL, and returns non-zero
- [x] 5.3 Run `./scripts/release_ios.sh --no-upload` and confirm the `.ipa` passes with both URLs present
- [x] 5.4 Confirm the existing 2026-07-31 `.aab` fails the check — it is the artifact this change exists to have caught
- [x] 5.5 Confirm `release_android.sh` never modifies `app/pubspec.yaml`, and that `release_ios.sh` still bumps it by design unless `--no-bump` is passed — the original wording ("neither script modified pubspec when run with `--no-upload`") was wrong: `--no-upload` does not suppress the iOS bump, and suppressing it is not a goal

## 6. Follow-ups for the operator (not code)

- [x] 6.1 Skipped by the operator: the Android app has no real users yet, so whether earlier releases carried the same defect changes nothing actionable. Rolling back was never a useful stopgap here.
- [x] 6.2 Cut the corrected release as `0.4.4+14` via the new script and ship it
