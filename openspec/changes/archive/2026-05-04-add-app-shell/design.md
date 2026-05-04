## Context

Every other surface of argus is implemented and specced. The mobile app has been a placeholder since day one. The api side now exposes everything an end-user app needs: AppUser identity (`/app/auth/login`, `/app/auth/logout`), self-introspection (`/app/me`), forced first-login password change (`/app/me/password` + the 423 gate), and the four checkin event types. AGENTS.md has long pinned the high-level architecture for `app/` (UI → state → repository → network via dio + future OpenAPI codegen) without naming concrete tools — this change locks them in.

We split the original "Flutter app for AppUsers" idea into two changes:

- **`add-app-shell` (this change)**: project bootstrap + auth + identity context. Land the framework, the API plumbing, and the login flow. Every primitive future changes will reuse — dio interceptors, Riverpod providers, `go_router` shell, secure-storage helpers, the error mapping — is established here.
- **`add-app-checkin` (next)**: home-screen action buttons, GPS permission, persistent event queue, queue processor, history view. Consumes the shell.

The split is enforced strictly: this change must not pre-build queue tables, GPS plumbing, or anything that smells of `add-app-checkin`. A reviewer should be able to land this and not be surprised by a mostly-empty `lib/features/checkin/`.

## Goals / Non-Goals

**Goals:**

- Get a real, runnable Flutter app on iOS Simulator + Android Emulator that an AppUser can log into and see themselves in.
- Pin the tech stack so `add-app-checkin` and onwards do not relitigate Riverpod vs BLoC, dio vs http, drift vs sqflite, or where state lives.
- Establish API client behaviour (Bearer auth, 401 → login redirect, 423 → force change) once and reuse it for every future `/app/*` feature.
- Make iteration painless for developers and testers: dart-define for build-time configs, debug-only dev menu for runtime overrides, sane platform defaults so nobody has to remember 10.0.2.2.
- Land CI that catches regressions early — `flutter analyze` + `flutter test` on every PR that touches `app/**`.

**Non-Goals:**

- Submitting checkin events. The home screen explicitly stubs the "current status" pill with "尚未實作" so the lack is obvious to testers.
- GPS, location permissions, persistent storage beyond secure_storage. drift is not added in this change; it lands with the queue in `add-app-checkin`.
- OpenAPI codegen. Models are hand-mirrored using freezed + json_serializable — same pattern admin-web uses today. The migration is a separate change once the API schema settles.
- Release signing, keystores, App Store / Play Store metadata, app icon design. We use Flutter debug signing and the default app-icon placeholder until product naming is final.
- Localization beyond `zh-TW`. ARB infrastructure is in place but only one locale ships.
- Dark mode. M3 light theme only.
- Biometric unlock, push notifications, deep links beyond the basic go_router routes.
- Multi-device session management (UI for "see all my logged-in devices"). The token + Bearer flow inherits this trivially when a future change exposes it on the API side.

## Decisions

### Riverpod 2 with code generation, not BLoC

Riverpod's `@riverpod` annotation + `riverpod_generator` produces typed providers with auto-disposal and zero boilerplate, while keeping the UI layer pure. Our state is shallow (auth + identity + a couple of UI bits) and Riverpod scales down well — we don't pay for ceremony we don't need.

BLoC was considered. It's good for complex form state and event sourcing, but for this app the win is moot: we have a few async fetches and a couple of mutations, not multi-step reducers. Riverpod's terser code wins.

Provider, GetX, signals were ruled out: Provider is superseded by Riverpod, GetX mixes concerns we want separated, signals are too new for this kind of foundational pick.

### `go_router` for navigation

Declarative router with a tree of `GoRoute` definitions, integrates cleanly with Riverpod (we listen to auth state and `redirect` accordingly), and is the official recommendation. Routes are a flat namespace for v1 (`/login`, `/force-change-password`, `/`); we'll nest them later if the app grows.

`auto_route` was considered for type-safe routes, but the codegen overhead and additional builder costs aren't justified for this app's size.

### dio for HTTP, with three interceptors

Three interceptors, kept small and single-purpose:

1. **AuthInterceptor**: injects `Authorization: Bearer <token>` from secure storage on outbound requests. No-op when there's no token.
2. **ErrorInterceptor**: catches dio errors, parses the API error envelope (`{ error: { code, message, retry_after? } }`), and re-throws as `ApiException`. Maps known codes to `ApiException` constants the UI layer recognizes (`InvalidCredentials`, `NeedsPasswordChange`, `Unauthorized`, etc).
3. **LogInterceptor (debug only)**: pretty-prints requests / responses / errors. Wrapped in `kDebugMode` so it compiles out of release builds.

Token storage and clear-on-401 logic live in the auth Riverpod provider, not in the interceptor — separation of concerns. The interceptor knows nothing about Riverpod or navigation.

### freezed + json_serializable for DTOs

Hand-mirrored models per AGENTS.md. `freezed` gives us immutability + sealed unions where useful (e.g. an `AuthState` sealed class), `json_serializable` gives us the `fromJson` / `toJson` boilerplate. Both lean on `build_runner`.

A future `add-openapi-codegen` change will swap these for generated models from the API's OpenAPI schema. Until then, the DTOs sit in a single `lib/core/api/models/` folder for easy migration.

### Secure storage scope: bearer token, last org_code, dev override

Three keys only:

- `auth.bearer_token` — set on login, cleared on logout / 401.
- `auth.last_org_code` — populated on successful login so the next visit pre-fills the field. Cleared on full logout.
- `dev.api_base_url_override` — debug builds only.

Anything more (refresh tokens, device-id, etc.) is a future concern.

### API base URL: dart-define + platform-aware default + debug-only runtime override

The Env class:

```dart
class Env {
  static const String _dartDefine = String.fromEnvironment('API_BASE_URL', defaultValue: '');

  static String compileTimeDefault() {
    if (_dartDefine.isNotEmpty) return _dartDefine;
    if (Platform.isAndroid) return 'http://10.0.2.2:9090';
    return 'http://localhost:9090';
  }
}
```

Resolution order at request time, evaluated by the dio provider:

1. If `kDebugMode` AND a dev override is stored → use override.
2. Else → `Env.compileTimeDefault()`.

In release builds the override branch is statically dead-code-eliminated (the secure-storage read is gated behind `kDebugMode`).

Why this layered approach: developers want zero-config "press F5" on a fresh checkout. Testers want to switch between staging URLs without rebuilding. CI wants explicit baked-in URLs in release builds. All three needs land cleanly.

### Forced password-change: navigation-driven, not modal

The 423 + `NEEDS_PASSWORD_CHANGE` response is intercepted by `ErrorInterceptor` and re-thrown as `ApiException.needsPasswordChange`. The UI catches this in the auth flow and *navigates* to `/force-change-password` (a full screen, no nav-bar back). After a successful `POST /app/me/password` the screen pops to `/`.

An overlay/modal would be tempting but breaks deep-linking semantics: the app should be in a clear state where the user knows they cannot escape until they change the password. A dedicated route surface this clearly and lets `go_router`'s redirect logic enforce it (any attempt to navigate away while `needs_password_change=true` redirects back).

### Auto-login flow

App startup:

```
1. Read stored token from secure storage.
2. If absent → show /login.
3. If present:
   a. Call GET /app/me.
   b. 200 → store identity in Riverpod auth state, navigate to / (or /force-change-password if flag set).
   c. 401 → clear stored token, navigate to /login.
   d. Network error → show splash with retry button (no auto-clear of token).
```

Why we don't trust a token age check on the client: the spec already gives sliding refresh on every authenticated request, so any successful `/app/me` extends the session. We don't need to track the expiry ourselves.

### Where the dev menu lives

A single hidden affordance: tap the "Argus" logo on the login screen 5 times. Opens `/dev-server-config`, a simple form with the current effective URL + an input + "save" + "clear override" buttons. Submit writes to secure storage; clearing it removes the key. A "current effective URL" line at the bottom of the login screen (debug builds only) shows what the app is actually pointing at, so testers can confirm.

In release builds the entire dev menu file is excluded from compilation via a `core/storage/dev_overrides_release.dart` shim that's swapped in by a conditional import. (Conditional imports are cleaner than runtime `kDebugMode` for excluding code, since they remove the imports too.)

### Error mapping is one place, not scattered

`lib/core/api/api_error.dart` defines `ApiException` and maps known `code` strings to enum cases. UI screens catch `ApiException` and use a tiny helper to look up the friendly Chinese string. There's no per-screen string-matching on `code` — the moment we have multiple screens dealing with `INVALID_CREDENTIALS` they should use the same translation.

### Logout is fire-and-forget on the network

`POST /app/auth/logout` is best-effort. On any response (including network failure or 401) the client clears the stored token + org_code and navigates to `/login`. We never block logout on a successful server reply — the worst case is a stale session row that expires in 14 days, which is fine.

### Riverpod auth state shape

Sealed `AuthState` via freezed:

```
AuthState.loading                     // app startup / pending /me
AuthState.unauthenticated             // no token or 401
AuthState.authenticated(AppUserDto user, OrgDto org, bool needsPasswordChange)
AuthState.error(String message)       // network failure on /me, retry available
```

`go_router` listens to this and redirects accordingly:

- `loading` → splash (nothing else)
- `unauthenticated` → `/login`
- `authenticated && needsPasswordChange` → `/force-change-password`
- `authenticated && !needsPasswordChange` → `/`
- `error` → `/login` with a retry banner (or splash, depending on whether token was stored)

### Folder layout (locked)

```
app/lib/
├── main.dart                         # entry: ProviderScope, run app
├── app/
│   └── argus_app.dart                # MaterialApp.router + theme + go_router
├── core/
│   ├── api/
│   │   ├── api_client.dart           # dio singleton + interceptors wiring
│   │   ├── auth_interceptor.dart
│   │   ├── error_interceptor.dart
│   │   ├── api_error.dart            # ApiException + code map
│   │   └── models/
│   │       ├── app_user.dart         # AppUserDto + status enum
│   │       ├── org.dart              # OrgDto incl. checkin sub-doc
│   │       └── auth_responses.dart   # LoginResponse, MeResponse
│   ├── env/
│   │   └── env.dart                  # API_BASE_URL resolver
│   ├── storage/
│   │   ├── secure_storage.dart       # typed wrapper
│   │   ├── dev_overrides.dart        # debug-only override read/write
│   │   └── dev_overrides_release.dart # conditional-import stub for release
│   └── result.dart                   # tiny Result<T,E> helper
├── features/
│   └── auth/
│       ├── data/
│       │   └── auth_repository.dart  # login, logout, me, change_password
│       ├── presentation/
│       │   ├── login_screen.dart
│       │   ├── force_password_change_screen.dart
│       │   ├── home_screen.dart      # placeholder for add-app-checkin
│       │   └── dev_server_config_screen.dart  # debug-only
│       └── state/
│           ├── auth_provider.dart    # @riverpod authState + actions
│           └── auth_state.dart       # freezed sealed class
├── shared/
│   ├── theme/
│   │   └── app_theme.dart            # M3 light
│   └── widgets/                      # status pill placeholder, loading view
└── l10n/
    ├── app_zh_TW.arb
    └── app_en.arb                    # only enough to satisfy ARB schema; not shipped
```

`features/checkin/` is intentionally absent. `add-app-checkin` creates it.

### Versioning + naming policy

- `pubspec.yaml`: `name: argus_app`, `version: 0.1.0+1`. We bump `+N` per build, `0.1.0` until the first staged release.
- Display name `Argus` in `ios/Runner/Info.plist` (`CFBundleDisplayName`) and Android `app/src/main/res/values/strings.xml` (`app_name`).
- Bundle ID `tw.ccmos.app.argus` set on both platforms.
- README documents the rename procedure when product name lands: 4 files to touch (`pubspec.yaml`, `Info.plist`, Android `applicationId` in `app/build.gradle`, strings.xml). The Info.plist `CFBundleDisplayName` is the only place that's visible to users.

### Testing scope

This change ships a small but real test suite, not a token one:

- Unit: `Env.compileTimeDefault` per-platform behaviour, `ApiException` parsing for known error codes, `AuthState` transitions in the provider.
- Widget: login screen renders, displays errors, navigates on success; force-change screen submits successfully and clears the flag; home screen shows identity from the auth provider.
- Integration tests skipped (no real server available in CI for now).

Mocking is via Riverpod's `ProviderContainer.overrideWithValue` — we don't pull in `mockito` / `mocktail` for v1.

### CI

`.github/workflows/app.yml`:

```
- triggers: PRs touching app/**
- steps:
  - checkout
  - subosito/flutter-action@v2 with channel: stable
  - flutter pub get
  - flutter analyze
  - flutter test
```

No build / artifact upload yet. That's a future change once we have a real release pipeline.

## Risks / Trade-offs

- **AppID may change before launch** → we document the rename procedure (4 files) and avoid hard-coding `tw.ccmos.app.argus` anywhere except platform manifests. Internal Dart code uses `Env.appId` if it ever needs to know. Pre-launch rename is cheap; post-launch (App Store / Play Store) is hard, but we're not there yet.
- **`flutter pub get` slow on first checkout** → CI caches `~/.pub-cache` and `app/.dart_tool/` between runs. Local devs incur a one-time ~2-minute hit.
- **Riverpod 2 codegen requires `build_runner watch` during dev** → README documents it. We accept the build-runner UX over the boilerplate of plain `Provider`s.
- **freezed + json_serializable build_runner step is brittle when models change** → mitigated by the small model surface (5 classes) and `dart run build_runner build --delete-conflicting-outputs` baked into the README.
- **Testing the dev menu** → it only exists in debug, hard to unit-test cleanly. We test the underlying `dev_overrides.dart` storage logic; the UI is verified by hand. Acceptable for v1.
- **Auto-login behaviour on token expiry mid-app-launch** → the splash → /me path handles it (401 clears token, sends to login). If `/me` succeeds but a later request 401s, the global auth listener catches it and redirects. Edge case: a request made *during* the redirect resolves with stale state — we accept this for v1; user just retries.
- **Deep linking** → not implemented in v1. `go_router` supports it later if push notifications or share-sheets become a thing.
- **Multi-device login** → the API supports multiple `app_sessions` rows per AppUser, and our logout only kills the current token. So the user can be logged in on phone + tablet simultaneously. We don't surface this in UI but the underlying behaviour is correct.
- **Localization stub** → we ship ARB files for `zh_TW` only. The empty `en.arb` is to satisfy `flutter gen-l10n`'s schema. If we add English later it's a translation-only PR, no scaffolding work.

## Migration Plan

There is no migration. `app/` was an empty `.gitkeep`. The change replaces it with a complete Flutter project.

If a developer has an unrelated tool open in `app/` (unlikely — it's empty), they'll need to `git pull` and `cd app && flutter pub get` to start using it. README spells this out.

No backwards compatibility concerns: there's no v0 to be backwards compatible with.

## Open Questions

None blocking implementation. Deferred but worth noting:

- Whether to add an iOS-only "Open settings" button surfacing the system Settings page when the user lands on a state that requires location permission (relevant for `add-app-checkin`, not this change).
- Whether to show the dev menu's "current effective URL" line in release builds too (probably not — but a "support" page that displays it could be useful for debugging customer issues).
- Whether to switch to riverpod's `Notifier` + `AsyncNotifier` everywhere or leave `@riverpod` annotation as the default. We start with the annotation; if the team finds it noisy, swap. Same code generation, just a class shape difference.
- Naming: the pubspec name is `argus_app`. If the product rename lands as e.g. `signbook`, do we rename the pubspec name too? Probably yes; documented in the README's rename section.
