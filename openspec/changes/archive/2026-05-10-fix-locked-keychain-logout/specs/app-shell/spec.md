## ADDED Requirements

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
