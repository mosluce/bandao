# flutter-toolchain Specification

## Purpose

Govern the Flutter SDK version-pinning contract for `app/`: `app/.tool-versions` (local, via asdf) and `.github/workflows/app.yml` (CI) must pin the exact same Flutter stable version, and `app/pubspec.yaml`'s `environment.flutter` lower bound must stay compatible with that pinned version, so local development and CI never drift relative to each other. See `openspec/changes/update-flutter-latest/proposal.md` and `design.md` for the context that motivated this capability (local and CI versions had drifted nearly ten minor versions apart, and `.tool-versions` had never actually pinned Flutter).

## Requirements

### Requirement: Flutter SDK version SHALL be pinned identically for local development and CI

`app/.tool-versions` SHALL declare a single, specific Flutter stable version (`<major>.<minor>.<patch>-stable`, no ranges). `.github/workflows/app.yml`'s `flutter-version` input to `subosito/flutter-action` SHALL specify the exact same version string. `app/pubspec.yaml`'s `environment.flutter` lower bound SHALL be less than or equal to that pinned version, so a fresh `flutter pub get` against the pinned SDK never fails the environment constraint.

#### Scenario: A contributor installs the pinned version via asdf

- **WHEN** a contributor runs `asdf install` inside `app/`
- **THEN** asdf installs the exact Flutter version declared in `app/.tool-versions`
- **AND** that version is a stable channel release (no `-beta`/`-dev`/`-pre` suffix)

#### Scenario: CI uses the same version as the pinned local toolchain

- **WHEN** `.github/workflows/app.yml` runs the `analyze + test` job
- **THEN** the `flutter-version` passed to `subosito/flutter-action` equals the version declared in `app/.tool-versions`

#### Scenario: pubspec environment constraint does not reject the pinned SDK

- **WHEN** `flutter pub get` runs against the Flutter version declared in `app/.tool-versions`
- **THEN** it succeeds without an SDK version constraint error from `environment.flutter`

### Requirement: Version bumps document the pin change in one commit

Any change to the Flutter version pinned in `app/.tool-versions` SHALL update `.github/workflows/app.yml`'s `flutter-version` and `README.md`'s stated Flutter version in the same change, so the three locations never drift relative to each other.

#### Scenario: Bumping the pinned version updates all three locations together

- **WHEN** `app/.tool-versions`'s Flutter version is changed
- **THEN** `.github/workflows/app.yml`'s `flutter-version` is changed to the same value in the same change
- **AND** `README.md`'s stated Flutter version text is updated to match
