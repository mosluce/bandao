## ADDED Requirements

### Requirement: Auth state transitions always terminate

Every method on the auth controller that mutates auth state (`login`, `logout`, `changePassword`, `refresh`, and the bootstrap `build`) SHALL leave the state machine in a terminal state — `authenticated`, `unauthenticated`, or `error` — on **every** exit path, including exceptions of types the method does not anticipate.

`AuthState.loading()` SHALL NOT be observable as a resting state. It exists only for the bootstrap window, during which no screen owns the failure; it SHALL NOT be entered by a user-initiated action that has a screen capable of reporting its own outcome.

This is a structural guarantee, not a per-exception fix: it is what prevents an unhandled failure anywhere on an auth path from stranding the user on a spinner with no error, no retry, and no route forward.

Where the controller cannot determine whether the user is authenticated, it SHALL resolve to `error`, which the router already directs to a screen with a retry affordance.

#### Scenario: An unexpected exception during login terminates the state machine

- **GIVEN** a user submits valid credentials and the API returns `200`
- **WHEN** any subsequent step throws an exception that is not an `ApiException` — a storage failure, a parse failure, or anything else
- **THEN** the auth state SHALL be a terminal state, never `loading`
- **AND** the user SHALL see an error with a way to proceed
- **AND** the app SHALL NOT display an indefinite spinner

#### Scenario: A user-initiated login does not unmount its own screen

- **WHEN** the user submits the login form
- **THEN** the auth state SHALL NOT transition to `loading`
- **AND** the login screen SHALL remain mounted for the duration of the request
- **AND** progress SHALL be shown by the screen's own submitting indicator

#### Scenario: The login screen reports failures it is still mounted to receive

- **GIVEN** a login attempt fails for any reason after the request is sent
- **WHEN** the failure reaches the login screen's error handling
- **THEN** the message SHALL be displayed on the form
- **AND** the submit button SHALL be re-enabled

#### Scenario: Bootstrap retains its loading window

- **WHEN** the app starts and the auth controller is resolving the stored token
- **THEN** `loading` remains the correct state and `/splash` remains the correct screen
- **AND** a failure there SHALL resolve to `error`, which `/splash` renders with a retry button

### Requirement: Secure-storage writes and deletes fail loudly and non-fatally

Writes and deletes to secure storage SHALL NOT propagate raw platform exceptions to their callers. Each SHALL surface failure as a typed storage failure that names the key involved and carries the underlying platform error.

A typed failure is required rather than a silently-absorbed one or a boolean return. Silent absorption is what made this class of bug invisible on the read path — a Keychain that cannot be read is indistinguishable from one holding nothing — and a boolean is ignorable by the next caller written.

Every secure-storage failure, read or write, SHALL be reported to the app's error-reporting sink so that a device with a broken keystore is diagnosable from telemetry rather than requiring a tethered debug build.

#### Scenario: A failing token write does not escape as a platform exception

- **GIVEN** the platform keystore rejects a write
- **WHEN** the bearer token is written after a successful login
- **THEN** the caller receives a typed storage failure naming the key
- **AND** no raw `PlatformException` escapes the storage wrapper

#### Scenario: Read failures remain fail-soft but are reported

- **GIVEN** a stored entry cannot be decrypted or read
- **WHEN** the app reads that key
- **THEN** the read still resolves as absent so the app can start
- **AND** the failure is reported to the error-reporting sink rather than discarded

#### Scenario: Delete failures do not break logout

- **GIVEN** the platform keystore rejects a delete
- **WHEN** the user logs out
- **THEN** local auth state is still cleared and the user reaches `/login`
- **AND** the storage failure is reported

### Requirement: The app accepts both internal and external AppUser identities

The client model of an AppUser SHALL treat `username` and `external_key` as optional and SHALL NOT require either to be present.

`AppUserDto` identifies an AppUser by **either** `username` (internal) **or** `external_key` (an external shadow user provisioned by `external-db-auth`), and omits whichever does not apply — the absent one is not sent as `null`, it is not sent at all. Requiring `username` therefore makes **every** login to an external-auth Org fail while parsing an otherwise successful `200` response, which the user cannot recover from and did nothing to cause.

Where the UI shows a machine-readable identity, it SHALL prefer `username`, fall back to `external_key`, and render nothing when neither is present — an external user's key in the customer's own system is the identifier they actually recognise.

#### Scenario: An external shadow user's login response parses

- **GIVEN** an Org whose `auth_source` is external, so none of its AppUsers has a `username`
- **WHEN** a user of that Org logs in and the API returns `200`
- **THEN** the response parses without error
- **AND** the session reaches the authenticated app

#### Scenario: Identity falls back to the external key

- **WHEN** the UI displays the identity of an AppUser that has `external_key` but no `username`
- **THEN** it shows the `external_key`

#### Scenario: Neither identifier renders nothing

- **WHEN** an AppUser payload carries neither `username` nor `external_key`
- **THEN** the identity line is omitted entirely rather than rendered blank

## MODIFIED Requirements

### Requirement: Login screen authenticates an AppUser via three-field form

The system SHALL provide a `/login` route showing a form with three required fields: `org_code`, `username`, `password`. Submission SHALL call `POST /app/auth/login` and on success store the returned bearer token at `auth.bearer_token` and the entered `org_code` at `auth.last_org_code` in secure storage. On subsequent visits to `/login`, the `org_code` field SHALL be pre-filled from the stored value. Field validation SHALL require all three fields to be non-empty before enabling the submit button.

A successful authentication whose token cannot be persisted SHALL still produce an authenticated session. The user is authenticated — the server issued the token and the process holds it — and the only thing lost is persistence across a cold start. The system SHALL therefore proceed to the authenticated app and SHALL surface a non-blocking notice stating the actual consequence in the user's terms (that they will need to log in again next time the app is opened), NOT a platform error code. Treating this as a login failure is prohibited: on a device whose keystore is permanently unwritable it would make the app impossible to use at all, discarding a session that is otherwise entirely valid.

A failure to store `auth.last_org_code` SHALL never affect the session or block navigation; it only pre-fills a form field.

#### Scenario: Successful login stores token and org_code

- **WHEN** the user submits valid `(org_code, username, password)` matching an active AppUser
- **THEN** the API returns 200 with `{ token, user, org, needs_password_change }`
- **AND** `auth.bearer_token` is written to secure storage
- **AND** `auth.last_org_code` is written to secure storage with the entered `org_code`
- **AND** the app navigates to `/` (or `/force-change-password` if the flag is true)

#### Scenario: Login succeeds but the token cannot be persisted

- **GIVEN** the API returns `200` with a valid token
- **WHEN** writing `auth.bearer_token` to secure storage fails
- **THEN** the app still navigates to `/` as an authenticated session
- **AND** a non-blocking notice tells the user they will need to log in again next time they open the app
- **AND** the failure is reported to the error-reporting sink
- **AND** the app does NOT show an indefinite spinner, and does NOT return the user to `/login`

#### Scenario: A failed org_code write is invisible to the user

- **GIVEN** the token was stored successfully
- **WHEN** writing `auth.last_org_code` fails
- **THEN** the session proceeds normally with no user-facing notice
- **AND** the only effect is that `org_code` is not pre-filled on a future visit to `/login`

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

### Requirement: Bearer-token reads tolerate iOS device-lock during background usage

The system SHALL keep the stored bearer token readable for the entire duration of an authenticated session regardless of whether the iOS device is locked, including during background-mode HTTP requests issued while the app process is alive but the screen is locked. The system SHALL achieve this by (a) configuring the iOS Keychain item that stores `auth.bearer_token` with an accessibility class that survives device lock after the first post-reboot unlock, and (b) ensuring `AuthInterceptor` and other authenticated request paths do not require synchronous Keychain availability on every request — a single successful Keychain read at session bootstrap is sufficient. The token SHALL still be encrypted at rest by iOS Keychain hardware-backed protection, scoped to this app's Keychain access group, and removable via the existing logout / 401 / handover-wipe paths.

Keychain unavailability SHALL be non-fatal in **both** directions. Reads already resolve as absent so the app can start; writes SHALL be equally survivable, per "Secure-storage writes and deletes fail loudly and non-fatally". A device whose Keychain cannot be written to SHALL remain usable for the current session rather than being locked out of the app.

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

#### Scenario: An unwritable Keychain does not lock the user out of the app

- **GIVEN** a device whose Keychain rejects every write
- **WHEN** the AppUser logs in with valid credentials
- **THEN** the session proceeds and the app is fully usable for that process lifetime
- **AND** the in-memory token continues to authorise outbound requests normally
