## Context

The existing `location-tracking` capability (archived 2026-05-05) wired up:

- App: `GeolocatorTrackingService` collects pings during shift, enqueues to
  drift, flushed by `LocationPingProcessor` via `POST /app/checkin/locations`.
- API: per-Org toggle, `location_pings` collection with 90-day TTL, admin
  read (`GET /checkin/users/:id/locations`) + xlsx export.
- admin-web: trajectory map page at `/checkin/:id/trajectory`.

The AppUser side of this pipeline is **write-only** — there is no in-app
surface for an AppUser to review their own pings. Every UX path that
*reads* pings is dashboard-side. This is the structural reason App Review
flagged 2.5.4: from Apple's perspective the persistent location data
benefits only the employer.

This change closes that loop by adding an AppUser-facing read path and the
UI to render it. The collection pipeline, the per-Org toggle, the 90-day
TTL, and the admin trajectory dashboard are all unchanged.

Related change history: see `add-location-tracking-app`,
`add-location-tracking-server`, `add-location-tracking-dashboard`
(all archived) for the existing pipeline.

## Goals / Non-Goals

**Goals:**

- AppUsers can open their own daily trajectory on a map inside the app for
  today and any of the previous 7 days, without going through an admin.
- The "我的工作日記" surface is reachable as a top-level destination from
  the main bottom navigation **and** as a dynamic summary card on home, so
  an App Review tester opening the app for the first time sees it without
  guidance.
- The new API endpoint reuses the existing list-pings data path; no new
  collection, index, or background job.
- App Review 2.5.4 resubmission has concrete evidence: in-repo
  reply-letter draft + updated store metadata + new screenshot slot.

**Non-Goals:**

- Per-user xlsx / CSV / PDF export of own pings (deferred to a later
  change if requested; the tester does not need it for 2.5.4).
- Editing or annotating one's own trajectory (rest-break tags, notes).
- Stop-point clustering / dwell-time analysis.
- Historical range beyond 7 days inside the app (server TTL is 90; the
  shorter app window is a UX simplification, not a privacy boundary).
- Changing the Org-level `location_tracking_enabled` toggle semantics or
  the consent flow's gating (only the wording changes).
- Removing or weakening the admin trajectory dashboard.
- A Cupertino-style tab bar — the rest of the app is Material, the new
  navigation stays Material (`NavigationBar`).

## Decisions

### D1. Map library: `flutter_map` + OSM/CARTO Positron tiles

`flutter_map` is chosen over `google_maps_flutter`.

- **Visual continuity** with admin-web, which already uses Leaflet + CartoDB
  Positron — the user sees the same tile style on phone and dashboard.
- **No Google API key** in the release pipeline (one less secret to
  rotate, one less vendor account to keep alive for a free app).
- **Smaller binary** and no Play Services dependency on Android.
- **Same attribution string** the admin-web already complies with
  (`© OpenStreetMap contributors © CARTO`).

Trade-off: `flutter_map` doesn't render native MapKit on iOS, so iOS users
get an OSM tile look rather than Apple Maps. Acceptable — the admin tool is
the visual baseline this app should match.

### D2. New API route: `GET /app/checkin/me/locations`

Add a new handler that **reuses** `LocationPingsCollection::list_by_app_user_paginated`,
with the AppUser id taken from the bearer token (`RequireAppUser`) rather
than a path param. Query params (`before`, `from`, `to`, `limit`) and
validation rules (`INVALID_RANGE`) are identical to the admin endpoint.

Rejected alternatives:

- *Reusing the admin route with a `me` alias*: tempts coupling between admin
  cookie auth and AppUser bearer auth in one handler. Two thin handlers is
  cleaner.
- *Reusing `POST /app/checkin/locations`*: that endpoint is for ingest;
  overloading it would muddy the semantics.

The Org-level `location_tracking_enabled` toggle **does not** gate this
read endpoint — once pings are persisted (because they were ingested while
the toggle was on), the user can always read what was already written about
them, even if the org later disables tracking. This matches the natural
"user owns their data" framing required for 2.5.4.

### D3. Navigation: introduce a Material `NavigationBar` shell

Today the app routes `/home` and `/history` as siblings reached via in-page
buttons. With trajectory added as a third top-level surface, this becomes
unwieldy and — more importantly — App Review wants to see a clear,
discoverable entry point.

Refactor to `StatefulShellRoute.indexedStack` with three branches:

```
   /home           (existing HomeScreen, clock in/out)
   /history        (existing HistoryScreen, event log)
   /trajectory     (NEW, personal trajectory map)
```

The shell scaffold owns a `NavigationBar` with three destinations. Login,
splash, force-password-change, and dev-server-config stay outside the shell
(as today). Destination labels:

| Route          | Icon (Material)          | Label   |
|----------------|--------------------------|---------|
| `/home`        | `Icons.access_time`      | 首頁     |
| `/history`     | `Icons.history`          | 歷史     |
| `/trajectory`  | `Icons.map_outlined`     | 我的軌跡 |

The `IconButton` on home's app bar that opens `/history` is removed (the
nav bar replaces it).

### D4. Trajectory screen UX

```
   ┌──────────────────────────────────────┐
   │ ◀  我的工作日記           [今天 ▾]    │  AppBar + DropdownButton
   ├──────────────────────────────────────┤
   │ ┌──────────────────────────────────┐ │
   │ │                                  │ │
   │ │       flutter_map                │ │
   │ │       (polyline, ✚ start, ✖ end) │ │
   │ │                                  │ │
   │ └──────────────────────────────────┘ │
   │   走動距離  3.2 km                    │
   │   在班時長  4 小時 12 分              │
   │   位置點   38 筆                      │
   │   ───────────────────────────────    │
   │   © OpenStreetMap contributors © CARTO│
   └──────────────────────────────────────┘
```

- **Date picker**: dropdown showing today + last 7 calendar days in the
  Org's timezone (matches admin dashboard's date semantics). Selecting a
  day re-fetches.
- **Empty state** for a day with zero pings: text "該日無軌跡資料" (matches
  admin-web's localized string).
- **Loading state**: skeleton on the map area, spinner-less elsewhere.
- **Permission gate**: if the location permission is *not* granted at all,
  the screen shows a primer card pointing the user at the system settings
  via `app_settings`. (Permission can be denied even though tracking happens
  during shift — system can demote at any time.)
- **Distance calculation**: client-side, sum of geodesic distances between
  consecutive points using `latlong2`'s `Distance().distance()`. No
  server-side aggregation.
- **On-shift duration**: derived from the first and last ping's
  `occurred_at_client` on that day. If we wanted a per-shift granular view
  it'd require crossing with `checkin_events` — out of scope.

### D5. Home summary card

Replace the current home screen's idle area below the clock-in button (or
between the status pill and the queue chip) with a card:

```
   ┌──────────────────────────────────┐
   │ 📍 我的今天                       │
   │                                  │
   │   走動距離  3.2 km                │
   │   在班時長  4 小時 12 分          │
   │                                  │
   │       [ 查看軌跡 → ]              │
   └──────────────────────────────────┘
```

- Visible whenever the AppUser is on shift OR has any pings for today
  (off-shift but already-on-shift-once-today still shows last numbers).
- Tap-through routes to `/trajectory?date=<today>`.
- Pulls the same `GET /app/checkin/me/locations` and computes locally.
- Polls (or listens to the `tickStream` already exposed by
  `LocationTrackingService`) so numbers update during an active shift —
  cap refreshes at one network call every 60s to avoid hammering the API.

### D6. Consent dialog + permission description reword

The current `NSLocationWhenInUseUsageDescription` leads with the org-side
record framing ("產生工作軌跡"). Reword to lead with personal-log framing:

```
你的位置會用來繪製「我的工作日記」，讓你可以在 app 內回顧自己每一天的工作路線
與走動距離。按下「上班」後開始記錄，背景時 iOS 螢幕上方顯示藍色提示，按「下班」
即停止。
```

The clock-in consent dialog (Flutter side) gets the same reword applied to
its body text. Same effect — the message presented to the user *and* the
message presented to App Review reads "this is for you".

### D7. Store metadata reframing

Lead paragraph of `description.txt` reordered so the personal log appears in
the first bullet list. The org-side "管理員工出勤" framing is preserved but
demoted below the personal feature. `promotional_text.txt` mirrors.

A new screenshot slot is reserved at position 2 or 3 (after the
clock-in/out hero shot) for the trajectory map view.

### D8. App Review reply trail lives in-repo

Apple's App Store Connect message thread is the source of truth, but it
isn't replayable from a fresh clone. Drop the resubmission reply letter
under `store_metadata/ios/app_review_replies/2.5.4-2026-05-15.md` so
future maintainers can read what we said in response to which rejection.
Format: rejection guideline, submission id, our reply body verbatim.

## Risks / Trade-offs

- **[App Review may still reject]** → Mitigation: the reply explicitly points
  the reviewer at the new tab, the new home card, and the new screenshot.
  If they still reject (sometimes Apple reads the screenshot before opening
  the app), the next step is to record a short screen capture of the
  feature and attach it to the reply thread. Worst case, fall back to the
  "drop background location" or "Apple Business Manager" branches the
  exploration covered.

- **[`flutter_map` polyline performance with 1000+ points]** → Mitigation:
  enforce the existing 60s / 100m ping throttle on the collection side,
  which caps a 12-hour shift at ~720 points. Tested ranges around this size
  render well on a mid-range iPhone. If a future change loosens the throttle,
  revisit polyline simplification (Douglas–Peucker via `latlong2` or a
  hand-rolled pass).

- **[OSM tile servers may rate-limit]** → Mitigation: use CARTO's Positron
  CDN (same one admin-web uses), which has more headroom than the bare
  tile.openstreetmap.org server.

- **[Distance numbers may surprise users]** → Geodesic distance between
  100m-throttled samples will under-report a winding path. Acceptable
  for a "rough idea of today" surface; explicitly not framed as a fitness
  tracker.

- **[Nav bar refactor regression risk]** → The home screen and history
  screen are existing top-of-funnel screens. Wrapping them in a stateful
  shell route can introduce subtle state-preservation bugs. Mitigation:
  add a widget test that switches tabs and verifies the home screen's
  clock-in state survives the tab switch.

- **[App Privacy form drift]** → The form lives in App Store Connect and
  can't be code-reviewed. Mitigation: a checklist line in DEPLOY.md's
  "iOS cut" section reminds the operator to verify it before submitting.

## Migration Plan

1. API change is additive (new route). Deploy to production first, idle —
   the new endpoint exists but no client calls it yet. No DB change. No
   risk to existing routes.
2. App build 0.3.1 (8) lands. CI exercises the new endpoint.
3. Update store metadata (description, promotional, screenshots).
4. Submit binary to App Store Connect. In the same submission, paste the
   2.5.4 reply letter into the "App Review notes" field and send.
5. If approved → release to production. If rejected again → iterate per the
   risk-1 mitigation above.

No backfill, no schema migration. Rollback for the app side is the
standard "previous build phased rollout" lever in App Store Connect; the
API endpoint can stay live indefinitely (it's harmless if unused).

## Open Questions

- **Demo Org / demo AppUser for App Review**: which Org do we point Apple at,
  and do we pre-seed a day's worth of pings so the reviewer sees a non-empty
  map on the test account? This needs an operator decision before submission;
  flagged in tasks.md.
- **Settings tab**: do we want a fourth "我的" tab now (with logout, server
  config, "delete my data") or leave logout where it is (home app bar)?
  Out of scope here; revisit after 2.5.4 is cleared.
- **iOS 17 location indicator UI change**: showBackgroundLocationIndicator
  still works on iOS 17/18, but worth verifying on the reviewer's iPhone 17
  Pro Max iOS version before resubmission. Smoke step.
