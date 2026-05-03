## 1. Project bootstrap

- [x] 1.1 From repo root: `rm app/.gitkeep && flutter create --org tw.ccmos.app --project-name argus_app --platforms=ios,android app` to scaffold the project. Confirm `app/pubspec.yaml`, `app/ios/Runner.xcodeproj`, `app/android/app/build.gradle` exist.
- [x] 1.2 Set `app/pubspec.yaml`: `version: 0.1.0+1`, Dart SDK constraint `'>=3.5.0 <4.0.0'`, Flutter SDK constraint `'>=3.24.0'`. Add an explicit `flutter:` section with `uses-material-design: true` and `generate: true` (for ARB).
- [x] 1.3 In `app/ios/Runner.xcodeproj/project.pbxproj` confirm `PRODUCT_BUNDLE_IDENTIFIER = tw.ccmos.app.argus` for both Debug and Release configurations of the `Runner` target. Adjust if `flutter create` set something else.
- [x] 1.4 In `app/android/app/build.gradle` confirm `applicationId "tw.ccmos.app.argus"` and `minSdkVersion 24`.
- [x] 1.5 In `app/ios/Runner/Info.plist` set `CFBundleDisplayName = "Argus"`. In `app/android/app/src/main/res/values/strings.xml` create the file (if missing) with `<string name="app_name">Argus</string>` and reference it from `AndroidManifest.xml` via `android:label="@string/app_name"`.
- [x] 1.6 Set iOS deployment target to 13.0 in both `app/ios/Podfile` (`platform :ios, '13.0'`) and the Xcode project's `IPHONEOS_DEPLOYMENT_TARGET` build setting.
- [x] 1.7 Replace the default app icon with a tinted placeholder (the Flutter default icon, but in Argus brand colour). Document the icon-replacement procedure in `app/README.md` for when the final asset arrives.
- [x] 1.8 Add `app/.gitignore` covering Flutter (`.dart_tool/`, `build/`, `.flutter-plugins`, `.flutter-plugins-dependencies`), iOS (`Pods/`, `Podfile.lock` is committed, `*.xcuserstate`, `*.xcworkspace/xcuserdata`), Android (`local.properties`, `key.properties`, `*.iml`), and IDE files (`.idea/`, `.vscode/launch.json` if not committed).
- [ ] 1.9 Verify the project runs on iOS Simulator and Android Emulator with `flutter run` after `flutter pub get`. (deferred — section 15 live smoke)

## 2. Dependencies + analyzer + lints

- [x] 2.1 Add runtime deps to `pubspec.yaml`: `flutter_riverpod`, `riverpod_annotation`, `go_router`, `dio`, `flutter_secure_storage`, `freezed_annotation`, `json_annotation`, `logger`, plus the SDK `flutter_localizations`. Use latest stable versions at the time of writing.
- [x] 2.2 Add dev deps: `build_runner`, `riverpod_generator`, `freezed`, `json_serializable`, `flutter_lints`. Lock to versions compatible with the runtime deps above.
- [x] 2.3 Replace generated `analysis_options.yaml` with one that includes `package:flutter_lints/flutter.yaml`, enables `prefer_relative_imports`, `prefer_single_quotes`, `require_trailing_commas`, `avoid_print`, and `unawaited_futures`. Document the rationale (consistency, safety) in a small comment block at the top of the file.
- [x] 2.4 Run `flutter pub get`; commit `pubspec.lock`.

## 3. Folder skeleton + entry point

- [x] 3.1 Create the directory layout under `app/lib/`: `app/`, `core/api/models/`, `core/env/`, `core/storage/`, `features/auth/{data,presentation,state}/`, `shared/{theme,widgets}/`, `l10n/`. Each directory gets a placeholder `_dir.dart` if Dart complains about empty folders (delete once real files arrive).
- [x] 3.2 `lib/main.dart`: initialize `runApp(ProviderScope(child: ArgusApp()))`. No business logic here.
- [x] 3.3 `lib/app/argus_app.dart`: `MaterialApp.router` wired to a `GoRouter` exposed via a Riverpod provider. Theme = `AppTheme.light()`. Locale = `Locale('zh','TW')`, with `flutter_localizations` delegates wired up. Title = `'Argus'`.
- [x] 3.4 `lib/shared/theme/app_theme.dart`: M3 light theme with a primary-colour seed. No bespoke widgets yet — defaults are fine for v1.
- [x] 3.5 `lib/l10n/app_zh_TW.arb` and `lib/l10n/app_en.arb` containing only the keys this change needs (login title, error messages, force-change copy, home stub, dev menu strings). Add `l10n.yaml` at `app/` for `flutter gen-l10n`. Generate once via `flutter gen-l10n` and commit the generated `lib/l10n/app_localizations.dart` if codegen sets it up that way (check current Flutter conventions). (Deviation: Flutter 3.29 gen-l10n hits a cwd bug with non-cwd project dirs; replaced with a hand-rolled `lib/l10n/app_localizations.dart` shim. ARB files retained as the source of truth for future codegen migration.)

## 4. Env + secure storage primitives

- [x] 4.1 `lib/core/env/env.dart`: define `class Env` with `compileTimeDefault()` returning the dart-defined value when non-empty, else `http://10.0.2.2:9090` on Android and `http://localhost:9090` on iOS / others. Use `dart:io` `Platform`. Add a const `appId = 'tw.ccmos.app.argus'` for any rare in-Dart use.
- [x] 4.2 `lib/core/storage/secure_storage.dart`: a thin typed wrapper around `FlutterSecureStorage`. Methods: `readToken`, `writeToken`, `clearToken`, `readLastOrgCode`, `writeLastOrgCode`, `clearLastOrgCode`. Each takes/returns `Future<String?>` / `Future<void>`. Riverpod-providable.
- [x] 4.3 `lib/core/storage/dev_overrides.dart`: debug-only readers / writers for `dev.api_base_url_override`. Do NOT import this directly anywhere; consumers import it through a conditional import.
- [x] 4.4 `lib/core/storage/dev_overrides_release.dart`: release stub exposing the same API surface but always returning `null` and being a no-op on writes. The condition import in consumers picks one or the other based on `dart.library.io` (or use `kDebugMode` checks at the call site — pick whichever is cleaner; document the choice in a comment). (Deviation: `dart.library.X` only switches on web vs mobile, not debug vs release. Used `kReleaseMode` early-returns inside `dev_overrides.dart` instead — `kReleaseMode` is a const so the entire branch tree-shakes out in release. No separate release stub file needed; choice is documented inline.)
- [x] 4.5 `lib/core/storage/api_base_url.dart` (or similar): exposes `effectiveBaseUrl()` Future returning the override when present (debug) else `Env.compileTimeDefault()`. This is what dio reads.

## 5. API client + interceptors + error mapping

- [x] 5.1 `lib/core/api/api_error.dart`: define `class ApiException implements Exception { final int status; final String code; final String message; final String? retryAfter; }` with named constructors / factory matching common codes (`invalidCredentials`, `invalidPassword`, `needsPasswordChange`, `unauthorized`, `forbidden`, `validation`, `network`). Add a `friendlyZh()` extension or method that maps `code` to a Chinese string used by the UI.
- [x] 5.2 `lib/core/api/auth_interceptor.dart`: dio `Interceptor` that reads the bearer token (via injected `SecureStorage`) and adds `Authorization: Bearer <token>` ONLY when the request path starts with `/app/`.
- [x] 5.3 `lib/core/api/error_interceptor.dart`: dio `Interceptor` that catches `DioException` and either parses `{ error: { code, message, retry_after? } }` from the response or maps network errors to `ApiException(status: 0, code: 'NETWORK_ERROR', ...)`. Re-throws as `ApiException`.
- [x] 5.4 `lib/core/api/log_interceptor.dart` (debug only): pretty-prints requests / responses / errors. Wrapped via `if (kDebugMode)` at registration time so the import is harmless in release.
- [x] 5.5 `lib/core/api/api_client.dart`: builds a `Dio` instance with the resolved base URL, sane timeouts (10s connect / 15s receive), and the three interceptors registered in order: log → auth → error. Riverpod-providable; rebuilds when the resolved base URL changes (e.g. dev menu save).
- [x] 5.6 `lib/core/api/models/app_user.dart`: freezed `AppUser` mirroring api side (`id, username, display_name, status, needs_password_change, last_login_at?, created_at`). `status: AppUserStatus { active, disabled }` enum with snake_case JSON. (Deviation: hand-rolled value class instead of freezed — see 5.9.)
- [x] 5.7 `lib/core/api/models/org.dart`: freezed `Org` mirroring api side (`id, name, code, owner_id, timezone, checkin: { transfer_enabled }, slug?, slug_changed_at?`). (Deviation: hand-rolled value class.)
- [x] 5.8 `lib/core/api/models/auth_responses.dart`: freezed `LoginResponse { token, expires_at, user, org, needs_password_change }` and `MeResponse { user, org, needs_password_change }`. (Deviation: hand-rolled value classes.)
- [x] 5.9 Run `dart run build_runner build --delete-conflicting-outputs`; commit the generated `*.freezed.dart` and `*.g.dart` files (or add them to .gitignore — check current Flutter convention; we lean COMMIT them so PR review shows the generated surface). (Deviation: build_runner can't run from this Claude Code sandbox because `cd <dir> && dart run` is not in the bash allowlist and `dart pub -C` doesn't change `Directory.current` for build_runner. Switched the five DTOs and the upcoming AuthState sealed class to hand-rolled Dart classes — equivalent semantics, immutable + value equality + JSON conversion, fewer moving parts. freezed/json_serializable/build_runner removed from pubspec. A future `add-openapi-codegen` change will replace these anyway.)

## 6. Auth state + repository + provider

- [x] 6.1 `lib/features/auth/state/auth_state.dart`: freezed sealed `AuthState` with cases `loading`, `unauthenticated`, `authenticated(AppUser user, Org org, bool needsPasswordChange)`, `error(String message)`. (Deviation: Dart 3 `sealed class` instead of freezed — no codegen, same exhaustiveness.)
- [x] 6.2 `lib/features/auth/data/auth_repository.dart`: methods `Future<LoginResponse> login(orgCode, username, password)`, `Future<void> logout()`, `Future<MeResponse> me()`, `Future<void> changePassword(currentPassword, newPassword)`. Each is a thin wrapper around the dio client that throws `ApiException` on errors.
- [x] 6.3 `lib/features/auth/state/auth_provider.dart`: `@riverpod` `AuthNotifier` exposing `state: AuthState`, methods `login(...)`, `logout()`, `changePassword(...)`, and an internal `_bootstrap()` called on construction that runs the auto-login flow (read token → call `/app/me` → set state). State transitions match the cases in `auth_state.dart`. (Deviation: hand-written `AsyncNotifierProvider` instead of `@riverpod` codegen — same shape, no build_runner.)

## 7. go_router + redirect logic

- [x] 7.1 `lib/app/router.dart`: define the `GoRouter` with routes `/login`, `/force-change-password`, `/`, `/dev-server-config` (debug only). Provide it as a Riverpod provider that `argus_app.dart` consumes.
- [x] 7.2 Implement `redirect` against the auth provider's state:
  - `loading` → no redirect (let the splash render at whatever path)
  - `unauthenticated` → redirect any non-`/login`, non-`/dev-server-config` path to `/login`
  - `authenticated && needsPasswordChange` → redirect any non-`/force-change-password` path to `/force-change-password`
  - `authenticated && !needsPasswordChange` → redirect `/login` and `/force-change-password` to `/`
  - `error` → redirect to `/login` (the screen will surface a banner)
- [x] 7.3 Verify via a small widget test that the redirects fire: with overridden auth state, `goNamed('/')` while unauthenticated lands on `/login`.

## 8. Login screen

- [x] 8.1 `lib/features/auth/presentation/login_screen.dart`: form with three required `TextFormField`s. The `org_code` field initial value is loaded from `secureStorage.readLastOrgCode()` once on first build.
- [x] 8.2 Submit button is disabled until all three fields have non-empty trimmed values. On submit, call `authNotifier.login(...)`. Loading state disables the form.
- [x] 8.3 Map `ApiException` to friendly strings via `ApiException.friendlyZh()`. `INVALID_CREDENTIALS` → "帳號、密碼或組織代碼錯誤". `NETWORK_ERROR` → "連線失敗，請確認網路". Other codes → use the API message.
- [x] 8.4 The Argus logo / title widget at the top of the screen tracks rapid taps; on the 5th tap within a rolling 3-second window it navigates to `/dev-server-config` (only when `kDebugMode` is true).
- [x] 8.5 In debug builds only, show a small "API: <effective URL>" line at the bottom of the login screen.

## 9. Force-change-password screen

- [x] 9.1 `lib/features/auth/presentation/force_password_change_screen.dart`: two `TextFormField`s (`current_password`, `new_password`). `new_password` requires length ≥ 8 (client-side hint, server enforces).
- [x] 9.2 Submit calls `authNotifier.changePassword(...)`. On success, the notifier refreshes via `/app/me`, which clears the flag and `go_router` navigates to `/`.
- [x] 9.3 `INVALID_PASSWORD` → "目前密碼不正確". Validation errors → use API message. Other codes → friendly fallback.
- [x] 9.4 Disable any "back" affordance (no AppBar back button); the redirect logic also catches programmatic exits.

## 10. Home screen (placeholder for add-app-checkin)

- [x] 10.1 `lib/features/auth/presentation/home_screen.dart`: an authenticated route showing `display_name` prominently, `username` (mono font) and `org.name` underneath.
- [x] 10.2 A status-pill placeholder rendering "尚未實作" with explanatory subtitle. Wire it as a separate widget so `add-app-checkin` can replace it cleanly.
- [x] 10.3 An overflow menu (or single button) with "登出". On tap, call `authNotifier.logout()` (which navigates to `/login`).
- [x] 10.4 A small "了解更多" footer text linking to `app/README.md` style instructions, OR omit if it adds noise. (Omitted — see task; org name shows in AppBar instead, no footer noise.)

## 11. Dev server config screen

- [x] 11.1 `lib/features/auth/presentation/dev_server_config_screen.dart` (only built in debug mode — guarded by conditional import or `kDebugMode` check at route registration time).
- [x] 11.2 Display: current effective base URL (read live), a `TextFormField` for the new value (pre-filled with the override if set, else with `Env.compileTimeDefault()`), buttons "儲存" and "清除".
- [x] 11.3 "儲存": validate as a parseable URL, then call `devOverrides.write(value)`. On success, pop and force-rebuild the dio provider (so subsequent requests pick up the new URL).
- [x] 11.4 "清除": call `devOverrides.clear()`, pop, force-rebuild dio.

## 12. Tests

- [ ] 12.1 `test/core/env/env_test.dart`: assert the dart-define / Android / iOS branches of `Env.compileTimeDefault()`. (Use a mock `Platform` or factor the OS check into an injectable function so it's testable.)
- [ ] 12.2 `test/core/api/api_error_test.dart`: feed sample dio error responses for each known code into the parser; assert the resulting `ApiException` shape. Include a network-error case (dio `connectionError`).
- [ ] 12.3 `test/features/auth/state/auth_provider_test.dart`: exercise the `AuthNotifier` state transitions with a fake `AuthRepository` — auto-login success / 401 / network error, login success / `INVALID_CREDENTIALS`, logout (success and network failure both clear state).
- [ ] 12.4 `test/features/auth/presentation/login_screen_test.dart`: pump the screen with overridden providers; assert disabled submit when fields empty, enabled when all three present, error rendering on `INVALID_CREDENTIALS`, navigation on success.
- [ ] 12.5 `test/features/auth/presentation/force_password_change_screen_test.dart`: pump screen, assert submit disabled until both fields filled, error renders for `INVALID_PASSWORD`, success path completes.
- [ ] 12.6 `test/app/router_test.dart`: with overridden auth states, assert that the redirects match the spec (unauthenticated → /login; needsPasswordChange → /force-change-password; etc).

## 13. CI

- [ ] 13.1 `.github/workflows/app.yml`: workflow triggered on `pull_request` with `paths: [app/**]` and on `push` to `main` with the same paths. Runs on `ubuntu-latest`. Steps: checkout, `subosito/flutter-action@v2` with `channel: stable`, `flutter pub get`, `dart run build_runner build --delete-conflicting-outputs`, `flutter analyze`, `flutter test`.
- [ ] 13.2 Cache `~/.pub-cache` and `app/.dart_tool/` between runs keyed off `pubspec.lock`'s hash to keep CI fast.
- [ ] 13.3 Verify the workflow does NOT fire on PRs that only touch `api/`, `admin-web/`, or `openspec/`.

## 14. Docs

- [ ] 14.1 `app/README.md`: how to run (`flutter pub get`, `dart run build_runner build`, `flutter run`); the platform-aware default base URL (10.0.2.2 vs localhost); how to override via `--dart-define=API_BASE_URL=...`; how to use the dev menu (5-tap on logo); the renaming procedure (4 files: pubspec.yaml, Info.plist, build.gradle applicationId, strings.xml app_name) for when the product name lands; how to add a new locale.
- [ ] 14.2 Update `README.md` (repo root) to describe `app/` as a real, runnable Flutter project (replacing the previous placeholder bullet) and link to `app/README.md`.
- [ ] 14.3 Update `AGENTS.md` `app/`(Flutter) section if needed: lock `Riverpod` + `go_router` + `dio` as the chosen tools. Strike-through the "依後續決定" wording.

## 15. Smoke

- [ ] 15.1 `flutter analyze` clean, `flutter test` all green locally.
- [ ] 15.2 Live smoke: bring up local stack (api on `:9090`, mongo via docker compose). On iOS Simulator, run `flutter run` in `app/`. Walk: see splash → land on `/login` → enter `(org_code, username, initial_password)` → see force-change screen → change password → land on home → see identity → tap logout → return to `/login`. Do the same on Android Emulator (default URL should resolve to `10.0.2.2:9090`). Try the dev menu: tap logo 5x → enter a different URL → save → confirm subsequent login uses it; clear → confirm fallback.
