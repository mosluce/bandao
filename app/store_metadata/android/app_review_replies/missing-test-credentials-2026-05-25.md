# Play policy reply — missing test credentials (2026-05-25)

- **App / Version:** 班到 (Bandao) 0.3.0 (versionCode 4) — first public submission
- **Policy cited:** Play Console policy — "缺少試用/訪客帳戶詳細資料"
  (Missing trial / guest account details). Reviewer was unable to sign in
  past the org-code / username / password gate, so the rest of the app
  could not be evaluated.
- **Reply date:** 2026-05-25
- **Resubmission strategy:** Metadata-only fix. The .aab itself is fine;
  we just need to populate **Play Console → App content → App access**
  with working demo credentials and resubmit the existing release for
  review. No versionCode bump needed unless Play forces a new release
  (in which case bump to `versionCode 5` and add
  `app/store_metadata/android/changelog/5.txt` identical to `4.txt`).

---

## What to fill into Play Console

### App content → App access

Select **"All or some functionality in my app is restricted"**, then add
one access instruction with the following fields:

```
Name of section:    Sign in to 班到 (Bandao)

Username:           demo
Password:           demodemo

Any other info the reviewer needs to access your app:
  Organization code (組織代碼) is required as a third field on the login
  screen. Enter: demo

  After signing in, the home screen exposes the full feature set:
    1. 上班 (Clock in) — bottom-left primary button. Tapping it requests
       fine-location permission via a foreground notification (no
       background-location permission is requested) and starts a
       work-shift trajectory. A persistent notification stays visible
       for the duration of the shift, per Foreground Service policy.
    2. 下班 (Clock out) — same button flips to 下班 once on shift; tap
       to end the shift.
    3. 我的軌跡 (My Trajectory) — third tab in the bottom nav. Opens
       "我的工作日記 / My Work Day", showing a per-day map polyline of
       the signed-in user's own movement during their shift, with
       distance + on-shift duration. A dropdown picks today or any of
       the previous 7 days.
    4. 歷史 (History) — second tab. Lists past clock-in / out events;
       pull-to-refresh.

  The demo account has location pings pre-seeded for at least one
  recent day so the trajectory tab is not empty when the reviewer
  opens it.
```

### App content → Target audience and content

No change needed for this rejection — the rejection was scoped to App
access, not to target-audience / data-safety / content-rating questions.

---

## Optional reviewer-facing reply (if Play offers a message box)

Some Play policy decisions expose a free-text "reply to Google Play"
box on the policy-review status page. If that box is present, paste:

> Thank you for the review. The rejection was caused by missing test
> credentials in **App content → App access**. We have added a demo
> sign-in (org code `demo`, username `demo`, password `demodemo`) that
> grants full access to every reviewable surface of the app — clock
> in / out, trajectory tab, history. The demo account is seeded with
> recent location pings so the trajectory view renders a visible
> polyline. No app code changes were required; the same .aab
> (versionCode 4) is being resubmitted.
>
> If anything further is unclear, we are happy to record a short screen
> capture and attach it.
>
> Thank you,
> The Bandao team
> support@ccmos.tw

---

## Demo credentials

```
Org code:   demo
Username:   demo
Password:   demodemo

Seeding:    Before resubmitting, verify the demo user still has at
            least one full day of location pings within the last 7
            days so 我的軌跡 renders a polyline. Quickest path:
            sign in on a real device with the demo creds, tap 上班,
            walk for a few minutes, tap 下班. Confirm the polyline
            appears under 我的軌跡 before clicking "Send for review"
            in Play Console. (Same seeding rule as the iOS App Review
            checklist — see DEPLOY.md "App Review submission
            checklist".)
```

---

## Internal notes (not sent to Google)

- This file is the source of truth for the Play Console App-access copy
  and the optional reply message. Operator copy-pastes the relevant
  sections into Play Console at resubmit time.
- If Google rejects again with a different sub-rationale, archive this
  file with a `-r1` suffix (e.g. rename to
  `missing-test-credentials-2026-05-25-r1.md`) and create a new dated
  reply file alongside.
- Related artifacts:
  - `app/store_metadata/ios/app_review_replies/2.5.4-2026-05-15.md` —
    the iOS-side analogue; uses the same demo creds.
  - `DEPLOY.md` — "App Review submission checklist" section now
    includes the Play-side equivalent.
