## Why

班到 iOS ships via **Unlisted Distribution** — it is unsearchable on the App Store, so a direct link is the *only* discovery path for iPhone users. Today there is no web page hosting that link (iOS `marketing_url.txt` is blank), and Android's public Play link has no shareable home either. Org admins onboarding staff (small service businesses: cleaning, security, catering) have nothing to hand out. A single public download page gives both stores a shareable home and makes the unlisted iOS build reachable.

## What Changes

- Add a **public, no-auth** `/download` page in admin-web (same pattern as the existing `pages/privacy.vue`, which deliberately applies no middleware) so unauthenticated staff can open a shared link without being bounced to login.
- The page presents both store download points:
  - iOS: `https://apps.apple.com/app/id6767153656` (country-neutral link — app is Taiwan-only under unlisted distribution; avoids a region-locked `/us/` path failing to resolve).
  - Android: `https://play.google.com/store/apps/details?id=tw.ccmos.app.bandao`.
- Each store link rendered as its **official store badge** (Apple "Download on the App Store" / Google "Get it on Google Play"), following each vendor's brand guidelines.
- Each store link accompanied by a **QR code generated client-side** via a JS QR library, for in-person onboarding (admin shows the screen, staff scans).
- The page links to the **privacy policy** (`/privacy`) and the **support email** (`support@ccmos.tw`).
- Add a **"下載 App" NuxtLink** to the "管理員工具" card on the admin home (`pages/index.vue`) pointing at `/download`. The menu entry is only visible to logged-in admins, but the page it links to is public so the same URL can be shared externally.
- After deploy, point iOS store metadata `app/store_metadata/ios/marketing_url.txt` (currently blank) at the live download-page URL.

Out of scope (follow-ups, not this change): User-Agent platform detection to auto-emphasize the matching store button; migrating landing-site content or `/privacy` to a standalone `bandao.ccmos.tw` site (ROADMAP `[cross]` item, decided separately).

## Capabilities

### New Capabilities
- `web-download-page`: a public admin-web page hosting the App Store and Google Play download points (badges + QR codes) plus privacy/support links, reachable without authentication and linked from the authenticated admin home.

### Modified Capabilities
<!-- None. The marketing_url.txt update is a metadata follow-up, not a spec-level requirement change to mobile-release. -->

## Impact

- **admin-web** (Nuxt 3 / pnpm): new `pages/download.vue`; new NuxtLink in `pages/index.vue` "管理員工具" card; new client-side QR-code dependency added to `package.json`; store badge assets added under admin-web public assets.
- **app/store_metadata/ios/marketing_url.txt**: updated post-deploy to the download-page URL.
- No API, database, or auth changes. Reuses the existing public-route pattern from `pages/privacy.vue`.
