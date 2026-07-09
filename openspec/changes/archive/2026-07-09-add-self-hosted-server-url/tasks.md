## 1. Override layer: open release + validation

- [x] 1.1 Remove the three `kReleaseMode` early-returns so release also reads/writes the override — replaced `dev_overrides.dart`/`DevOverrides`/`devOverridesProvider` with `server_url_override.dart`/`ServerUrlOverride`/`serverUrlOverrideProvider` (thin wrapper, no gating); renamed the storage key value `dev.api_base_url_override` → `server.api_base_url` (method names were already neutral, so the two fake-storage impls needed no change); `background_sync.dart` reads via the same `readApiBaseUrlOverride()` and now follows the override in release automatically
- [x] 1.2 Added `validateBaseUrlOverride(url, {bool? release})` in `api_base_url.dart` returning `BaseUrlOverrideError?` (`malformed` / `insecureScheme`): release requires `scheme=='https'` + host; debug accepts any scheme + host. `release` param defaults to `kReleaseMode` so tests can force the release path
- [x] 1.3 Updated `ApiBaseUrlResolver` doc comments (no longer debug-only); precedence (override → compile-time default) unchanged

## 2. Server config screen (promote to production)

- [x] 2.1 Renamed `dev_server_config_screen.dart`/`DevServerConfigScreen` → `server_config_screen.dart`/`ServerConfigScreen`; removed the `kReleaseMode` early-return in `_seed()`; router route `devServerConfig` (`/dev-server-config`) → `serverConfig` (`/server-config`) and dropped the release "Not available" gate
- [x] 2.2 `_save()` now routes through `validateBaseUrlOverride`: `insecureScheme` → shows `serverConfigHttpsRequired` ("需 https"), `malformed` → generic error; debug stays loose. Also clears the bearer token + invalidates `authProvider` when the saved URL differs from the current effective URL (server change ⇒ re-login); `_clear()` mirrors this when leaving a custom server
- [x] 2.3 Crashlytics self-test button still `kDebugMode`-gated; privacy-URL override section unchanged (still loosely validated, dev-facing)
- [x] 2.4 Screen title now uses `serverConfigTitle` ("伺服器設定"); reset button uses `serverConfigResetDefault`; URL field has a helper string

## 3. Login screen entry + current-server display + session reset

- [x] 3.1 Removed the debug-only logo 5-tap easter egg (`_BandaoLogo` is now a plain `StatelessWidget`); added an always-visible `_ServerConnectionInfo` with a "伺服器設定" `TextButton` (key `login.server_config`) that `context.push`es `/server-config`
- [x] 3.2 `_ServerConnectionInfo` shows "官方預設" when the effective URL equals `Env.compileTimeDefault()`, else "自訂 <host>" (host only, via `serverConnectionCustom`)
- [x] 3.3 Session reset on server change implemented in the config screen's `_save()`/`_clear()` (see 2.2): bearer token cleared, `last_org_code` retained

## 4. Localization

- [x] 4.1 Added `serverConfigTitle`, `serverConfigEntry`, `serverConfigHelper`, `serverConfigResetDefault`, `serverConfigHttpsRequired`, `serverConnectionOfficial`, `serverConnectionCustom(host)` to the hand-rolled `app_localizations.dart` shim (zh + en) and the three ARB source files (zh_TW / zh / en)

## 5. Tests, docs & verification

- [x] 5.1 `test/core/storage/api_base_url_test.dart`: validator — release rejects `http`/`http://localhost`/no-scheme/path-only and accepts `https://host`; debug accepts `http://localhost:9090` + LAN IP + https, still rejects malformed; plus resolver precedence (default vs override)
- [x] 5.2 `test/features/auth/presentation/server_config_screen_test.dart` (GoRouter harness): screen renders reachably; saving a valid https URL persists the override AND clears the bearer token; a malformed URL is not persisted
- [x] 5.3 `test/features/auth/presentation/login_server_info_test.dart`: login shows "官方預設" with no override and "自訂 api.myco.com" with an override
- [x] 5.4 Updated `app-shell` spec delta: folded release override + split validation into the base-URL requirement, REMOVED "release excludes override path", ADDED server-config-screen + login entry/current-server/session-reset requirements. `openspec validate --strict` ✓
- [x] 5.5 Docs: `app/README.md` Run section + new "Self-hosted server" section (https-only, deploy behind TLS, no CORS/backend change) + updated the privacy-override note to reference the server-config screen. `flutter analyze` clean; `flutter test` 176/176 pass. **Manual release-build smoke DONE**: TestFlight build `0.4.0 (1)` (release, `--dart-define=API_BASE_URL=https://bandao-api.ccmos.tw`) pointed at a self-hosted api via a Cloudflare quick tunnel (`https://<slug>.trycloudflare.com` → local `api/` on :9090 → docker Mongo). Verified the release-only path: 伺服器設定 entry visible in release, `http://` override rejected ("需 https"), `https://` accepted + login-screen shows custom host, reset-to-default works; then real login succeeded end-to-end (org `V2B4F3KHZ4` / `mosluce`, HTTP 200). Upload note: `flutter build ipa` export failed once on Xcode 26 ("No such file or directory") — worked via `xcodebuild -exportArchive`; and marketing version had to bump 0.3.1→0.4.0 because the 0.3.1 pre-release train was closed.
