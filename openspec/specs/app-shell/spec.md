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

The system SHALL define an `Env.compileTimeDefault()` resolver that returns: (1) the value of `String.fromEnvironment('API_BASE_URL')` when non-empty, (2) `http://10.0.2.2:9090` when running on Android, (3) `http://localhost:9090` otherwise. Release builds SHALL use only this value. Debug builds SHALL additionally consult a per-device override stored in secure storage; when present, that value SHALL take precedence over `Env.compileTimeDefault()`.

#### Scenario: Default URL on Android emulator

- **WHEN** the app is built for Android without `--dart-define=API_BASE_URL=...`
- **AND** there is no debug-only override stored
- **THEN** dio's base URL resolves to `http://10.0.2.2:9090`

#### Scenario: Default URL on iOS Simulator

- **WHEN** the app is built for iOS without `--dart-define=API_BASE_URL=...`
- **AND** there is no debug-only override stored
- **THEN** dio's base URL resolves to `http://localhost:9090`

#### Scenario: dart-define overrides platform default

- **WHEN** the app is built with `--dart-define=API_BASE_URL=https://api.example.com`
- **AND** there is no debug-only override stored
- **THEN** dio's base URL resolves to `https://api.example.com`

#### Scenario: Debug override beats dart-define

- **WHEN** the app is built in debug mode with `--dart-define=API_BASE_URL=https://api.example.com`
- **AND** the user previously saved `http://192.168.1.42:9090` via the dev menu
- **THEN** dio's base URL resolves to `http://192.168.1.42:9090`

#### Scenario: Release build excludes the override path

- **WHEN** the app is built in release mode
- **THEN** the conditional-import shim for `dev_overrides.dart` SHALL be the release stub
- **AND** no secure-storage read for `dev.api_base_url_override` happens at runtime

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

### Requirement: Debug-only dev menu allows runtime API URL override

The system SHALL, in debug builds only, expose a hidden "Server" configuration screen reachable by tapping the "Bandao" logo on the login screen 5 times within 3 seconds. The screen SHALL display the current effective base URL, allow the user to enter a new base URL, and provide "儲存" and "清除" actions. Saved values SHALL persist in secure storage at `dev.api_base_url_override` and take precedence over `Env.compileTimeDefault()` for all subsequent requests. Cleared values SHALL revert to the compile-time default. Release builds SHALL exclude this screen entirely.

#### Scenario: Hidden gesture opens the dev menu in debug

- **WHEN** the app is running in debug mode on the login screen
- **AND** the user taps the "Bandao" logo 5 times within 3 seconds
- **THEN** the dev server config screen opens

#### Scenario: Saved override is applied immediately

- **WHEN** the user enters `http://192.168.1.42:9090` and taps "儲存"
- **THEN** the value is persisted to `dev.api_base_url_override` in secure storage
- **AND** subsequent dio requests use this base URL

#### Scenario: Cleared override falls back to default

- **WHEN** the user taps "清除"
- **THEN** the `dev.api_base_url_override` key is removed from secure storage
- **AND** subsequent dio requests fall back to `Env.compileTimeDefault()`

#### Scenario: Hidden gesture is inert in release

- **WHEN** the app is running in release mode on the login screen
- **AND** the user taps the "Bandao" logo 5 times within 3 seconds
- **THEN** nothing happens (the dev menu is excluded from compilation)

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
