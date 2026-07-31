## ADDED Requirements

### Requirement: Release artifacts SHALL bake the production backend and privacy policy URLs

Every artifact built for store upload SHALL carry the production API base URL and the production privacy policy URL as compile-time values, supplied via `--dart-define=API_BASE_URL` and `--dart-define=PRIVACY_URL`.

`Env.compileTimeDefault()` and `Env.privacyUrlCompileTimeDefault()` fall back to platform-specific development loopback addresses (`http://10.0.2.2:9090` on Android, `http://localhost:9090` elsewhere) when the corresponding define is empty. Those fallbacks are correct for `flutter run` and SHALL be retained, but a release artifact SHALL NOT be built such that it depends on them. On Android the fallback is doubly unusable: `10.0.2.2` is the emulator's alias for the build host's loopback and means nothing on a device, and the app declares neither `usesCleartextTraffic` nor a `network_security_config`, so the platform blocks the plain-HTTP request regardless.

The privacy policy URL baked into an artifact SHALL be the same URL declared to that artifact's store listing — `app/store_metadata/ios/privacy_url.txt` for iOS and `app/store_metadata/android/privacy_policy_url.txt` for Android — so that the in-app link opened from the location-consent dialog cannot diverge from what was declared to app review. Release tooling SHALL read those files rather than restate the URL, so the two cannot drift apart.

Both platforms SHALL be cut through a release script that supplies these defines. A bare `flutter build appbundle --release` or `flutter build ipa --release` SHALL NOT be the documented release procedure, because omitting the defines produces a well-formed, correctly signed artifact that silently cannot reach its backend.

#### Scenario: Android release artifact carries the production API URL

- **WHEN** an Android release bundle is cut through the documented release procedure
- **THEN** the Dart snapshot in the produced `.aab` SHALL contain the production API base URL
- **AND** the app on a device SHALL resolve that URL rather than `http://10.0.2.2:9090`

#### Scenario: iOS release artifact carries the production API URL

- **WHEN** an iOS release archive is cut through the documented release procedure
- **THEN** the Dart snapshot in the produced `.ipa` SHALL contain the production API base URL
- **AND** the app on a device SHALL resolve that URL rather than `http://localhost:9090`

#### Scenario: In-app privacy link matches the store declaration

- **GIVEN** a store listing declaring a privacy policy URL in `app/store_metadata/`
- **WHEN** a release artifact for that store is cut
- **THEN** the privacy URL baked into the artifact SHALL equal the declared URL
- **AND** opening the privacy link from the location-consent dialog SHALL reach that URL

#### Scenario: Development fallbacks remain intact for local runs

- **WHEN** a developer runs `flutter run` with no `--dart-define=API_BASE_URL`
- **THEN** the app SHALL resolve `http://10.0.2.2:9090` on Android and `http://localhost:9090` elsewhere
- **AND** the runtime override from 伺服器設定 SHALL continue to take precedence over both

### Requirement: Release procedure SHALL verify the built artifact before upload

The release script for each platform SHALL, after the build and before any upload step, inspect the artifact it just produced and confirm that the expected production URLs are present in its Dart snapshot. When a URL is absent the script SHALL exit non-zero, report which URL was missing, and SHALL NOT upload.

The check SHALL read the artifact itself — `base/lib/*/libapp.so` inside the `.aab` for every ABI present, and `Payload/Runner.app/Frameworks/App.framework/App` inside the `.ipa` — rather than trusting the arguments the script believes it passed. A build can log the correct flags and still emit a binary without them; the artifact is what reaches users, so the artifact is what is asserted.

The check SHALL assert **presence** of the production URLs only. It SHALL NOT assert the absence of the development loopback addresses: those literals originate in `env.dart`'s own source and are present in every artifact regardless of which defines were supplied, so an absence check would fail on correctly built releases.

#### Scenario: Missing define stops the release

- **GIVEN** a release build produced without `--dart-define=API_BASE_URL`
- **WHEN** the release script's verification step runs
- **THEN** the script SHALL report the missing production API URL
- **AND** SHALL exit non-zero without uploading the artifact

#### Scenario: Correctly built artifact passes verification

- **GIVEN** a release build produced with both production defines supplied
- **WHEN** the verification step runs
- **THEN** the check SHALL pass despite the artifact also containing the development loopback literals
- **AND** the release SHALL proceed to its upload step

#### Scenario: Every ABI in the bundle is checked

- **GIVEN** an `.aab` carrying a Dart snapshot for more than one ABI
- **WHEN** the verification step runs
- **THEN** each `base/lib/<abi>/libapp.so` present SHALL be checked
- **AND** a missing URL in any one of them SHALL fail the release

#### Scenario: Unrecognised artifact layout fails closed

- **WHEN** the expected snapshot path cannot be found inside the artifact
- **THEN** the verification step SHALL treat this as a failure and stop the release
- **AND** SHALL NOT treat an unlocatable snapshot as a passing check
