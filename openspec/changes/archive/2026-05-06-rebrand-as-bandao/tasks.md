## 1. Audit

- [x] 1.1 Run `grep -rl "argus\|Argus\|ARGUS" /Volumes/Backup/Workspace/ccmos/argus --include="*.rs" --include="*.ts" --include="*.tsx" --include="*.vue" --include="*.dart" --include="*.toml" --include="*.yaml" --include="*.yml" --include="*.json" --include="*.md" --include="*.swift" --include="*.kt" --include="*.gradle" --include="*.plist" --include="*.xml" --exclude-dir=node_modules --exclude-dir=target --exclude-dir=build --exclude-dir=.dart_tool --exclude-dir=.output --exclude-dir=archive` and capture the file list. Skip everything under `openspec/changes/archive/` (historical).
- [x] 1.2 Group hits by category: (a) package identifiers, (b) bundle / DNS namespaces, (c) storage / runtime keys, (d) UA / outbound strings, (e) UI display text, (f) docs / READMEs, (g) test assertion strings.

## 2. API rust crate

- [x] 2.1 `api/Cargo.toml`: `name = "bandao-api"` (package + `[[bin]]`).
- [x] 2.2 `api/src/services/reverse_geocoder.rs`: `NOMINATIM_USER_AGENT` → `"bandao-api/0.1.0 (https://github.com/mosluce/bandao)"`.
- [x] 2.3 `api/src/main.rs`, `api/src/db/mod.rs`: any `argus_api`, `argus-api`, `app_name = "argus-api"` → bandao.
- [x] 2.4 `api/src/state.rs` / wherever app_name is set: `"argus-api"` → `"bandao-api"`.
- [x] 2.5 `api/Cargo.lock`: regenerated after `cargo build` — committed.
- [x] 2.6 `api/README.md`: replace top-line title, scattered references; keep historical archived-change references untouched.

## 3. admin-web (Nuxt)

- [x] 3.1 `admin-web/package.json`: `"name": "bandao-admin-web"`.
- [x] 3.2 `admin-web/pnpm-lock.yaml`: regenerated after `pnpm install`.
- [x] 3.3 `admin-web/nuxt.config.ts`: `app.head.title` → `"班到 admin"`.
- [x] 3.4 `admin-web/pages/login.vue` (or wherever the "argus admin" h1 / brand mark lives): `"argus admin"` → `"班到"` or `"班到 admin"` consistently.
- [x] 3.5 `admin-web/pages/no-org.vue`: same — replace `"argus admin"` h1.
- [x] 3.6 `admin-web/pages/privacy.vue`: any `"Argus"` / `"argus"` in policy text → `"班到"` (with optional `(bandao)` parenthetical).
- [x] 3.7 `admin-web/README.md`: title + scattered references.
- [x] 3.8 Search any other components (`OrgCreateForm.vue`, `OrgJoinForm.vue`, `OrgSwitcher.vue`) for stray brand strings.

## 4. Flutter app

- [x] 4.1 `app/pubspec.yaml`: `name: bandao_app`. Bump version optional.
- [x] 4.2 Run `dart run build_runner build` — drift code regeneration. (Drift output uses package name.)
- [x] 4.3 Replace ALL `package:argus_app/` import paths with `package:bandao_app/` across `app/lib/**` and `app/test/**`. Use sed:
  ```bash
  find app/lib app/test -name "*.dart" -exec sed -i '' 's|package:argus_app/|package:bandao_app/|g' {} +
  ```
- [x] 4.4 `app/lib/main.dart`, `app/lib/app/argus_app.dart` → consider renaming the file `argus_app.dart` to `bandao_app.dart` and update import paths. (Class name `ArgusApp` → `BandaoApp`.)
- [x] 4.5 `app/lib/core/storage/secure_storage.dart`: storage key constants — `argus.location_tracking.*` → `bandao.location_tracking.*`. Audit every key.
- [x] 4.6 `app/lib/core/storage/privacy_url.dart`: any `argus` in dart-define names (`ARGUS_*` → `BANDAO_*`).
- [x] 4.7 `app/lib/core/env/env.dart`: similar — env / dart-define names.
- [x] 4.8 `app/lib/l10n/app_localizations.dart`: any `argus` / `Argus` in user-facing strings → `班到`.
- [x] 4.9 iOS `app/ios/Runner/Info.plist`:
  - `CFBundleName` / `CFBundleExecutable` → `bandao_app`
  - `CFBundleDisplayName` → `班到`
  - `BGTaskSchedulerPermittedIdentifiers` → `tw.ccmos.app.bandao.queue-drain`
  - any `NSLocationWhenInUseUsageDescription` referencing brand name
- [x] 4.10 iOS `app/ios/Runner.xcodeproj/project.pbxproj`: `PRODUCT_BUNDLE_IDENTIFIER` → `tw.ccmos.app.bandao`. Keep Team ID etc. intact.
- [x] 4.11 iOS `app/ios/Podfile.lock` may not need changes; regenerate via `pod install` if Podfile references the project name.
- [x] 4.12 Android `app/android/app/src/main/AndroidManifest.xml`: package + label → `tw.ccmos.app.bandao` / `班到`.
- [x] 4.13 Android `app/android/app/build.gradle` (or `.kts`): `applicationId "tw.ccmos.app.bandao"`.
- [x] 4.14 Android Kotlin / Java package directory rename if there's an `.../argus/` path: rename to `.../bandao/`.
- [x] 4.15 `app/README.md`: title + body.

## 5. openspec/specs

- [x] 5.1 Grep `openspec/specs/**/*.md` for `argus` / `Argus`. Replace example code strings, prose references — keep them in technical context (e.g., describing identifier strings) but updated to `bandao`.
- [x] 5.2 NOTE: do NOT touch `openspec/changes/archive/**` — archived history is preserved.

## 6. Root / project docs

- [x] 6.1 `README.md`: title `argus` → `班到 (bandao)`, body references.
- [x] 6.2 `AGENTS.md`: title + product description.
- [x] 6.3 `ROADMAP.md`: any prose mention. (No argus refs found.)
- [x] 6.4 `.github/workflows/*.yml`: only the path filter / cache key reference `argus` if any. Job names / workflow names if they say "argus" — update. (No argus refs found.)

## 7. Test assertion strings

- [x] 7.1 Grep `argus` in `api/tests/`, `app/test/`, `admin-web/test/`. For UI text assertions (e.g. `expect(text).toContain('argus admin')`) update to new brand string. For Org test names (e.g. `register_admin("admin@example.com", "Acme")`) leave alone.

## 8. Build verification

- [x] 8.1 `cd api && cargo build && cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --no-fail-fast` (full).
- [x] 8.2 `cd admin-web && pnpm install --frozen-lockfile && pnpm typecheck && pnpm test && pnpm build` (full).
- [x] 8.3 `cd app && flutter pub get && dart run build_runner build && flutter analyze && flutter test` (full).

## 9. Native rebuild smoke

- [x] 9.1 `cd app/ios && pod install` — verify Podfile project resolves with new bundle id.
- [x] 9.2 `flutter build ios --simulator` (or `flutter run -d "iPhone 17"`) — verify the app launches with new label `班到` and bundle id `tw.ccmos.app.bandao`.
- [x] 9.3 Verify cold-start on simulator: previous storage is gone (because prefix changed); login flow runs fresh.
- [-] 9.4 Skipped (Android optional): Android `flutter build apk --debug`; verify launcher label.

## 10. CI verification

- [x] 10.1 Push and verify all three workflows (`api`, `admin-web`, `app`) pass on GitHub Actions.

## 11. Optional follow-ups (user decides at archive time)

- [-] 11.1 Skipped — user kept directory at /Volumes/Backup/Workspace/ccmos/argus `/Volumes/Backup/Workspace/ccmos/argus` → `bandao` — manual.
- [x] 11.2 GitHub repo rename `mosluce/argus` → `mosluce/bandao` (web UI) + local `git remote set-url origin git@github.com:mosluce/bandao.git`.
- [x] 11.3 Update `Cargo.toml` `repository =` URL after GitHub rename.

## 12. Manual smoke (pre-archive)

- [x] 12.1 Open simulator: see `班到` as app label, login flow displays `班到` branding.
- [x] 12.2 admin-web `pnpm dev` → `/login` shows `班到`, browser tab title is `班到 admin`.
- [x] 12.3 API `cargo run` → check `Server-Agent` / log lines / startup banner reads `bandao-api/0.1.0`.
- [x] 12.4 Sanity: no leftover `argus` in active code paths via final grep. (Only `.claude/settings.local.json` remains with intentional repo-rename-deferred refs per D4.)
