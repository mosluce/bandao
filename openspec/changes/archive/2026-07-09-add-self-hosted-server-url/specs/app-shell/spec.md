## MODIFIED Requirements

### Requirement: API base URL resolves via dart-define with platform-aware default

The system SHALL define an `Env.compileTimeDefault()` resolver that returns: (1) the value of `String.fromEnvironment('API_BASE_URL')` when non-empty, (2) `http://10.0.2.2:9090` when running on Android, (3) `http://localhost:9090` otherwise. In **all** build modes the system SHALL additionally consult a per-device base-URL override stored in secure storage; when present and non-empty, that value SHALL take precedence over `Env.compileTimeDefault()`. When no override is stored, the effective base URL SHALL be `Env.compileTimeDefault()`.

The system SHALL validate a candidate override before persisting it, with rules that depend on build mode:

- In **release** builds the override SHALL be accepted only when it parses as a URI with `scheme == "https"` and a non-empty authority (host). Values with any other scheme (including `http`), no scheme, or no host SHALL be rejected.
- In **debug** builds the override SHALL be accepted when it parses as a URI with a scheme and a non-empty authority, permitting `http://`, `localhost`, and LAN IP addresses for local development.

Because release only accepts `https`, the app SHALL NOT require any iOS App Transport Security or Android cleartext-traffic exception for the self-hosted-server case.

#### Scenario: Default URL on Android emulator

- **WHEN** the app is built for Android without `--dart-define=API_BASE_URL=...`
- **AND** there is no override stored
- **THEN** dio's base URL resolves to `http://10.0.2.2:9090`

#### Scenario: Default URL on iOS Simulator

- **WHEN** the app is built for iOS without `--dart-define=API_BASE_URL=...`
- **AND** there is no override stored
- **THEN** dio's base URL resolves to `http://localhost:9090`

#### Scenario: dart-define overrides platform default

- **WHEN** the app is built with `--dart-define=API_BASE_URL=https://api.example.com`
- **AND** there is no override stored
- **THEN** dio's base URL resolves to `https://api.example.com`

#### Scenario: Override beats compile-time default in release

- **WHEN** the app is a release build with compile-time default `https://api.bandao.example.com`
- **AND** the user previously saved the override `https://api.myco.com`
- **THEN** dio's base URL resolves to `https://api.myco.com`

#### Scenario: Release rejects a non-https override

- **WHEN** the user attempts to save `http://192.168.1.42:9090` as the override in a release build
- **THEN** the value is rejected and not persisted
- **AND** an error indicating an `https` URL is required is shown

#### Scenario: Debug accepts a loopback http override

- **WHEN** the user saves `http://localhost:9090` as the override in a debug build
- **THEN** the value is persisted
- **AND** dio's base URL resolves to `http://localhost:9090`

## REMOVED Requirements

### Requirement: Debug-only dev menu allows runtime API URL override

**Reason**: Superseded by the self-hosted-server feature. The override screen is no longer debug-only, no longer hidden behind a 5-tap logo gesture, and release builds now include it (subject to https-only validation). Its behavior is now specified by the added "Server configuration screen is reachable in all builds" and "Login screen surfaces server configuration and current connection" requirements. The old "Release build excludes the override path" scenario is likewise dropped — folded into the modified base-URL requirement.

**Migration**: The 5-tap gesture is replaced by a visible "伺服器設定" entry; the storage key moved from `dev.api_base_url_override` to `server.api_base_url`. Behavior is unchanged for users who never set an override — the effective base URL remains `Env.compileTimeDefault()`.

## ADDED Requirements

### Requirement: Server configuration screen is reachable in all builds

The system SHALL provide a server-configuration screen, reachable in all build modes, that lets the user view the current effective API base URL, enter a new override, save it (subject to the build-mode validation in the base-URL requirement), and reset back to the compile-time default. Saving or resetting SHALL invalidate the API client and base-URL resolver so subsequent requests use the new value. The screen's development-only affordances (e.g. the Crashlytics self-test button) SHALL remain gated behind `kDebugMode`.

#### Scenario: Screen reachable in release

- **WHEN** the user opens the server-configuration screen from a release build
- **THEN** the screen renders with the current effective base URL and an input to change it
- **AND** it is not gated out of release builds

#### Scenario: Saving a valid override takes effect

- **WHEN** the user saves a valid override URL
- **THEN** the override is persisted
- **AND** the base-URL resolver and API client are invalidated so the next request uses the new URL

#### Scenario: Resetting reverts to the official default

- **WHEN** the user chooses to reset / clear the override
- **THEN** the stored override is cleared
- **AND** the effective base URL reverts to `Env.compileTimeDefault()`

### Requirement: Login screen surfaces server configuration and current connection

The system SHALL, on the `/login` screen, provide a low-key entry point (visible in all build modes) that navigates to the server-configuration screen, and SHALL display which server the app is currently pointed at: the label for "official default" when the effective base URL equals `Env.compileTimeDefault()`, otherwise a label naming the custom host. When the user saves an override whose value differs from the current effective base URL, the system SHALL clear the stored bearer token (and dependent auth state) so the user must re-authenticate against the new server; the stored `last_org_code` MAY be retained.

#### Scenario: Server-config entry is visible in release

- **WHEN** the user is on `/login` in a release build
- **THEN** a "伺服器設定" entry point is visible and navigates to the server-configuration screen
- **AND** it does not depend on a debug-only gesture

#### Scenario: Login screen shows the official default

- **WHEN** the effective base URL equals `Env.compileTimeDefault()`
- **THEN** the login screen indicates the app is connected to the official default server

#### Scenario: Login screen shows a custom server

- **WHEN** the effective base URL is a saved override `https://api.myco.com`
- **THEN** the login screen indicates a custom connection naming the host `api.myco.com`

#### Scenario: Changing the server clears the session

- **WHEN** the user is logged in with a bearer token issued by server A
- **AND** the user saves a different base URL for server B
- **THEN** the stored bearer token is cleared
- **AND** the user is required to log in again against server B
