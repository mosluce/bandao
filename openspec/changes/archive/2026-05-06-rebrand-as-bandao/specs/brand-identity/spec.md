## ADDED Requirements

### Requirement: Code identifiers use the `bandao` prefix consistently

The system SHALL use the ASCII identifier `bandao` (and the `tw.ccmos.app.bandao` reverse-DNS namespace where applicable) for all source-of-truth identifier strings. This includes Cargo / npm / pubspec package names, iOS bundle id and BGTask identifiers, Android application id and package, secure-storage key prefixes, and the HTTP `User-Agent` reported by the API to upstream services. Identifier strings SHALL NOT mix `argus` and `bandao`; any active code path that emits or consumes such strings SHALL use `bandao`.

#### Scenario: Cargo package name uses bandao

- **WHEN** a developer runs `cargo metadata` against the api crate
- **THEN** the package `name` field is `bandao-api`
- **AND** the binary `name` is `bandao-api`

#### Scenario: iOS bundle id uses bandao reverse-DNS

- **WHEN** an iOS build is produced
- **THEN** `CFBundleIdentifier` is `tw.ccmos.app.bandao`
- **AND** any `BGTaskSchedulerPermittedIdentifiers` entries are namespaced under `tw.ccmos.app.bandao.*`

#### Scenario: Secure storage prefix uses bandao

- **WHEN** the app reads or writes a key in `flutter_secure_storage`
- **THEN** the key starts with `bandao.` (e.g. `bandao.location_tracking.last_clean_stop`)

#### Scenario: API HTTP User-Agent uses bandao

- **WHEN** the API issues an outbound HTTP request to a third-party service (e.g., Nominatim)
- **THEN** the `User-Agent` header begins with `bandao-api/`

### Requirement: Display strings use the Chinese brand name `班到`

The system SHALL surface the Chinese brand string `班到` in all user-facing display contexts. This includes admin-web `<title>`, login screen heading, iOS `CFBundleDisplayName`, Android app label, and any in-product copy that previously used a brand name. The pinyin form `bandao` MAY appear in subtitles, attribution lines, or developer-facing surfaces; it SHALL NOT replace the Chinese form in primary display contexts.

#### Scenario: admin-web browser title

- **WHEN** an admin loads any admin-web page
- **THEN** the browser tab title contains `班到`

#### Scenario: iOS app springboard label

- **WHEN** the app is installed on iOS
- **THEN** the springboard label reads `班到`
- **AND** the launch screen / first surface uses `班到` for any branding text

#### Scenario: Android app drawer label

- **WHEN** the app is installed on Android
- **THEN** the launcher label reads `班到`
