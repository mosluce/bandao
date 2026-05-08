## 0. Operator pre-flight (off-repo)

- [x] 0.1 Generate Android upload keystore via `keytool -genkey -v -keystore ~/.bandao/keystores/bandao-upload.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload`; record store password / key password / alias.
- [x] 0.2 Store the keystore file + all passwords in the operator's password manager (any of 1Password, Bitwarden Premium, self-hosted Vaultwarden, or equivalent that supports binary attachments) as a single item titled "Bandao Android Upload Keystore" (attach .jks + password / alias fields). Verify retrieval works on a second device.
- [x] 0.3 Configure the `ccmos.tw` mail provider with alias `support@ccmos.tw → mosluce@no8.io`; send a test email and confirm it arrives.
- [x] 0.4 Open Firebase Console; create the `Bandao` project; add iOS app (`tw.ccmos.app.bandao`) and Android app (`tw.ccmos.app.bandao`); enable Crashlytics for both; download `GoogleService-Info.plist` and `google-services.json` and stage them locally for §1 / §2 placement.
- [x] 0.5 App Store Connect: register Bundle ID `tw.ccmos.app.bandao`; create app record with primary language `zh-Hant`, name `班到`, subtitle `Bandao`, category Productivity. Confirm the developer team `SGP5JZGDM3` matches the value already in `ios/Runner.xcodeproj/project.pbxproj`. (Real team ID turned out to be `P92HHTP8SE`; project.pbxproj corrected on this branch.)
- [x] 0.6 Google Play Console: create the app with default language `zh-Hant` and app name `班到`; enroll in Play App Signing; create the Internal Testing track. Confirm `applicationId` matches `tw.ccmos.app.bandao`.

## 1. Android signing infrastructure (in-repo)

- [x] 1.1 Update `app/.gitignore` (or repo root `.gitignore`) so `android/key.properties`, `**/*.jks`, and `**/*.keystore` are excluded from version control.
- [x] 1.2 Refactor `app/android/app/build.gradle.kts` to (a) load `android/key.properties` if present, (b) declare a `signingConfigs.release` block reading `keyAlias / keyPassword / storeFile / storePassword` from those properties, and (c) wire `buildTypes.release.signingConfig` to use `release` when properties exist or fall back to `debug` otherwise (so `flutter run --release` still works locally without keystore configured).
- [x] 1.3 Drop the `google-services.json` from §0.4 into `app/android/app/google-services.json`.
- [x] 1.4 Author a local `app/android/key.properties` (gitignored) pointing `storeFile` at `~/.bandao/keystores/bandao-upload.jks` and the three passwords from §0.1. (File lives at `app/android/key.properties` on operator's machine; verified by §1.5 release build succeeding with the upload key alias.)
- [x] 1.5 Smoke `cd app && flutter build appbundle --release`; verify the produced `.aab` is signed with the upload key (e.g. `unzip -p build/app/outputs/bundle/release/app-release.aab META-INF/MANIFEST.MF` and confirm the upload key alias rather than `androiddebugkey`). (Required a workmanager pubspec bump from `^0.5.2` to `>=0.6.0 <0.8.0` — 0.5.2 still references Flutter v1 embedding APIs (`shim`, `ShimPluginRegistry`, `Registrar`) that were removed in Flutter 3.27+; resolved to 0.7.0 since 0.8+ requires Flutter SDK 3.32+.)

## 2. iOS version sync, Firebase plist, iPad confirmation

- [x] 2.1 Edit `app/ios/Runner.xcodeproj/project.pbxproj`: change every `MARKETING_VERSION = 1.0;` to `MARKETING_VERSION = "$(FLUTTER_BUILD_NAME)";` and every `CURRENT_PROJECT_VERSION = 1;` to `CURRENT_PROJECT_VERSION = "$(FLUTTER_BUILD_NUMBER)";` (expect 6 line changes total — 3 of each).
- [x] 2.2 Drop the `GoogleService-Info.plist` from §0.4 into `app/ios/Runner/GoogleService-Info.plist`; ensure it is added to the Runner target's Copy Bundle Resources phase via the Xcode project (verify by re-opening the workspace). (Operator added the file to the Runner target via Xcode UI; project.pbxproj diff includes PBXBuildFile + PBXFileReference + group entry + Resources phase entry.)
- [x] 2.3 Confirm `TARGETED_DEVICE_FAMILY = "1,2"` is set across all relevant build configurations in `project.pbxproj` (no change expected; this is a guard).
- [ ] 2.4 Smoke `cd app && flutter build ipa --release` (operator must have valid signing in keychain); inspect `build/ios/ipa/*.ipa` `Info.plist` for `CFBundleShortVersionString = 0.3.0` and `CFBundleVersion = 3` matching `pubspec.yaml`.

## 3. Permissions and usage descriptions

- [x] 3.1 Edit `app/ios/Runner/Info.plist`: rewrite `NSLocationWhenInUseUsageDescription` to explicitly state when tracking starts (after pressing 上班), how it shows in background (iOS 螢幕上方藍色提示), and how to stop it (按下班). Keep wording under 175 characters total to fit the prompt dialog comfortably.
- [x] 3.2 Audit `app/android/app/src/main/AndroidManifest.xml`: confirm `<uses-permission android:name="android.permission.FOREGROUND_SERVICE"/>` and `<uses-permission android:name="android.permission.FOREGROUND_SERVICE_LOCATION"/>` are declared; confirm `ACCESS_BACKGROUND_LOCATION` is NOT declared. (Audit result: both FOREGROUND_SERVICE and FOREGROUND_SERVICE_LOCATION already declared; ACCESS_BACKGROUND_LOCATION absent. No edits needed.)
- [x] 3.3 Cross-check the Foreground Service declaration in the manifest specifies `android:foregroundServiceType="location"` on the relevant `<service>` element; if missing, add it. (Audit result: app's own Manifest does not declare a `<service>` — the foreground service is contributed by the `geolocator` Android plugin's manifest at merge time, which already sets `foregroundServiceType="location"`. No edits needed in app's Manifest.)
- [x] 3.4 Review `admin-web/pages/privacy.vue` content; confirm it covers the four data categories declared in store privacy: email, location, device id, crash diagnostics. If a category is missing, update the page in the same change (small edit, do not split into another change). (Updated: added 「裝置識別資料」 and 「當機與診斷資料」 bullets to section 2; updated PLATFORM_CONTACT_EMAIL from placeholder to support@ccmos.tw; bumped LAST_UPDATED_AT to 2026-05-07; removed the placeholder note.)

## 4. Crashlytics integration

- [x] 4.1 Add `firebase_core` and `firebase_crashlytics` to `app/pubspec.yaml#dependencies`; run `cd app && flutter pub get`.
- [x] 4.2 iOS: edit `app/ios/Podfile` (or generate via `cd app/ios && pod install` after 4.1) to ensure Firebase pods are linked; add a Run Script Phase to the Runner target that uploads dSYMs to Crashlytics on archive (`${PODS_ROOT}/FirebaseCrashlytics/upload-symbols`). (Operator ran `pod install` — Podfile.lock updated; added the Run Script Phase as the last phase on the Runner target via Xcode UI.)
- [x] 4.3 Android: add the `com.google.gms.google-services` plugin in `app/android/build.gradle.kts` (project-level `plugins {}`) and apply it together with `com.google.firebase.crashlytics` in `app/android/app/build.gradle.kts` (module-level `plugins {}`). (Declared in `settings.gradle.kts` `pluginManagement.plugins {}` with `apply false` since this project uses Settings-level plugin management; applied in `app/build.gradle.kts`.)
- [x] 4.4 Edit `app/lib/main.dart`: in `main()` initialize `Firebase.initializeApp()`, hook `FlutterError.onError = FirebaseCrashlytics.instance.recordFlutterFatalError`, and `PlatformDispatcher.instance.onError` → `FirebaseCrashlytics.instance.recordError(error, stack, fatal: true); return true;`. Do NOT call `setUserIdentifier`.
- [x] 4.5 Add a debug-only "Force Crash" entry point (e.g. a button in a developer / about screen guarded by `if (kDebugMode)`); in release builds this UI SHALL NOT exist. (Added to `dev_server_config_screen.dart` — the existing dev menu reachable via "tap logo 5x on /login".)
- [x] 4.6 Local smoke: in a debug build, trigger the force-crash button; observe the crash event appear in Firebase Console's Crashlytics dashboard within 5 minutes; verify the stack trace is symbolicated.

## 5. Store metadata structure and content

- [x] 5.1 Create `app/store_metadata/ios/` with files: `description.txt`, `promotional_text.txt` (≤170 chars), `keywords.txt` (≤100 chars), `support_url.txt` containing `mailto:support@ccmos.tw`, `privacy_url.txt` containing `https://bandao-admin.ccmos.tw/privacy`, `marketing_url.txt` empty, `release_notes/0.3.0.txt` (or whatever version §9 ships).
- [ ] 5.2 Create `app/store_metadata/ios/screenshots/iphone_6.7/` (≥4 images), `iphone_6.5/` (≥4), `ipad_12.9/` (≥2). Optionally `iphone_5.5/` and `ipad_11.0/` if the operator wants broader coverage.
- [x] 5.3 Create `app/store_metadata/android/` with files: `short_description.txt` (≤80 chars), `full_description.txt` (≤4000 chars), `contact_email.txt` containing `support@ccmos.tw`, `website.txt` (empty for now — marketing URL deferred), `privacy_policy_url.txt` containing `https://bandao-admin.ccmos.tw/privacy`, `changelog/3.txt` (matching versionCode).
- [x] 5.4 Create `app/store_metadata/android/images/` with `icon_512.png` (512×512), `feature_graphic.png` (1024×500), `phone-screenshots/` (≥2 1080×1920+), `tablet-screenshots/` (≥2). (Directories scaffolded with `.gitkeep`; actual binary assets are operator-produced — see §5.2 / §5.4 in the deferred operator-side work.)
- [x] 5.5 Write the description copy: hero one-liner emphasising 「為小型團隊打造的多組織打卡 app」; bullets for register/join Org / 上下班 / 軌跡 (org toggle) / 多裝置 / 隱私 footprint. Keep tone matter-of-fact; avoid feature claims that aren't yet shipped.
- [ ] 5.6 Stage the iOS + Android release notes for the version going to first review; reference CHANGELOG entries authored in §9.

## 6. Console preparation (operator-only)

- [ ] 6.1 In App Store Connect, fill the App Information page: name 班到, subtitle Bandao, primary language zh-Hant, primary category Productivity. Upload the icon (1024×1024 png, no alpha).
- [ ] 6.2 Fill the App Privacy nutrition labels: Email + Location + Device ID linked to identity (app functionality); Crash Data + Performance Data not linked (app functionality); no third-party sharing; no tracking.
- [ ] 6.3 Upload the iOS metadata files from §5.1–§5.2 via App Store Connect UI (or fastlane `deliver init` once phase 2 starts; for now, manual upload).
- [ ] 6.4 In Google Play Console, fill the store listing: app name 班到, short description, full description, screenshots, feature graphic, app icon. Set category to Productivity. Mark the app as free with no IAP.
- [ ] 6.5 Fill the Data Safety form: same data items as iOS + answer "Is location collected in the background?" → Yes (via foreground service); attach a screenshot of the sticky notification as supporting evidence; confirm none of the data is shared with third parties.
- [ ] 6.6 Confirm Play App Signing is enabled (it should already be, from §0.6); verify Internal Testing track is created and an internal tester list exists.

## 7. Smoke (release builds + Crashlytics + store-track installs)

- [x] 7.1 Trigger a Crashlytics test crash from a debug build (per §4.6); confirm the crash appears in Firebase Console with symbolicated stack within 5 minutes. (Same act as §4.6, already verified — crash event surfaced on Firebase Console.)
- [ ] 7.2 Android live smoke for location tracking (folds in ROADMAP `[app] Android live smoke for location tracking`): on a real Android device installed via Play Internal Testing, run the full sequence — register / login as AppUser / 上班 / send app to background / observe `工作期間定位追蹤中` sticky notification persists / clock_in 事件出現在 admin-web `/checkin` / 下班 / sticky notification 消失 / verify the toggle for org-level location tracking turns the foreground service on/off as expected.
- [ ] 7.3 iOS smoke via TestFlight: install the build on at least one iPhone and one iPad (largest available simulator counts if no physical iPad); run register / login / 上班 → background → confirm iOS blue indicator appears → resume → 下班 → indicator clears.
- [ ] 7.4 Verify the on-store version (TestFlight build number / Play Internal Testing release name) matches `pubspec.yaml#version` exactly.
- [ ] 7.5 Confirm the in-store displayed app name on both TestFlight (or App Store sandbox) and Play Console preview reads 班到 with Bandao subtitle / promotional text where applicable.

## 8. Submit for review (operator-only)

- [ ] 8.1 Apple: submit the TestFlight build for App Store review via App Store Connect; in the submit notes, briefly explain the work-shift location tracking model (manual 上班/下班, blue indicator visible while backgrounded, no Always permission requested) and link to the privacy URL.
- [ ] 8.2 Google: promote the Internal Testing build to Closed Testing or Production track; complete the background-location-via-foreground-service justification form (Play Console will surface this); attach the sticky-notification screenshot if requested.

## 9. Documentation

- [x] 9.1 Add `CHANGELOG.md` at the repo root in Keep a Changelog style; seed it with an entry for the version being shipped (e.g. `0.3.0+3`) summarising what's in this first store release.
- [x] 9.2 Append an "App cut release" section to `DEPLOY.md` covering: pre-reqs (Apple / Play / Firebase / support alias / keystore restored), Android cut steps (`flutter build appbundle --release` → upload to Play Console), iOS cut steps (`flutter build ipa --release` → upload via Xcode Organizer or Transporter to App Store Connect), store-side review tips (privacy nutrition / Data Safety / location justification), and rollback (don't promote the bad build; cut a hotfix patch).
- [x] 9.3 Update `app/README.md` (or repo root `README.md` — whichever is canonical for app contributors) to add a pointer to the DEPLOY.md app section.
- [x] 9.4 Update `ROADMAP.md`: remove the `[app] Android live smoke for location tracking` item (covered by §7.2 above). Do NOT remove the `[cross] Marketing landing site at bandao.ccmos.tw` item.
- [x] 9.5 Run `openspec validate app-release-prep` and confirm it returns valid.

## 10. Hand-off

- [x] 10.1 Open a PR titled `chore(app): app-release-prep` covering all in-repo changes (§1–§5 + §9). Operator-only sections (§0 / §6 / §7 / §8) get crossed off in tasks.md as the operator completes them; PR body should explicitly note these. (In-repo work shipped as four PRs instead of one — #9 (step 3 no-deps), #10 (step 4 Firebase + Crashlytics), #11→#12 (DEVELOPMENT_TEAM correction + §0.5/§0.6 ticks). Spirit met: every in-repo change is on main; operator-only ticks tracked in this same tasks.md.)
- [ ] 10.2 After the PR merges and the operator has completed §0 / §6, do at least one full §7 smoke; only then proceed to §8.
- [ ] 10.3 Once both stores have the app live, run `/opsx:archive app-release-prep` to archive this change and sync the `mobile-release` capability to `openspec/specs/`.
