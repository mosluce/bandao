## Why

`app/` has been a `.gitkeep` since the repo was bootstrapped. Every other surface in argus is now built — dashboard auth and tenancy, AppUser identity and admin tooling, the full checkin event surface — and the only thing missing is what AppUsers actually use: a phone app. Without it, the four-state checkin machine is exercised only via curl, and the live admin board has no real data flowing in.

Doing the Flutter bootstrap as its own change (split from `add-app-shell`'s sibling, `add-app-checkin`) keeps the scope reviewable: this change stops where login + identity context end and the checkin event surface begins. After this lands, an authenticated AppUser can open the app, see who they are and which Org they belong to, change their initial password, and log out — but they cannot yet submit checkin events. That comes next as `add-app-checkin`.

## What Changes

- New top-level `app/` Flutter project (replaces the `.gitkeep`):
  - **AppID** (iOS bundle ID + Android applicationId): `tw.ccmos.app.argus`. Provisional — the product name may change before launch; renaming is a one-line config flip and is explicitly allowed for now.
  - **Display name**: `Argus` (English, no localized override yet).
  - **Icon / splash**: placeholders (Flutter default with a single tinted swap). Final assets land when the product name is locked.
  - **Min OS**: iOS 13+, Android API 24 (Android 7.0)+. Standard Flutter defaults.
  - **Locale**: `zh-TW` only for v1 (ARB-driven, but a single locale).
  - **Theme**: Material 3 light only. Dark mode is a follow-up.

- **Tech stack** (locked here so `add-app-checkin` and onwards just consume):
  - Flutter latest stable / Dart 3.5+.
  - **State**: Riverpod 2 with code generation (`riverpod_generator`).
  - **Navigation**: `go_router` (declarative).
  - **HTTP**: `dio` with custom interceptors for Bearer auth, error mapping, and request/response logging in debug builds.
  - **Secure storage**: `flutter_secure_storage` (Keychain on iOS, Keystore on Android) for the bearer token, the last-used `org_code`, and the dev menu's API base URL override.
  - **Models**: hand-mirrored DTOs using `freezed` + `json_serializable`. OpenAPI codegen migration is a separate ROADMAP item.
  - **Logging**: `logger` package in app code; dio logging interceptor only in debug builds.
  - **Analyzer**: project `analysis_options.yaml` extending `flutter_lints` with a small custom layer.

- **API base URL configuration**:
  - Compile-time: `--dart-define=API_BASE_URL=...`.
  - Default fallback when not set: `http://10.0.2.2:9090` on Android (the host-loopback alias for the emulator), `http://localhost:9090` on iOS Simulator, `http://localhost:9090` for everything else.
  - Runtime override (debug builds only): a hidden "Server" page reachable from the login screen; the entered URL is persisted to secure storage and takes precedence over the dart-define / default. Release builds compile this code path out via `kDebugMode`.

- **Auth + identity surface (the entire change scope on the API side)**:
  - Login screen with three required fields: `org_code` (random Org code, active slug, or grace-period slug), `username`, `password`. After a successful login the org_code is remembered in secure storage so subsequent visits can pre-fill it.
  - Forced password-change screen, shown automatically when `/app/me` returns `needs_password_change=true` after login. The screen requires `current_password` (the initial password) and `new_password` (≥ 8 chars), then transitions to home.
  - Home screen (placeholder for `add-app-checkin`): displays the AppUser's `display_name`, `username`, the current Org's `name`, and a stub current-status pill that reads from the future checkin status endpoint when present (and shows "尚未實作" for v1). Has a logout action.
  - Auto-login: app start tries to load the stored bearer token, hits `/app/me`. If 200 → home (or the forced-change screen depending on the flag). If 401 → login screen.
  - Logout: hits `/app/auth/logout`, clears the stored token + org_code, returns to login.

- **API client behavior**:
  - Bearer interceptor injects `Authorization: Bearer <token>` for all `/app/*` requests when the token exists.
  - 401 → clear stored token + redirect to login (handled at navigation layer via Riverpod auth state listener).
  - 423 + `code: NEEDS_PASSWORD_CHANGE` → push the forced-change screen.
  - Generic `ApiException { status, code, message, retryAfter? }` raised from the dio error interceptor; UI maps it to friendly Chinese strings.

- **CI**: a single GitHub Actions workflow `.github/workflows/app.yml` running on PRs that touch `app/**`: `flutter analyze` + `flutter test`. No release pipeline yet.

- **Repo housekeeping**:
  - `app/.gitignore` covers Flutter / iOS / Android build artifacts.
  - `app/README.md` with run / build / dev menu instructions.
  - Top-level `README.md` and `AGENTS.md` are updated to reflect that `app/` is now real.

Out of scope (covered by `add-app-checkin` immediately after, or further-out ROADMAP items):

- Submitting any `/app/checkin/events` request, the persistent queue, or the queue processor.
- Reading `/app/checkin/status` or `/app/checkin/events` history (we leave the home screen's status pill as a stub here).
- GPS permission handling and the `geolocator` package.
- Background sync, push notifications, biometric unlock.
- iOS code signing / Android keystore configuration (dev signing only — the project uses Flutter's auto-generated debug certs).
- App Store / Play Store metadata, screenshots, listings.
- Final icon / splash assets (placeholders only until the product name lands).
- OpenAPI codegen migration (separate ROADMAP item).

## Capabilities

### New Capabilities

- `app-shell`: Flutter project scaffold + auth flow (login, force-change, auto-login, logout) + identity context (`/app/me`) + dev-menu API base URL override + the API client primitives (dio + Bearer interceptor + error mapping) that all future `/app/*` features will consume.

### Modified Capabilities

(none — this change introduces a new top-level `app-shell` capability. The `app-user-mgmt` API surface this consumes is already specified; no spec changes there.)

## Impact

- **Files**: a complete `app/` Flutter project (pubspec, lib/, android/, ios/, test/). Roughly 30–40 small files, mostly scaffolding.
- **Dependencies**: pubspec adds `flutter_riverpod`, `riverpod_annotation`, `go_router`, `dio`, `flutter_secure_storage`, `freezed_annotation`, `json_annotation`, `logger`, `flutter_localizations`. Dev deps: `build_runner`, `riverpod_generator`, `freezed`, `json_serializable`, `flutter_lints`. No new system packages required.
- **CI**: one new workflow at `.github/workflows/app.yml`; no changes to existing workflows.
- **Docs**: `app/README.md` is new. `README.md` and `AGENTS.md` get small updates referencing the new `app/` directory and pointing at it for onboarding.
- **Build artifacts**: `flutter pub get` populates `app/.dart_tool/` and `app/pubspec.lock`. The lock file is committed (so reviewers reproduce the same dependency graph); `.dart_tool/` and other generated dirs are gitignored.
- **No API or admin-web changes** in this change.
- **Renaming risk**: AppID `tw.ccmos.app.argus` is provisional. Changing it before launch is allowed; we document the rename procedure in `app/README.md` (touches `pubspec.yaml`, Android `applicationId`, iOS `PRODUCT_BUNDLE_IDENTIFIER`, and any place the display name appears).
- **Downstream**: `add-app-checkin` reuses every primitive from this change (the dio client, the Riverpod auth state, the navigation shell). It needs to add the queue + state + screens + GPS, but no further bootstrapping of the project itself.
