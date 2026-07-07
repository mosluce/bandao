# mobile-release Specification

## Purpose
TBD - created by archiving change add-app-personal-trajectory. Update Purpose after archive.
## Requirements
### Requirement: Store metadata SHALL lead with the AppUser-facing personal log, not the org-side record

The repository's `app/store_metadata/ios/description.txt` and `app/store_metadata/android/short_description.txt` (or equivalent) and `app/store_metadata/ios/promotional_text.txt` SHALL position the in-app personal trajectory feature ("我的工作日記" / "My Work Day") as the primary user benefit. The personal-log feature SHALL appear in the first bullet (or first sentence) of the feature list. Any framing that emphasizes the employer's access to the data ("for managers to track employees") SHALL appear, if at all, after the personal-benefit framing.

This requirement exists to satisfy App Store Review Guideline 2.5.4, which treats persistent background location whose primary beneficiary is the employer rather than the user as non-compliant.

#### Scenario: description.txt feature list leads with personal log

- **WHEN** the contents of `app/store_metadata/ios/description.txt` are read
- **THEN** the personal-log feature ("我的工作日記" or equivalent zh-TW phrasing) appears in the first bullet of the feature list
- **AND** the bullet describes the user (employee) as the consumer of the data

#### Scenario: promotional_text.txt mentions the personal log

- **WHEN** the contents of `app/store_metadata/ios/promotional_text.txt` are read
- **THEN** the file references the personal-log feature by name

### Requirement: App Store screenshot set SHALL surface the personal trajectory feature within the first three positions

The `app/store_metadata/ios/screenshots/` directory SHALL contain at least one screenshot of the `/trajectory` "我的工作日記" screen, and that screenshot SHALL be placed in one of the first three positions of the iPhone screenshot set (positions 1, 2, or 3). This ensures App Store browsers and App Review reviewers see the personal-log surface without scrolling.

#### Scenario: trajectory screenshot present in first three slots

- **WHEN** the iPhone screenshot set in `app/store_metadata/ios/screenshots/` is inspected
- **THEN** at least one screenshot depicts the `/trajectory` map view
- **AND** that screenshot's filename sort position places it in slots 1, 2, or 3

### Requirement: NSLocationWhenInUseUsageDescription SHALL lead with the AppUser-facing personal log

The iOS `NSLocationWhenInUseUsageDescription` in `app/ios/Runner/Info.plist` and any Android-side equivalent rationale text SHALL lead with the personal-log framing — explaining that the user themselves will be able to review their own work-day movement inside the app — before mentioning the iOS blue indicator, how to stop tracking, or any reference to org-side records.

#### Scenario: iOS permission rationale leads with personal-log framing

- **WHEN** the `NSLocationWhenInUseUsageDescription` string in `app/ios/Runner/Info.plist` is read
- **THEN** the first sentence references the in-app personal log ("我的工作日記" or equivalent) as the primary use of the data
- **AND** the iOS blue indicator and the "press 下班 to stop" instruction appear in subsequent sentences

### Requirement: App Review reply trail SHALL be recorded in-repo under store_metadata

Each App Review rejection that we respond to with anything beyond a one-line acknowledgement SHALL have its reply preserved under `app/store_metadata/ios/app_review_replies/` (or `android/` equivalent) as a Markdown file named for the cited guideline and date (e.g. `2.5.4-2026-05-15.md`). The file SHALL include the cited guideline number, the App Store Connect submission id, the date, and the reply body verbatim.

This requirement exists so a future maintainer (or a future AI agent picking up the resubmission) can reconstruct what we told App Review without access to the App Store Connect message thread.

#### Scenario: 2.5.4 reply file exists for the 2026-05-15 rejection

- **WHEN** the directory `app/store_metadata/ios/app_review_replies/` is listed after this change ships
- **THEN** a file named `2.5.4-2026-05-15.md` exists
- **AND** the file contains the cited guideline (`2.5.4`), the submission id, and the verbatim reply body sent to App Review

### Requirement: app SHALL ship with a non-debug release keystore enrolled in Play App Signing

The Android variant of the app SHALL be signed with a production upload keystore loaded from a `key.properties` file outside of version control. The repository SHALL NOT contain the keystore file (`*.jks`), its passwords, or `key.properties` itself. The corresponding Google Play Console app entry SHALL be enrolled in Play App Signing so that the upload keystore can be reset by the operator without abandoning the `applicationId tw.ccmos.app.bandao`.

#### Scenario: release build refuses to use debug signing

- **WHEN** an operator runs `flutter build appbundle --release` against this repo with `android/key.properties` populated
- **THEN** the produced `.aab` SHALL be signed with the upload keystore referenced by `key.properties`
- **AND** SHALL NOT be signed with the Android debug keystore

#### Scenario: keystore secrets stay out of version control

- **WHEN** the repository's tracked files are scanned
- **THEN** no `*.jks` file, no `key.properties` file, and no keystore password text SHALL be present in any tracked file

#### Scenario: lost upload keystore does not orphan the applicationId

- **WHEN** the operator loses the upload keystore (machine failure, password manager loss)
- **THEN** the operator SHALL be able to request a new upload key through Google Play Console
- **AND** the published app under `applicationId tw.ccmos.app.bandao` SHALL continue to receive the operator's future uploads after the new key is registered

### Requirement: app version SHALL be canonically defined in pubspec.yaml

`app/pubspec.yaml`'s `version: <name>+<number>` field SHALL be the single source of truth for the marketing and build numbers shipped to both stores. Every native and tooling location that exposes a version SHALL resolve from that field.

#### Scenario: Android build pulls version from pubspec

- **WHEN** `flutter build appbundle --release` runs
- **THEN** the `.aab`'s `versionName` SHALL equal the name part of `pubspec.yaml#version`
- **AND** its `versionCode` SHALL equal the build-number part of `pubspec.yaml#version`

#### Scenario: iOS archive pulls version from pubspec

- **WHEN** `flutter build ipa --release` runs
- **THEN** the produced `.ipa`'s `CFBundleShortVersionString` (Info.plist) SHALL equal the name part of `pubspec.yaml#version`
- **AND** its `CFBundleVersion` SHALL equal the build-number part

#### Scenario: Xcode UI matches the binary version

- **WHEN** the operator opens `ios/Runner.xcworkspace` in Xcode and inspects the General tab for the Runner target
- **THEN** the displayed Version and Build values SHALL match `pubspec.yaml#version` (no hardcoded `1.0` / `1` fallbacks)

### Requirement: app SHALL request only When-In-Use location, with continuous shift tracking via OS-visible foreground mechanisms

The app SHALL NOT declare `NSLocationAlwaysAndWhenInUseUsageDescription` on iOS or `ACCESS_BACKGROUND_LOCATION` on Android. Continuous location tracking during a clock-in shift SHALL be sustained by:

- **iOS**: an active `CLLocationManager` session under `UIBackgroundModes = ["location"]`, which causes iOS to display its blue status bar while the app is backgrounded.
- **Android**: a foreground service of type `location` declared via `FOREGROUND_SERVICE` + `FOREGROUND_SERVICE_LOCATION`, accompanied by the existing "工作期間定位追蹤中" sticky notification.

The location usage description on both platforms SHALL explain when tracking starts (after pressing 上班), how it is visually indicated (iOS blue bar / Android sticky notification), and how the user stops it (pressing 下班).

#### Scenario: iOS prompts only "While Using the App"

- **WHEN** a freshly installed iOS app first requests location
- **THEN** the prompt SHALL offer "Allow While Using App" / "Allow Once" / "Don't Allow"
- **AND** SHALL NOT offer "Always Allow" before the user opts into 上班

#### Scenario: Android does not request ACCESS_BACKGROUND_LOCATION

- **WHEN** the Android manifest is inspected
- **THEN** `ACCESS_BACKGROUND_LOCATION` SHALL NOT be declared
- **AND** `FOREGROUND_SERVICE` and `FOREGROUND_SERVICE_LOCATION` SHALL be declared

#### Scenario: iOS shows blue indicator during a shift

- **WHEN** the user taps 上班 in the foreground and then sends the app to background
- **THEN** iOS SHALL display its blue status bar / pill indicator showing the app is using location
- **AND** the bar SHALL disappear once the user taps 下班 and the location session ends

#### Scenario: Android shows sticky notification during a shift

- **WHEN** the user taps 上班 on Android
- **THEN** the system tray SHALL display a non-dismissible "工作期間定位追蹤中" notification while the shift is active
- **AND** the notification SHALL clear once 下班 ends the shift

### Requirement: uncaught errors SHALL be reported to Firebase Crashlytics without user-identity linkage

The app SHALL register Firebase Crashlytics handlers for `FlutterError.onError` and `PlatformDispatcher.instance.onError`, so that uncaught Flutter and platform errors flow to the Crashlytics console with symbolicated stack traces. The app SHALL NOT call `FirebaseCrashlytics.instance.setUserIdentifier(...)` or any equivalent that ties a crash report to a Bandao identity (email, AppUser id, dashboard-user id, or Org id).

#### Scenario: Flutter framework error reaches Crashlytics

- **WHEN** an uncaught Flutter framework error occurs in a release build with Crashlytics initialized
- **THEN** the error SHALL be uploaded to the Firebase project's Crashlytics dashboard
- **AND** the dashboard entry SHALL include a symbolicated stack trace (dSYM or Mapping File previously uploaded)

#### Scenario: platform-side error reaches Crashlytics

- **WHEN** a platform-thrown error reaches `PlatformDispatcher.instance.onError`
- **THEN** the error SHALL be uploaded to Crashlytics

#### Scenario: crash report carries no Bandao identity

- **WHEN** any Crashlytics report is inspected
- **THEN** it SHALL NOT contain a Bandao user email, AppUser id, dashboard-user id, or Org id
- **AND** the Crashlytics "user id" field SHALL be empty

### Requirement: support contact in store metadata SHALL be a domain alias under ccmos.tw

The repository's `app/store_metadata/` SHALL list a Bandao-domain support contact (currently `support@ccmos.tw`) and SHALL NOT contain a personal mailbox. The operator SHALL maintain the alias as a forward to whoever currently fields support requests.

#### Scenario: store_metadata files contain only the domain alias

- **WHEN** the contents of `app/store_metadata/ios/support_url.txt` and `app/store_metadata/android/contact_email.txt` are read
- **THEN** they SHALL reference `support@ccmos.tw` (e.g. `mailto:support@ccmos.tw`)
- **AND** they SHALL NOT reference any personal mailbox

#### Scenario: published store listings show the domain alias

- **WHEN** any user views the App Store or Google Play product page for Bandao
- **THEN** the listed Support / Developer Contact email SHALL be `support@ccmos.tw`

### Requirement: privacy policy URL SHALL be HTTPS and cover all declared store data items

The privacy policy URL referenced from `app/store_metadata/` and registered with both stores SHALL serve over HTTPS with a valid public-CA certificate, and SHALL describe handling for every data category declared in App Privacy / Data Safety: email, location, device id, and crash diagnostics. Until the marketing site lands, this URL SHALL point at the production admin-web `/privacy` page.

#### Scenario: privacy URL resolves over HTTPS

- **WHEN** an unauthenticated client requests the privacy URL configured in store metadata
- **THEN** the response SHALL be `200 OK` over a valid TLS connection
- **AND** the certificate SHALL chain to a public CA

#### Scenario: privacy policy enumerates declared data items

- **WHEN** an operator audits the privacy page content against the App Privacy / Data Safety declarations
- **THEN** the page SHALL describe collection and use for each of: email, location, device id, crash diagnostics

### Requirement: app SHALL be available on both App Store and Google Play public storefronts under the name 班到 / Bandao

The app SHALL be published as a free download on Apple App Store and Google Play, in both Taiwan and the global storefronts. The displayed app name SHALL be 班到 with subtitle / promotional text "Bandao". `CFBundleDisplayName` (iOS) and `app_name` (Android `strings.xml`) SHALL also be 班到 so the on-device springboard label matches the store listing.

#### Scenario: store search reveals both names

- **WHEN** a user searches App Store or Google Play for "班到"
- **THEN** the Bandao app product page SHALL appear in the results

#### Scenario: store search by Bandao also resolves

- **WHEN** a user searches App Store or Google Play for "Bandao"
- **THEN** the Bandao app product page SHALL appear in the results (via subtitle / promo text)

#### Scenario: on-device label is 班到

- **WHEN** the app is installed on iOS or Android
- **THEN** the home-screen / launcher label under the icon SHALL read 班到

### Requirement: iOS app SHALL support both iPhone and iPad without iPad-specific UI work

`TARGETED_DEVICE_FAMILY` SHALL be `"1,2"` (iPhone + iPad). The app's existing layouts SHALL render without breakage on the largest supported iPad simulator; iPad-specific layouts (split view, multi-column, Apple Pencil affordances) are NOT required for this change.

#### Scenario: iPad simulator runs the full clock-in flow

- **WHEN** the app is installed on the largest iPad simulator (e.g. iPad Pro 12.9" 6th gen)
- **AND** the operator runs through register / login / clock-in / clock-out
- **THEN** all screens SHALL render without overflow, off-screen content, or layout crashes

#### Scenario: store listing shows iPad screenshots

- **WHEN** the App Store product page for Bandao is viewed on an iPad
- **THEN** at least 2 iPad-class screenshots SHALL be displayed under the iPad device tab
