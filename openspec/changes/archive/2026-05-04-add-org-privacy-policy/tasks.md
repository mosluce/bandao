## 1. ROADMAP cleanup (preflight)

- [x] 1.1 Remove the archived `add-app-checkin` line from `ROADMAP.md` "下一批 changes" section (already shipped).
- [x] 1.2 Replace the single `add-location-tracking` line with three properly-scoped items: `add-location-tracking-server`, `add-location-tracking-app`, `add-location-tracking-dashboard`. Each carries the decisions locked during the explore session as bullet-form notes (sample 60s AND 100m, whenInUse + UIBgModes:location, batch flush ≥30 / ≥5 min / shift-end, 90-day TTL + admin export, Org toggle default-off + state-locked, Leaflet for the map, etc.).
- [x] 1.3 Add `add-org-privacy-policy` to "下一批 changes" with a one-line note that it'll self-delete on archive.

## 2. Page implementation

- [x] 2.1 Create `admin-web/pages/privacy.vue` with no `definePageMeta({ middleware })` — the route stays public and applies neither `auth` nor `guest` middleware.
- [x] 2.2 Add a top-of-file constant `const LAST_UPDATED_AT = '2026-05-04'` and surface it in section 9 via interpolation. Document in a comment that this constant SHALL only be bumped when the policy text actually changes.
- [x] 2.3 Implement the nine sections per design.md. Use `<h1>` for the page title (隱私政策) and `<h2>` for each section. Use semantic `<section>` wrappers around each heading-body pair.
- [x] 2.4 Implement section 1 (適用範圍): one paragraph covering "Argus 平台 + 您所屬組織的服務".
- [x] 2.5 Implement section 2 (我們蒐集的資料): three sub-bullets — 帳號識別 (email/username, password hash), 出勤事件 (clock_in/out/transfer + 座標 + 反向地理編碼), 位置軌跡 (phrase as "若您所屬組織開啟此功能時，工作期間每分鐘記錄一次").
- [x] 2.6 Implement section 3 (蒐集目的): bullets covering 勞基法 §30 出勤紀錄義務, 工作現場確認 / 安全 / 糾紛佐證.
- [x] 2.7 Implement section 4 (保留期): three lines — 出勤事件 5 年 (cite 勞基法 §30 V), 位置軌跡 90 天, 帳號識別 帳號有效期間.
- [x] 2.8 Implement section 5 (誰能存取): bullets — 您所屬組織的 admin / owner, Argus 平台維運人員 (系統管理需要).
- [x] 2.9 Implement section 6 (您的權利): cite 個資法 §3 / §10 / §11, list 查閱 / 更正 / 刪除 / 撤回同意 (with the legally-recognized caveat that withdrawal does not affect already-collected data per §3 II), and explain that the exercise path is via the user's Org admin.
- [x] 2.10 Implement section 7 (Cookie / Session): two short paragraphs — 必要 cookies 用於登入狀態維持, 不使用追蹤 / 廣告 cookies.
- [x] 2.11 Implement section 8 (聯絡方式): note that Org-specific concerns go to Org owner, plus placeholder `noreply@example.com` for Argus platform contact. Wrap the email in a phrasing or styling that makes the placeholder nature visible (e.g. parenthetical "(placeholder — 待平台運營更新)").
- [x] 2.12 Implement section 9 (政策更新): paragraphs covering 30-day advance notice for material changes plus the last-updated line `最後更新日期：{{ LAST_UPDATED_AT }}`.
- [x] 2.13 Render the disclaimer footer below all sections: "本政策範本未經法律審查，建議您所屬組織自行確認符合當地法規。"
- [x] 2.14 Style the page minimally — reuse existing Tailwind classes from other admin-web pages for headings and body text. No new global CSS, no new component dependencies. Keep the page max-width readable on desktop (e.g. `max-w-3xl mx-auto`).

## 3. Tests

- [ ] 3.1 Add a Nuxt component test at `admin-web/pages/__tests__/privacy.test.ts` (or wherever the existing test pattern lives — check the repo for `*.test.ts` colocation conventions before placing it). Verify the page renders, applies no middleware, and shows all nine section headings + the disclaimer + the placeholder email. *Deferred — `admin-web` has no test framework yet (no vitest / playwright / `*.test.ts` files); spinning one up doubles this change's scope. Tracked as a future ROADMAP item `add-admin-web-test-infra` alongside ESLint setup. Verification falls back to typecheck + manual smoke from §5.*
- [ ] 3.2 Verify that navigating from `/login` to `/privacy` and back does not redirect either way (manual smoke or routing test if the harness supports it). *Deferred for the same reason as 3.1; covered by manual smoke §5.2.*

## 4. Docs

- [x] 4.1 No README update needed for `admin-web` (the page is self-documenting via its content). Optionally append a one-line entry to a future "Pages" reference if such a doc is created — out of scope here.

## 5. Smoke

- [x] 5.1 `pnpm dev` boots admin-web; navigate to `http://localhost:3000/privacy` from a fresh incognito window (no cookies); page renders all nine sections + disclaimer + placeholder email.
- [x] 5.2 Same flow logged in as an admin: navigate to `/privacy`; page renders without redirect to `/checkin` or wherever the post-login default lands.
- [x] 5.3 `pnpm typecheck` clean, `pnpm build` clean.
- [x] 5.4 Open the URL on iOS Simulator's Safari (`http://localhost:3000/privacy` — same machine network); confirm the page is readable on mobile viewport (no horizontal scroll, font sizes legible, sections clearly separated).
