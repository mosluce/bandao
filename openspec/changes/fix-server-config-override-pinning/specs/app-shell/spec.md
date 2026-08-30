## MODIFIED Requirements

### Requirement: Server configuration screen is reachable in all builds

The system SHALL provide a server-configuration screen, reachable in all build modes, that lets the user view the current effective API base URL, enter a new override, save it (subject to the build-mode validation in the base-URL requirement), and reset back to the compile-time default. Saving or resetting SHALL invalidate the API client and base-URL resolver so subsequent requests use the new value. The screen's development-only affordances (e.g. the Crashlytics self-test button) SHALL remain gated behind `kDebugMode`.

Each override input SHALL be seeded with the **stored override** when one exists, and SHALL be seeded **empty** when none does. It SHALL NOT be pre-filled with the compile-time default. The screen already displays the effective URL immediately above each input, so a blank input means "no override is stored" and is distinguishable from an override that happens to equal the default.

Saving a **blank** input SHALL clear the stored override rather than be rejected as malformed, matching the field's stated behaviour that leaving it empty uses the official default.

The **default SHALL always be the official URL** — `Env.compileTimeDefault()` for the base URL, `Env.privacyUrlCompileTimeDefault()` for the privacy URL. The default SHALL NOT be overridable: no user action redefines it, which is why resetting / clearing SHALL always restore that official value and never a previously saved one. What an override sets is the **current** value, as a separate layer consulted ahead of the default.

A value the user types SHALL be persisted as entered, including one that equals the current default; the screen SHALL NOT compare the input against the default in order to decide whether to store it. Protection against accidental pinning comes from the empty seeding above: with no override stored the field is blank, so opening the screen and saving without editing saves blank, which clears. There SHALL therefore be no path from an unedited save to a stored override.

The comparison that decides whether to clear the stored bearer token SHALL remain against the current **effective** URL, not the compile-time default: switching servers must drop a session issued by the previous one, and that includes switching from an override back to the default.

The privacy-URL override section SHALL remain available in all build modes, and its candidate values SHALL be validated by the same build-mode rules as the API base-URL override.

#### Scenario: Screen reachable in release

- **WHEN** the user opens the server-configuration screen from a release build
- **THEN** the screen renders with the current effective base URL and an input to change it
- **AND** it is not gated out of release builds

#### Scenario: Input starts empty when no override is stored

- **GIVEN** no override is stored for a field
- **WHEN** the user opens the server-configuration screen
- **THEN** that field's input SHALL be empty
- **AND** the current effective URL SHALL still be displayed above it

#### Scenario: Input shows the stored override when one exists

- **GIVEN** an override of `https://api.myco.com` is stored
- **WHEN** the user opens the server-configuration screen
- **THEN** the base-URL input SHALL contain `https://api.myco.com`

#### Scenario: Opening and saving without editing changes nothing

- **GIVEN** no override is stored
- **WHEN** the user opens the screen and saves without editing any field
- **THEN** no override SHALL be written
- **AND** the effective URL SHALL still resolve from the compile-time default
- **AND** a later app update that changes that default SHALL take effect on this device

#### Scenario: Blank save clears an existing override

- **GIVEN** an override of `https://api.myco.com` is stored
- **WHEN** the user clears the input and saves
- **THEN** the stored override SHALL be removed
- **AND** the effective URL SHALL revert to the compile-time default

#### Scenario: A typed value equal to the default is still stored

- **GIVEN** no override is stored
- **WHEN** the user types the exact compile-time default into the input and saves
- **THEN** that value SHALL be persisted as an override
- **AND** the input SHALL still show it when the screen is reopened, so the override is visible rather than silently discarded

#### Scenario: Resetting after that returns to the official default

- **GIVEN** an override equal to the compile-time default is stored
- **WHEN** the user chooses to reset / clear
- **THEN** the stored override SHALL be removed
- **AND** the effective URL SHALL resolve from the compile-time default again, so a later app update that changes it reaches this device

#### Scenario: Saving a valid override takes effect

- **WHEN** the user saves a valid override URL that differs from the compile-time default
- **THEN** the override is persisted
- **AND** the base-URL resolver and API client are invalidated so the next request uses the new URL

#### Scenario: Resetting reverts to the official default

- **WHEN** the user chooses to reset / clear the override
- **THEN** the stored override is cleared
- **AND** the effective base URL reverts to `Env.compileTimeDefault()`

#### Scenario: Release rejects a non-https privacy URL override

- **WHEN** the user attempts to save `http://policy.myco.com/privacy` as the privacy-URL override in a release build
- **THEN** the value SHALL be rejected with the same message the base-URL field uses
- **AND** no privacy-URL override SHALL be persisted

#### Scenario: Debug accepts a loopback privacy URL override

- **WHEN** the user saves `http://localhost:3000/privacy` as the privacy-URL override in a debug build
- **THEN** the value SHALL be accepted and persisted

### Requirement: API base URL resolves via dart-define with platform-aware default

The system SHALL define an `Env.compileTimeDefault()` resolver that returns: (1) the value of `String.fromEnvironment('API_BASE_URL')` when non-empty, (2) `http://10.0.2.2:9090` when running on Android, (3) `http://localhost:9090` otherwise. In **all** build modes the system SHALL additionally consult a per-device base-URL override stored in secure storage; when present and non-empty, that value SHALL take precedence over `Env.compileTimeDefault()`. When no override is stored, the effective base URL SHALL be `Env.compileTimeDefault()`.

The system SHALL validate a candidate override before persisting it, with rules that depend on build mode:

- In **release** builds the override SHALL be accepted only when it parses as a URI with `scheme == "https"` and a non-empty authority (host). Values with any other scheme (including `http`), no scheme, or no host SHALL be rejected.
- In **debug** builds the override SHALL be accepted when it parses as a URI with a scheme and a non-empty authority, permitting `http://`, `localhost`, and LAN IP addresses for local development.

The same validation SHALL apply to the per-device **privacy-URL** override. Both are URLs the app opens on the user's behalf, so a release build SHALL NOT persist a cleartext value for either.

The validator SHALL treat the empty string as malformed and SHALL NOT assign it a "clear the override" meaning. Callers that wish to clear SHALL detect a blank input and invoke the clear path before validating, keeping the validator a pure predicate over candidate URLs.

Because release only accepts `https`, the app SHALL NOT require any iOS App Transport Security or Android cleartext-traffic exception for the self-hosted-server case.

#### Scenario: Default URL on Android emulator

- **WHEN** the app is built for Android without `--dart-define=API_BASE_URL=...`
- **THEN** the effective base URL SHALL be `http://10.0.2.2:9090`
- **AND** there is no override stored

#### Scenario: Default URL on iOS Simulator

- **WHEN** the app is built for iOS without `--dart-define=API_BASE_URL=...`
- **THEN** the effective base URL SHALL be `http://localhost:9090`
- **AND** there is no override stored

#### Scenario: dart-define overrides platform default

- **WHEN** the app is built with `--dart-define=API_BASE_URL=https://bandao-api.ccmos.tw`
- **THEN** the effective base URL SHALL be `https://bandao-api.ccmos.tw`
- **AND** there is no override stored

#### Scenario: Stored override wins over compile-time default

- **GIVEN** the app was built with `--dart-define=API_BASE_URL=https://bandao-api.ccmos.tw`
- **AND** the user previously saved the override `https://api.myco.com`
- **THEN** the effective base URL SHALL be `https://api.myco.com`

#### Scenario: Release rejects a non-https override

- **WHEN** the user attempts to save `http://192.168.1.42:9090` as the override in a release build
- **THEN** the value SHALL be rejected
- **AND** no override SHALL be persisted

#### Scenario: Debug accepts a loopback http override

- **WHEN** the user saves `http://localhost:9090` as the override in a debug build
- **THEN** the value SHALL be accepted and persisted

#### Scenario: Empty string is not a valid override

- **WHEN** `validateBaseUrlOverride("")` is called in any build mode
- **THEN** it SHALL report the value as malformed
- **AND** the decision to clear a stored override SHALL be made by the caller, not by the validator
