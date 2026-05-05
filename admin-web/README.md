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
pnpm test         # vitest 一次性執行（CI 用）
pnpm test:watch   # vitest 監看模式（本地開發）
pnpm build        # 產 production bundle 到 .output/
pnpm preview      # 跑 production bundle 本地預覽
pnpm generate     # 預先 render（SPA 模式下幾乎等同 build）
```

## 測試

`vitest` + `@nuxt/test-utils` + `happy-dom` + `@vue/test-utils`。測試檔放 `admin-web/test/`，目錄結構 mirror `pages/components/composables/middleware`，跟 `app/test/` 慣例一致。`pages/privacy.test.ts` 是寫 component test 的範本，新增測試時直接複製改寫即可。CI 在 typecheck 後、build 前跑 `pnpm test`，失敗就擋下來。

## 結構

```
pages/        # 路由頁面
  login.vue / register.vue
  index.vue                # 當前組織總覽 + 離開組織
  members.vue              # 成員管理（含 owner transfer 表單）
  cooldowns.vue            # 冷卻管理
  app-users/index.vue      # AppUser CRUD（admin only）
  checkin/index.vue        # 打卡看板（admin only）
  checkin/[appUserId].vue  # 單個 AppUser 事件歷史（admin only）
  no-org.vue               # 0 個 membership 時的著陸頁
  orgs/new.vue             # 已登入時建立新組織
  orgs/join.vue            # 已登入時用 org_code 加入新組織
components/   # 跨頁面共用
  OrgSwitcher.vue          # header dropdown，切換 / 建立 / 加入 Org
  OrgCreateForm.vue        # createOrg 包裝
  OrgJoinForm.vue          # joinOrg 包裝
composables/  # useApi、useAuth、useOrgSlug、useAppUsers、useCheckin、useOrgSettings、useOrgTime
middleware/   # auth（要登入；current_org=null 時導去 /no-org，除非路徑屬於 ORG_AGNOSTIC_PATHS）、guest（已登入則導走）
types/        # 對應 api 的 DTO 型別（手寫 mirror，OpenAPI codegen 在 ROADMAP）
assets/css/   # Tailwind entry
```

## Multi-org 流程

- `useAuth()` 暴露 `user` / `memberships` / `currentOrg` / `role`，以及 `createOrg` / `joinOrg` / `switchOrg` / `leaveOrg` / `transferOwnership` 行為。
- localStorage `argus.lastSelectedOrgId` 記住最後選的 Org；下次登入會自動 `switchOrg` 對齊 server。
- 沒有任何 membership 的使用者會被 middleware 導去 `/no-org`，那邊有併排的「建立新組織」與「加入既有組織」表單。
- `OrgSwitcher` 在每個 page header 都看得到（`pages/index.vue`、`members.vue`、`cooldowns.vue`），分組顯示「我擁有的」「我加入的」+「+ 建立新組織」「+ 用 org code 加入」入口。
- 切換 Org 後，依賴 server 資料的 page（members / cooldowns）會 watch `currentOrg.value?.id` 自動重打 API。

## Vanity slug UI

`pages/index.vue` 在組織資訊區塊內並列「組織代碼」與「自訂代碼」。admin 可以：

- 設定 slug（首次免限制，之後每 30 天一次）
- 變更 slug：舊 slug 進 30 天 grace，仍能被 join
- 清除 slug：同樣進 30 天 grace
- 邀請連結優先用 slug，沒有時 fallback 到 code

錯誤訊息對應 `ApiError.code`：`INVALID_SLUG_FORMAT` / `SLUG_RESERVED` / `SLUG_TAKEN` / `SLUG_CHANGE_TOO_SOON`（含 `retry_after` 時間戳）/ `FORBIDDEN`。non-admin 看得見 slug 但沒有 Edit / Clear 按鈕。

## App 使用者管理

`pages/app-users/index.vue`（admin only）。建立 AppUser 時 server 會產一次性初始密碼，admin-web 用 modal 顯示一次（含複製按鈕），關閉後 client 端不再持有；admin 線下告知員工。重設密碼走相同 ceremony：確認 → 產新密碼 → modal 顯示一次 → 該 AppUser 全部 sessions 被斷線、下次登入強制改密碼。停用 / 啟用即時生效（停用會立刻 invalidate 所有 sessions；啟用後沿用舊密碼）。錯誤碼：`USERNAME_TAKEN` / `INVALID_USERNAME_FORMAT` / `FORBIDDEN` / `NO_ACTIVE_ORG`。

## 加入申請審核 UI

`pages/admin/join-requests.vue`：admin-only。列當前 Org 的申請，可篩 pending / approved / rejected / cancelled。pending row 有 `[同意]` `[拒絕]` 兩個 action — 拒絕會開 modal 讓 admin 填可選的 `rejection_reason`（≤ 500 字）。`/`（home）的「加入申請」連結會顯示紅點 badge（`N 筆待審核`），由 30 秒輪詢的 `useJoinRequests.countOrgPending()` 驅動。

申請者端：`/no-org` 頁列出自己的申請紀錄（`useJoinRequests.listMine()`），pending 的可一鍵取消，rejected 的會顯示拒絕理由。`OrgJoinForm`（home 設定面 + `/orgs/join`）submit 後不再立刻把 user 加進 Org，而是顯示「已送出申請，等待 X 管理員審核」提示。

## 擁有權轉移 UI

`pages/members.vue`：當前使用者是 owner 時，每位非自己的 admin 會多一個「轉移擁有權」按鈕。點下去 inline 展開密碼欄位，密碼正確 + 對方確實是 admin 才會成功。轉移後 `org.owner_id` 變成對方，原 owner 立刻變成可降級 / 可被踢 / 可自離的普通 admin（UI 自動 reflect，因為 `auth.refresh()` 會被呼叫）。錯誤碼：`INVALID_PASSWORD` / `INVALID_TARGET` / `SAME_OWNER` / `FORBIDDEN`。

## 打卡看板與設定

`pages/checkin/index.vue` 是 admin-only 即時看板，依狀態（在班 / 移動中 / 下班）分組列出 AppUser、最後事件、shift duration、skew warning，每 30 秒自動 refresh，可直接觸發強制收班（含選填 reason）。`pages/checkin/[appUserId]/index.vue` 是單人事件歷史，cursor 分頁、`載入更多`、每筆顯示事件類型 / Org TZ 時間 / 地點 / source badge / reason。

`pages/checkin/[appUserId]/trajectory.vue` 是單人單日的軌跡視覺化（依賴 Org `location_tracking_enabled` 開啟）：URL `?date=YYYY-MM-DD` 對應 Org 時區的當日；client 把 date 換成 RFC3339 區間打 `GET /checkin/users/:id/locations?from=&to=`，用 Leaflet + CartoDB Positron tiles 畫 polyline，事件 (clock_in / out / transfer_in / out) 用顏色不同的 circle marker 疊上去，地圖 fitBounds 自動覆蓋。該日無 ping 顯示「該日無軌跡資料」**不顯示地圖**。右上角「匯出 xlsx」按鈕開 modal 選 from/to（client 端先擋 `to < from` / span > 90 天），通過後直接 `<a href download>` 走 cookie session 觸發 `GET /checkin/users/:id/locations/export?from=&to=`。

`pages/index.vue` 在「打卡設定」段落讓 admin 切換 `transfer_enabled` 與 `location_tracking_enabled`（兩者都 state-locked — 有人在班時 server 回 `STATE_LOCKED`，UI 顯示「目前有 App 使用者在班，需先全部下班才能調整此設定」），以及 `timezone`（下拉選單列常見 IANA + 自訂輸入；不合法值回 `INVALID_TIMEZONE`）。

時間 render 用 `useOrgTime.formatInOrgTz(iso, org.timezone)`；missing TZ 時 fallback 到瀏覽器 locale。`shiftDuration(iso)` 算上班至今的時數 / 分鐘給看板用。

## 已知 / 暫緩

- ESLint 暫未裝，計畫見 `ROADMAP.md`。
- 升 Nuxt v4 已記在 `ROADMAP.md`，目前 pin 在 `3.21.2`（避開 3.21.3+ ssr:false 的 vite-node IPC regression）。
