## Context

`admin-web` 是 Nuxt 3.21.2 SPA（`ssr: false`），用 `@nuxtjs/tailwindcss`、5 個 composable、3 個 component、11 個 page route，CI 在 GitHub Actions 跑 `pnpm typecheck` + `pnpm build`。完全沒測試 framework。

對比 `app/`：Flutter 端有完整 unit + widget test 套，139 tests pass，CI 跑 `flutter analyze` + `flutter test`。`api/` Rust 端也有 unit + integration（testcontainers）。**只有 `admin-web` 是空白**。

歷史 deferred：`add-org-privacy-policy` 的 §3「privacy.vue 測試」明確標記「等 `add-admin-web-test-infra`」。

Nuxt v4 升級在 ROADMAP，已排程 2026-05-17 背景檢查 `nuxt/nuxt#34957` — 解了 pin 後動。本 change 寫的測試風格要對得上 v4 升級路徑，不要綁太深。

## Goals / Non-Goals

**Goals:**
- 提供 admin-web 可立即使用的 vitest + Nuxt-aware 測試環境
- CI 跑 `pnpm test` 守住行為層
- 履行 `add-org-privacy-policy` §3 deferred 的 privacy.vue 測試
- 設定一個下次寫 component / composable test 時不用再思考的「就照範本貼」目錄結構與配置

**Non-Goals:**
- 不示範 composable 測試（留給 `add-location-tracking-dashboard` 寫的時候按需擴）
- 不裝 coverage 工具
- 不改現有 source code 加 test hook
- 不引入 Playwright e2e 或 visual regression
- 不為 `pages/index.vue`、`pages/login.vue` 等既有頁面回填測試（範圍爆炸）
- 不解 Nuxt v4 pin（獨立 change）

## Decisions

### D1：Stack = Vitest + @nuxt/test-utils + happy-dom + @vue/test-utils

**Why**：
- **Vitest** 是 Nuxt 文件 / 社群預設選擇，Vite-native、fast watch、Jest-compatible API
- **@nuxt/test-utils** 提供 `mountSuspended`、自動 inject runtime config / auto-import / Nuxt context — 否則自己手動 mock 各種 `useRuntimeConfig`、`useFetch` 黑魔法
- **happy-dom** 比 jsdom 啟動快約 4×、API surface 對 SPA Vue components 夠用；Nuxt 文件預設範例就是它
- **@vue/test-utils** mount + interaction helpers，常用就是 `.text()`、`.find()`、`.trigger()`

**Alternatives**：Jest（不 Vite-native，整合 Nuxt 麻煩）、Cypress component testing（重、需要 browser）、Vitest browser mode（實驗性、跑 real browser，不是這個 change 要的成本）。

### D2：測試檔放 `admin-web/test/`，mirror source 結構

```
admin-web/
  test/
    pages/
      privacy.test.ts        ← 對應 pages/privacy.vue
    components/              ← 之後加
    composables/             ← 之後加
    middleware/              ← 之後加
```

**Why**：
- 跟 `app/test/features/...` 的 mirror 慣例一致，整個 repo 一個習慣
- 測試檔不污染 source 樹；publish / build 時不會誤打包
- 找測試直接看 `test/` 目錄，不用 `find . -name "*.test.ts"`

**Alternatives**：
- `__tests__/` 資料夾（React 慣例，Vue 圈少用）
- Co-locate（`pages/privacy.test.ts`）— Vue 圈也常見，但會跟 Nuxt 的 `pages/` route 自動掃描衝突（Nuxt 會把 `*.test.ts` 也當 route 嗎？實測過會跳警告）

### D3：vitest.config.ts 用 `defineVitestConfig` 從 `@nuxt/test-utils/config`

```ts
import { defineVitestConfig } from '@nuxt/test-utils/config'

export default defineVitestConfig({
  test: {
    environment: 'nuxt',
    environmentOptions: {
      nuxt: {
        domEnvironment: 'happy-dom',
      },
    },
  },
})
```

**Why**：官方推薦寫法，自動 inject Nuxt 環境（auto-imports、runtime config 等）。`environment: 'nuxt'` 是 @nuxt/test-utils 註冊的 vitest environment。

### D4：不裝 coverage（`@vitest/coverage-v8`）

**Why**：MVP 階段、test 數量少，coverage 數字沒參考價值；裝了反而 noise。要的時候 1 行 deps + `--coverage` 就有。

### D5：CI 把 `pnpm test` 放在 typecheck 後、build 前

**Why**：
- typecheck 失敗 → 不用跑 test（早 fail）
- test 失敗 → 不用 build（早 fail）
- 順序 typecheck → test → build 是「快檢查 → 行為檢查 → 產物建置」的合理階梯

CI 預估 +20-40 秒。可接受。

### D6：privacy.test.ts 涵蓋範圍

對應 `add-org-privacy-policy` deferred §3：
1. 頁面能 mount 不報錯
2. **不套用 middleware**（檢查 `definePageMeta` 沒有 middleware key，或檢查 `auth` / `guest` middleware 沒被 invoke — 用最直接的：file-level 解析確認沒有 `middleware:` 出現）
3. 9 個 section 的 `<h2>` heading 都 render（`1. 適用範圍` 到 `9. 政策更新`）
4. Disclaimer 字串 `本政策範本未經法律審查` 出現
5. Placeholder email `noreply@example.com` 出現

**為什麼不寫 navigation test**：要驗 middleware 真的不套，需要起 Nuxt router + middleware pipeline。`@nuxt/test-utils` 的 `mountSuspended` 不跑 middleware（component 層級 mount），所以 unit 層的測試不會誤套。退而求其次：用 `fs.readFileSync` 解析 `privacy.vue` 確認沒有 `middleware:` 字串。簡單實用。

### D7：Mock 策略

privacy.vue 是純靜態頁面，沒有 `$fetch`、沒有 composable 呼叫，不需要任何 mock。第一個測試純粹 mount + 斷言文字內容。

未來其他 component / composable 要 mock `$fetch` 時，建議走 `@nuxt/test-utils` 的 `mockNuxtImport` 或 `vi.mock('#app')`，但範例這次不寫（不在 scope）。

## Risks / Trade-offs

- **Nuxt v4 升級 break**：`@nuxt/test-utils` API 內部變動是已知風險。**Mitigation**：寫的測試只用穩定 API（`mountSuspended`、`fs.readFile`），不碰 `mockNuxtImport` 等 unstable 路徑。風險面集中在 1 個檔案，升級時 review 成本可控。
- **happy-dom 偶爾跟 jsdom 行為不同**（如 `getBoundingClientRect` 回 0）：privacy.vue 是純文字頁面，撞不到。日後 component test 撞到再切 jsdom。
- **CI 時間增加**：vitest 啟動 + happy-dom 初始化 ~5s + 1 個 test ~50ms = 約 +6-8s，比預估更短。
- **單一測試檔的範本效應**：`privacy.test.ts` 將被未來開發者複製改寫；要寫得乾淨，否則漂移。
- **無 watch script 的 IDE 整合**：本 change 提供 `pnpm test:watch` 但 IDE 整合（VSCode vitest extension）由開發者各自設定，不在 repo config 內。

## Migration Plan

純加值。沒有舊行為要 migrate：

1. 安裝 deps、寫 config、寫 test、跑 `pnpm test` 確認通過
2. CI 加 step
3. 推上去看 GitHub Actions 是否綠
4. 後續 change 開始把 deferred / 新功能的 test 補進來

**Rollback**：刪 `vitest.config.ts` + `test/` + 移除 `package.json` 的 test script + revert CI step。`@nuxt/test-utils` 等 deps 留在 lockfile 也無害。
