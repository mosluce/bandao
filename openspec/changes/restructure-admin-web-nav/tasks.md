## 1. admin-web: nav data structure & rendering

- [x] 1.1 `layouts/default.vue`: extended the `NavItem` interface — `to` is now optional (`undefined` = non-link label), added optional `children?: NavItem[]`
- [x] 1.2 Rewrote the `navItems` computed per design.md D1: `打卡看板` (top) → `成員管理` (children: `加入申請` if admin) → `App 使用者` (children: `驗證來源` if admin) → `進階工具` label (admin-only, children: `API Token`, `冷卻管理`) → `下載 App` (bottom)
- [x] 1.3 Template: top-level item renders as `NuxtLink` when `to` is present, else as a non-interactive `<p>` label (styled like the "我擁有的 / 我加入的" section labels in `OrgSwitcher.vue`); `children` render as an indented (`pl-6`), slightly muted (`text-slate-600` vs parent's `text-slate-700 font-medium`) list directly beneath, always visible
- [x] 1.4 Confirmed by code review: the badge `<span v-if="child.badge">` markup on the child `NuxtLink` is unchanged from the old top-level version; will re-confirm visually in the group 4 browser smoke
- [x] 1.5 Confirmed by code review: `active-class="bg-slate-100 text-slate-900"` is present on both the parent and the child `NuxtLink`; will re-confirm visually in the group 4 browser smoke

## 2. admin-web: member-view degeneration

- [x] 2.1 Confirmed in the group 4 browser smoke: member sees `成員管理` / `App 使用者` as plain links with no visible sub-list (no separate code path needed — falls out of the computed's `auth.isAdmin.value` check producing empty `children` arrays)
- [x] 2.2 Confirmed in the group 4 browser smoke: `進階工具` is entirely absent from the rendered nav for member (0 text matches anywhere in `<nav>`)

## 3. admin-web: OrgSwitcher dropdown positioning fix

- [x] 3.1 `OrgSwitcher.vue`: root wrapper `relative inline-block text-left` → `relative block w-full text-left`
- [x] 3.2 Dropdown panel: `absolute right-0 z-10 mt-2 w-72 origin-top-right ...` → `absolute left-0 right-0 z-10 mt-2 origin-top ...` (dropped the fixed `w-72`, now matches the full-width wrapper). Toggle button itself left untouched (still `inline-flex`, content-sized) per design.md's scope note.
- [x] 3.3 Confirmed in the group 4 browser smoke via `boundingBox()`: popup renders at `x:16, width:223`, fully inside the viewport, matching the sidebar's fixed ~224px content width

## 4. Docs & verification

- [x] 4.1 admin-web `pnpm typecheck` clean; `pnpm test` 38/38 passed; `pnpm build` clean
- [x] 4.2 Manual browser smoke as admin, real headless-Chromium session (Playwright) against the actual running dev servers. Confirmed via DOM inspection that label positions appear in the exact intended order (打卡看板 → 成員管理 → 加入申請 → App 使用者 → 驗證來源 → 進階工具 → API Token → 冷卻管理 → 下載 App); confirmed `進階工具` renders as `<p>` (0 `<a>` matches, 1 `<p>` match); confirmed clicking the `成員管理` label itself navigates to `/members`; confirmed the pending-request badge renders "1" on `加入申請` with one pending request outstanding (a follow-up script, since the main run had already approved the request before checking); confirmed via `boundingBox()` that the Org switcher popup (`x:16, width:223`) sits fully inside the 1280px viewport, and the full org name "Nav Test Org With A Reasonably Long Name" plus its role badge both render — the pre-fix version would have gone off-screen to the left. Screenshots also visually confirm correct indentation/styling of nested items and the `進階工具` label's muted uppercase treatment.
- [x] 4.3 Manual browser smoke as member, same session: confirmed the flat reduced nav (打卡看板, 成員管理, App 使用者, 下載 App), confirmed zero matches anywhere in the nav for `進階工具` or `加入申請` text.
