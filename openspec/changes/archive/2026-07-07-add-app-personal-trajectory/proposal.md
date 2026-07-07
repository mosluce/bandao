## Why

iOS App Review (submission `2f88a54d-2b9a-4069-b5fa-88e2ed770187`, 2026-05-15)
rejected build 0.3.0 (7) under Guideline 2.5.4 because the prior build's
persistent background location served only employer-facing trajectory export
on the admin dashboard — there was no in-app feature that surfaced the data
back to the AppUser themselves. Apple's policy treats "employee tracking" as
an insufficient justification for the `UIBackgroundModes: location` entitlement.

To clear 2.5.4 without dropping background tracking quality (which would
hollow out the existing location-tracking feature for the orgs that already
opted in), we add a user-owned "我的工作日記 / My Work Day" surface that lets
each AppUser see their own movement on a map and review their own daily
stats. The AppUser becomes the primary beneficiary of the persistent location
data; the admin trajectory dashboard remains as a secondary, org-consented
surface.

## What Changes

- API: new `GET /app/checkin/me/locations` endpoint — AppUser bearer auth,
  returns the caller's own pings for a date range, same shape and validation
  rules as the existing admin `GET /checkin/users/:id/locations`.
- App: new "我的工作日記" tab in the main bottom navigation (`go_router`
  shell route) showing today + previous 7 days of the AppUser's own
  trajectory on a `flutter_map` polyline with summary stats (distance,
  on-shift duration, ping count).
- App: dynamic summary card on home screen showing today's distance + shift
  duration; tap-through opens the trajectory tab for today.
- App: consent dialog and `NSLocationWhenInUseUsageDescription` reworded to
  lead with the AppUser's personal benefit ("you can review your own work
  day movement at any time") before any mention of org-side records.
- App: zh-TW strings for the new screens, tab label, empty states.
- Store metadata (`store_metadata/ios/description.txt`,
  `promotional_text.txt`): feature-list and lead description reframed so the
  personal log is the primary user benefit, ahead of org-side records.
- Store metadata: new App Store screenshot for "我的工作日記" map view,
  placed in the first three positions so it is visible without scrolling on
  the App Store listing.
- App Privacy form (App Store Connect, not a repo artifact): add
  "App Functionality" use case for Precise Location alongside the existing
  org-side use case. Recorded as a release-runbook checklist item.
- App Review reply letter drafted and stored under
  `store_metadata/ios/app_review_replies/2.5.4-2026-05-15.md` so the
  evidence trail lives in-repo.

## Capabilities

### New Capabilities

- `app-personal-trajectory`: AppUser-facing surface for viewing one's own
  shift trajectory in the app — bottom-nav tab, today-default + 7-day
  picker, `flutter_map` polyline + summary stats, home-screen summary card,
  consent-dialog and permission-description reword.

### Modified Capabilities

- `location-tracking`: add the new `GET /app/checkin/me/locations` endpoint
  alongside the existing admin endpoint, with token-derived `app_user_id`
  scoping and the same range-validation rules.
- `mobile-release`: store metadata description / promotional text reframing
  + App Privacy form checklist + App Review reply trail.

## Impact

- **API (Rust)**: new `app/checkin/me/locations` route handler; reuses the
  existing `list_pings` data access and validation; new integration tests
  mirroring the admin endpoint suite.
- **App (Flutter)**: adds `flutter_map` + `latlong2` to `pubspec.yaml`; new
  `lib/features/trajectory/` feature module (state, data, presentation);
  router shell extended with the new tab; home screen gets a summary card
  widget; consent dialog text rewritten; new widget + integration tests.
- **Store metadata**: `description.txt`, `promotional_text.txt`,
  `screenshots/ios/` updated; new `app_review_replies/` directory created.
- **iOS Info.plist**: `NSLocationWhenInUseUsageDescription` reworded;
  `UIBackgroundModes: location` stays.
- **App Store Connect** (out-of-repo): App Privacy form updated; build
  0.3.1 (8) submitted with the reply letter attached.
- **No DB schema change**; reuses existing `location_pings` collection and
  90-day TTL.
- **No breaking changes** to existing `POST /app/checkin/locations` or any
  admin route.
