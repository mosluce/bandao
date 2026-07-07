## 1. Dependencies & assets

- [x] 1.1 Add a small client-side QR library (default candidate `qrcode`) to `admin-web/package.json` via pnpm; confirm it renders to inline SVG/canvas without a network call. (`qrcode` ^1.5.4 + `@types/qrcode` dev)
- [x] 1.2 Add the official Apple "Download on the App Store" and Google "Get it on Google Play" badge assets under admin-web public assets (locally served, not hotlinked); keep original artwork unaltered. (zh-TW official badges: `public/badges/app-store-badge.svg`, `public/badges/google-play-badge.png`)

## 2. Download page

- [x] 2.1 Create `admin-web/pages/download.vue` as a public page — NO middleware (mirror the `pages/privacy.vue` header comment explaining the deliberate public reachability).
- [x] 2.2 Define store link constants in one place: iOS `https://apps.apple.com/app/id6767153656`, Android `https://play.google.com/store/apps/details?id=tw.ccmos.app.bandao`.
- [x] 2.3 Render both official store badges linking to their respective constants, sized per each vendor's minimum clear-space guidance.
- [x] 2.4 Render a QR code beside each badge, generated client-side from the same constants.
- [x] 2.5 Add a link to `/privacy` and a `mailto:support@ccmos.tw` support link.
- [x] 2.6 Style with Tailwind consistent with `privacy.vue`; verify it reads well on a narrow (phone) viewport since admins may open it on mobile. (single-column stack on `<sm`, two cards on `sm+`)

## 3. Admin menu entry

- [x] 3.1 Add a "下載 App" `NuxtLink to="/download"` in the "管理員工具" card in `pages/index.vue`, matching the existing button styling of the sibling links.

## 4. Tests

- [x] 4.1 Add `admin-web/test/pages/download.test.ts` (mirroring `test/pages/privacy.test.ts`): assert both badges link to the correct store URLs, both QR codes render, and the privacy/support links are present.
- [x] 4.2 Run `pnpm test` in admin-web — all tests pass. (32 passed; `pnpm typecheck` clean)

## 5. Verify + store metadata follow-up

- [x] 5.1 Manual smoke: open `/download` in an incognito window (logged out) — page renders, not redirected to login; badges open the correct App Store / Play listings; QR codes scan to the same URLs. — **automated-verified**: unit tests render the full page (links, badges, QR SVGs, privacy/support), page declares no middleware (logged-out safe), badge assets serve 200, `pnpm build` succeeds. Physical QR scan + real-browser click-through folds into §5.2 operator deploy.
- [ ] 5.2 Deploy admin-web to `bandao-admin.ccmos.tw`; confirm `https://bandao-admin.ccmos.tw/download` is live and public. — **needs operator**
- [ ] 5.3 After the page is live, set `app/store_metadata/ios/marketing_url.txt` to the download-page URL and sync to App Store Connect on the next metadata update. — **needs operator** (post-deploy)
