## Why

`admin-web` 從一開始就沒測試 framework。CI 只跑 `pnpm typecheck` 與 `pnpm build`，行為層完全沒覆蓋。每次新功能上線都得靠手動 smoke + admin-web 跟 API 的型別契約確認；下一批已規劃的 `add-location-tracking-dashboard`（軌跡視覺化 + Org settings toggle + CSV 匯出）會大量加 component / composable，缺測試 framework 寫起來會反覆遇到「不知道怎麼測」的卡點。

順手把 `add-org-privacy-policy` 當時 deferred 的 §3 測試（privacy.vue render / no middleware / 9 sections + disclaimer + placeholder email）一次補上 — 那筆 deferred 明確點名「等 `add-admin-web-test-infra` 動工」。

## What Changes

- 新增 `admin-web` 的 test 框架：`vitest` + `@nuxt/test-utils` + `happy-dom` + `@vue/test-utils`（dev dependencies）
- 新增 `admin-web/vitest.config.ts`（Nuxt-aware preset）
- 新增 `admin-web/test/` 目錄，結構 mirror `pages/components/composables/middleware`，跟 `app/test/` 慣例一致
- `package.json` 加 `"test": "vitest run"` script（`test:watch` 一併附上）
- `.github/workflows/admin-web.yml` 在 `pnpm typecheck` 之後、`pnpm build` 之前加 `pnpm test` step
- 履行 deferred §3：`admin-web/test/pages/privacy.test.ts` — render / no middleware / 9 個 section heading 都在 / disclaimer 出現 / `noreply@example.com` placeholder 出現
- **不**示範 composable 測試（不在 scope，留給之後 dashboard change 寫的時候帶）
- **不**裝 coverage（`@vitest/coverage-v8`）— 之後要的時候 1 行加

## Capabilities

### New Capabilities

- `admin-web-quality`：admin-web 的工程品質保證面 — 目前只放 test infra 的存在性 + CI gating 承諾 + privacy.vue retroactive test。未來如果加 ESLint、coverage threshold、Playwright e2e 等也擺這。

### Modified Capabilities

（無）

## Impact

- **`admin-web/`**：新增 4 dev deps、`vitest.config.ts`、`test/` 目錄、`package.json` script。
- **CI**：`admin-web.yml` 工作流多一個 step。estimated +20-40s 執行時間，可接受。
- **`api/` / `app/`**：完全不動。
- **Spec / Mongo schema**：不動。
- **未來 Nuxt v4 升級**：選用的 `@nuxt/test-utils` 最新版有 v4 相容路線；component test 的 `mountSuspended` 內部 API 升級時可能要小調，composable / pure-TS test 風險最低。
