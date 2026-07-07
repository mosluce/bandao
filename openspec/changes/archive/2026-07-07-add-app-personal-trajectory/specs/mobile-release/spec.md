## ADDED Requirements

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
