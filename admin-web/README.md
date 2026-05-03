# argus admin-web

Nuxt 3 + TypeScript + Tailwind CSS。給 Org admin / member 用的管理後台，純 SPA（`ssr: false`），靠 cookie session 跟 `api/` 通訊。

## 開發前置

- Node 20+（建議用 nvm / asdf / volta）
- pnpm 9+
- 後端 API 已啟動（看 [`../api/README.md`](../api/README.md)）

## 跑起來

```bash
cd admin-web
cp .env.example .env       # 首次：根據實際 API port 調整
pnpm install
pnpm dev
```

預設 `http://localhost:3000`。Vue DevTools 與 Nuxt DevTools 都會增加 dev server 重 render 成本，DevTools 在 `nuxt.config.ts` 已預設關閉，需要時再臨時打開。

## 環境變數

| 變數 | 預設 | 說明 |
| --- | --- | --- |
| `NUXT_PUBLIC_API_BASE_URL` | `http://localhost:8080` | API 的 base URL；`useApi()` 會帶 `credentials: 'include'`，所以只要 API 端 CORS 允許具體 origin 即可 |

## 常用命令

```bash
pnpm dev          # 開發 server，預設 :3000
pnpm typecheck    # vue-tsc / Nuxt TypeScript 檢查
pnpm build        # 產 production bundle 到 .output/
pnpm preview      # 跑 production bundle 本地預覽
pnpm generate     # 預先 render（SPA 模式下幾乎等同 build）
```

## 結構

```
pages/        # 路由頁面（login / register / index / members）
composables/  # useApi（$fetch 包裝）、useAuth（reactive auth state）、useOrgSlug（vanity slug set/clear）
middleware/   # auth（要登入）、guest（已登入則導走）
types/        # 對應 api 的 DTO 型別（手寫 mirror，OpenAPI codegen 在 ROADMAP）
assets/css/   # Tailwind entry
```

## Vanity slug UI

`pages/index.vue` 在組織資訊區塊內並列「組織代碼」與「自訂代碼」。admin 可以：

- 設定 slug（首次免限制，之後每 30 天一次）
- 變更 slug：舊 slug 進 30 天 grace，仍能被 join
- 清除 slug：同樣進 30 天 grace
- 邀請連結優先用 slug，沒有時 fallback 到 code

錯誤訊息對應 `ApiError.code`：`INVALID_SLUG_FORMAT` / `SLUG_RESERVED` / `SLUG_TAKEN` / `SLUG_CHANGE_TOO_SOON`（含 `retry_after` 時間戳）/ `FORBIDDEN`。non-admin 看得見 slug 但沒有 Edit / Clear 按鈕。

## 已知 / 暫緩

- Nuxt 鎖在 `~3.14.0`，會看到 `'manifest-route-rule' middleware already exists` 警告，3.14.x 已知 bug，cosmetic、不影響功能。升 v4 已記在 `ROADMAP.md`。
- ESLint 暫未裝，計畫見 `ROADMAP.md`。
