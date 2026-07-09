# app-shell Specification

## Purpose
TBD - created by archiving change add-app-shell. Update Purpose after archive.
## Requirements
### Requirement: Flutter project lives at app/ with locked tech stack and platform identity

The repo SHALL contain a Flutter project at `app/` with `pubspec.yaml` declaring `name: bandao_app`, `version: 0.1.0+1`, Flutter SDK constraint `>= 3.24.0`, and Dart SDK constraint `>= 3.5.0 < 4.0.0`. The project SHALL declare these runtime dependencies and no others for v1: `flutter_riverpod`, `riverpod_annotation`, `go_router`, `dio`, `flutter_secure_storage`, `freezed_annotation`, `json_annotation`, `logger`, `flutter_localizations` (sdk). It SHALL declare these dev dependencies: `build_runner`, `riverpod_generator`, `freezed`, `json_serializable`, `flutter_lints`. The iOS bundle identifier and Android `applicationId` SHALL be `tw.ccmos.app.bandao`. The display name on iOS (`CFBundleDisplayName`) and Android (`app_name` string resource) SHALL be `班到`. The minimum supported iOS version SHALL be 13.0; the minimum supported Android API level SHALL be 24 (Android 7.0).

#### Scenario: Project metadata is correct

- **WHEN** a developer runs `flutter pub get` in `app/` on a fresh checkout
- **THEN** all listed dependencies resolve without conflict
- **AND** `pubspec.lock` exists and is committed to the repo

#### Scenario: Bundle identifiers match the spec on both platforms

- **WHEN** the iOS project's `Runner.xcodeproj` is inspected
- **THEN** `PRODUCT_BUNDLE_IDENTIFIER` for the `Runner` target equals `tw.ccmos.app.bandao`
- **AND** the Android `app/build.gradle` `applicationId` equals `tw.ccmos.app.bandao`

#### Scenario: Display name renders as "班到"

- **WHEN** the app is installed on iOS Simulator or Android Emulator
- **THEN** the home-screen icon label reads `班到`

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

### Requirement: Login screen authenticates an AppUser via three-field form

The system SHALL provide a `/login` route showing a form with three required fields: `org_code`, `username`, `password`. Submission SHALL call `POST /app/auth/login` and on success store the returned bearer token at `auth.bearer_token` and the entered `org_code` at `auth.last_org_code` in secure storage. On subsequent visits to `/login`, the `org_code` field SHALL be pre-filled from the stored value. Field validation SHALL require all three fields to be non-empty before enabling the submit button.

#### Scenario: Successful login stores token and org_code

- **WHEN** the user submits valid `(org_code, username, password)` matching an active AppUser
- **THEN** the API returns 200 with `{ token, user, org, needs_password_change }`
- **AND** `auth.bearer_token` is written to secure storage
- **AND** `auth.last_org_code` is written to secure storage with the entered `org_code`
- **AND** the app navigates to `/` (or `/force-change-password` if the flag is true)

#### Scenario: Login error renders friendly message

- **WHEN** the user submits credentials that the API rejects with `INVALID_CREDENTIALS`
- **THEN** the form shows the error "帳號、密碼或組織代碼錯誤" without distinguishing which field failed
- **AND** no values are written to secure storage

#### Scenario: org_code is pre-filled on subsequent visits

- **WHEN** the user previously logged in successfully with `org_code = "ABCDEFGHIJ"`
- **AND** the user later returns to `/login` (e.g. after logout, after token expiry)
- **THEN** the `org_code` field is initially populated with `"ABCDEFGHIJ"`

#### Scenario: Submit button gating

- **WHEN** any of `org_code`, `username`, `password` is empty
- **THEN** the submit button is disabled

### Requirement: Forced password-change flow gates the rest of the app

The system SHALL, when `/app/me` returns `needs_password_change=true`, route the user to `/force-change-password` and prevent navigation to any other route until the password is changed. The screen SHALL display two fields: `current_password` and `new_password` (≥ 8 characters). Submission SHALL call `POST /app/me/password`; on success the system SHALL refresh the auth state via `/app/me` and navigate to `/`. The route SHALL be unreachable when the AppUser's `needs_password_change` is already false.

#### Scenario: Forced screen appears after login when flag is set

- **WHEN** an AppUser logs in and `/app/me` returns `needs_password_change = true`
- **THEN** the app navigates to `/force-change-password`
- **AND** any attempt to navigate to `/` is redirected back to `/force-change-password`

#### Scenario: Successful change clears the flag and unblocks the app

- **WHEN** the user submits a correct `current_password` and a `new_password` of length ≥ 8
- **THEN** the API returns 204 and the auth state is refreshed
- **AND** the app navigates to `/`
- **AND** further navigation is unrestricted

#### Scenario: Wrong current password renders friendly message

- **WHEN** the API returns `INVALID_PASSWORD`
- **THEN** the form shows "目前密碼不正確"
- **AND** the password fields remain populated for retry

#### Scenario: Force route is unreachable when flag is false

- **WHEN** the AppUser's `needs_password_change` is false
- **AND** the user manually navigates to `/force-change-password`
- **THEN** the app redirects to `/`

### Requirement: Auto-login on app start uses the stored bearer token

The system SHALL, on each app start, read `auth.bearer_token` from secure storage. When absent, the system SHALL navigate to `/login`. When present, the system SHALL call `GET /app/me` with the token; on `200` the response SHALL hydrate the auth state (and `needs_password_change` may route to `/force-change-password`); on `401` the token SHALL be cleared and the user routed to `/login`; on network failure the system SHALL show a splash screen with a "重試" button without clearing the token.

#### Scenario: No token at startup → login

- **WHEN** the app starts and `auth.bearer_token` is absent
- **THEN** the user lands on `/login`

#### Scenario: Valid token at startup → home

- **WHEN** the app starts with a valid `auth.bearer_token`
- **AND** `/app/me` returns 200 with `needs_password_change = false`
- **THEN** the user lands on `/`
- **AND** the auth state contains the AppUser, Org, and `needs_password_change = false`

#### Scenario: Expired or invalid token at startup → login

- **WHEN** the app starts with a stored `auth.bearer_token`
- **AND** `/app/me` returns 401
- **THEN** the stored token is removed from secure storage
- **AND** the user lands on `/login`

#### Scenario: Network failure during startup shows retry

- **WHEN** the app starts with a stored token
- **AND** `/app/me` fails with a network error (no response from server)
- **THEN** the user sees a splash with "重試" and "登出"
- **AND** the token is NOT removed from secure storage
- **AND** "重試" re-runs the same flow

### Requirement: Logout is best-effort and always clears local state

The system SHALL, when the user taps "登出", call `POST /app/auth/logout` (best-effort) and ALWAYS clear `auth.bearer_token` + `auth.last_org_code` from secure storage afterwards regardless of the response. The user SHALL then be navigated to `/login`. The system SHALL NOT block logout on a successful server response.

#### Scenario: Successful logout clears local state

- **WHEN** the user taps "登出" and the API returns 204
- **THEN** `auth.bearer_token` and `auth.last_org_code` are removed from secure storage
- **AND** the user lands on `/login`

#### Scenario: Network-failed logout still clears local state

- **WHEN** the user taps "登出" and the network call fails
- **THEN** `auth.bearer_token` and `auth.last_org_code` are still removed from secure storage
- **AND** the user lands on `/login`

#### Scenario: Server-rejected logout still clears local state

- **WHEN** the user taps "登出" and the API returns 401 (token expired during the request)
- **THEN** `auth.bearer_token` and `auth.last_org_code` are still removed
- **AND** the user lands on `/login`

### Requirement: Authenticated requests carry a Bearer token via dio interceptor

The system SHALL include `Authorization: Bearer <token>` on every dio request whose path matches `/app/*` whenever `auth.bearer_token` is non-null. When the token is null, no `Authorization` header SHALL be set. Requests outside `/app/*` SHALL NOT receive the header even if a token is present.

#### Scenario: Token present, /app/me request includes header

- **WHEN** `auth.bearer_token` is `"abc"`
- **AND** the app issues a GET to `/app/me`
- **THEN** the request carries `Authorization: Bearer abc`

#### Scenario: Token absent, /app/auth/login request omits header

- **WHEN** `auth.bearer_token` is null
- **AND** the app issues a POST to `/app/auth/login`
- **THEN** the request does NOT carry an `Authorization` header

### Requirement: Dio error responses are normalized to ApiException

The system SHALL define an `ApiException` Dart class with fields `status: int`, `code: String`, `message: String`, `retryAfter: String?` and constructors / factories for known API error codes. The dio error interceptor SHALL parse `{ error: { code, message, retry_after? } }` payloads, raise an `ApiException`, and the rest of the app SHALL catch only `ApiException` (not raw `DioException`). The mapping SHALL include at minimum: `INVALID_CREDENTIALS`, `INVALID_PASSWORD`, `NEEDS_PASSWORD_CHANGE`, `UNAUTHORIZED`, `FORBIDDEN`, `VALIDATION`.

#### Scenario: 401 response is parsed into ApiException

- **WHEN** an API call returns `401` with `{"error":{"code":"UNAUTHORIZED","message":"unauthorized"}}`
- **THEN** the caller receives an `ApiException` with `status=401`, `code="UNAUTHORIZED"`, `message="unauthorized"`, `retryAfter=null`

#### Scenario: 423 NEEDS_PASSWORD_CHANGE is recognized

- **WHEN** an API call returns `423` with `{"error":{"code":"NEEDS_PASSWORD_CHANGE","message":"..."}}`
- **THEN** the caller receives an `ApiException` whose `code` equals `"NEEDS_PASSWORD_CHANGE"`
- **AND** the auth state listener routes the user to `/force-change-password`

#### Scenario: Network error becomes an ApiException with status 0

- **WHEN** the device has no network connectivity
- **AND** the app issues any request
- **THEN** the caller receives an `ApiException` with `status=0`, `code="NETWORK_ERROR"`

### Requirement: Home screen displays identity and stubs the future checkin status

The system SHALL provide a `/` route that displays the authenticated AppUser's `display_name`, `username`, and the current Org's `name`. The route SHALL include a "登出" action and a "尚未實作" placeholder for the checkin status pill that `add-app-checkin` will populate. Unauthenticated visitors to `/` SHALL be redirected to `/login`.

#### Scenario: Authenticated user sees their identity

- **WHEN** an authenticated user with `display_name = "Alice Chen"`, `username = "alice"`, `org.name = "Acme Corp"` visits `/`
- **THEN** the screen renders `Alice Chen` prominently, `alice` as the username, and `Acme Corp` as the Org

#### Scenario: Stub for checkin status is visible

- **WHEN** an authenticated user visits `/`
- **THEN** the screen displays a placeholder element labelled `尚未實作` where the checkin status will eventually appear

#### Scenario: Unauthenticated visit redirects to /login

- **WHEN** an unauthenticated request reaches `/`
- **THEN** the app redirects to `/login`

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

### Requirement: app/ has its own README and CI workflow

The repo SHALL include `app/README.md` describing how to run the project (`flutter pub get`, `dart run build_runner build`, `flutter run`), how to override the API base URL (`--dart-define=API_BASE_URL=...`), how to use the dev menu, and the renaming procedure for changing AppID / display name. The repo SHALL include `.github/workflows/app.yml` triggered on PRs that touch `app/**`, running `flutter pub get`, `flutter analyze`, and `flutter test`. The workflow SHALL fail the PR if any of these steps fails.

#### Scenario: README documents the standard run flow

- **WHEN** a developer reads `app/README.md`
- **THEN** they find commands for `flutter pub get`, `dart run build_runner build --delete-conflicting-outputs`, and `flutter run` with explanation of the platform-aware default URL

#### Scenario: CI runs analyze + test on PRs touching app/

- **WHEN** a PR modifies any file under `app/`
- **THEN** GitHub Actions runs `flutter analyze` and `flutter test` against the project
- **AND** the workflow blocks the PR on failure

#### Scenario: CI does not run on unrelated PRs

- **WHEN** a PR modifies only files under `api/` or `admin-web/`
- **THEN** the `app.yml` workflow does not trigger


### Requirement: Bearer-token reads tolerate iOS device-lock during background usage

The system SHALL keep the stored bearer token readable for the entire duration of an authenticated session regardless of whether the iOS device is locked, including during background-mode HTTP requests issued while the app process is alive but the screen is locked. The system SHALL achieve this by (a) configuring the iOS Keychain item that stores `auth.bearer_token` with an accessibility class that survives device lock after the first post-reboot unlock, and (b) ensuring `AuthInterceptor` and other authenticated request paths do not require synchronous Keychain availability on every request — a single successful Keychain read at session bootstrap is sufficient. The token SHALL still be encrypted at rest by iOS Keychain hardware-backed protection, scoped to this app's Keychain access group, and removable via the existing logout / 401 / handover-wipe paths.

#### Scenario: Foreground request after lock-and-resume sees the token

- **WHEN** an authenticated AppUser is using the app, locks the device, leaves the app backgrounded for at least 5 minutes, and unlocks + resumes
- **THEN** the next outbound `/app/*` request from `AuthInterceptor` SHALL include `Authorization: Bearer <token>`
- **AND** the user SHALL remain on `/` (or wherever they were) — they SHALL NOT be redirected to `/login`

#### Scenario: Background location batch fired while screen is locked attaches the bearer token

- **WHEN** an authenticated AppUser has 上班 active, the device is locked with the app backgrounded, and `LocationPingProcessor` flushes a batch to `POST /app/checkin/locations` while the screen is still locked
- **THEN** the request SHALL include `Authorization: Bearer <token>` (i.e. the token MUST be available without the user unlocking the device)
- **AND** the server SHALL NOT receive a request with a missing `Authorization` header from the locked-screen background path

#### Scenario: Logout invalidates cached token immediately

- **WHEN** the user taps 登出 (or a 401 from any path triggers `_onAuthExpired`)
- **THEN** subsequent reads of `auth.bearer_token` from `SecureStorage` SHALL return `null` without falling back to a stale cached value
- **AND** the underlying Keychain item SHALL also be cleared

#### Scenario: Login overwrites any previous cached token

- **WHEN** an AppUser logs in successfully and `writeToken(<new>)` is called
- **THEN** subsequent reads of the bearer token SHALL return `<new>` even if the device is locked between the write and the next read
- **AND** any previously cached value SHALL be replaced atomically with the new token
