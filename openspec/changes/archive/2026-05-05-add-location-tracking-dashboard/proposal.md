## Why

`add-location-tracking-server` 與 `add-location-tracking-app` 把資料管路串通：手機上傳 pings、API 持久化、Org toggle 控制收集。但 admin 看不到收下的資料 — 沒有任何 UI 把軌跡視覺化、沒有 toggle 給 admin 控制 Org 設定、沒有按鈕讓 admin 匯出 xlsx。dashboard 端是這套 feature 真正落地給 admin 用的最後一哩。

順手把兩個小漏洞補掉：
1. `GET /checkin/users/:id/locations` 只有 `before` cursor，沒辦法用 `from` / `to` 取一天的軌跡。dashboard 需要這個 — 不擴的話只能多次分頁＋客戶端過濾，浪費頻寬與複雜度。
2. ROADMAP 寫「CSV streaming」但 API 實作為 xlsx — 文字過時，動工時校正。

## What Changes

### API（modified `location-tracking` capability）

- `LocationListQuery` 加 `from` / `to`（RFC3339）— 沿用 export 的驗證規則：解析失敗 / `to < from` / `to - from > 90 days` / `now - from > 90 days` 皆回 `INVALID_RANGE`
- 缺一個的話視為單側未限制（仍套 `before` 與 `limit`）— 容忍只給 `from` 或只給 `to` 的呼叫
- 既有 `before` cursor 行為保留（dashboard 拿一天可以靠 `from` + `to` 一次撈，但其他用途仍可走 cursor）

### admin-web（new `admin-trajectory-dashboard` capability）

- **軌跡頁** 路由 `/checkin/:appUserId/trajectory?date=YYYY-MM-DD`
  - Leaflet vanilla（不裝 vue-leaflet wrapper）+ CartoDB Positron tiles + `© OpenStreetMap contributors © CARTO` attribution
  - polyline of pings (chronological) + event markers（clock_in/out、transfer_in/out 用不同顏色/icon）+ 自動 fit-bounds
  - 日期 picker：native `<input type="date">`，預設 today
  - 該日無資料 → `「該日無軌跡資料」` 提示，**不顯示地圖**
  - URL `?date=` 跟 picker 雙向綁定
  - Org timezone 對應：`date=2026-03-01` 在 +08:00 Org 對應 `2026-03-01T00:00:00+08:00` ~ `2026-03-02T00:00:00+08:00`
  - 從既有 `/checkin/:appUserId` user-detail 頁加一個「查看軌跡」連結進來
- **Org settings toggle** 加 `location_tracking_enabled` 在 `pages/index.vue` 既有 settings 區塊，緊接 `transfer_enabled` 的下一格 `<dt>/<dd>` pair。state-locked 錯誤訊息用既有翻譯。
- **匯出按鈕** 在軌跡頁右上，開 modal 選 from/to date range，呼叫 `GET /locations/export?from=&to=` 觸發 xlsx 下載（`<a download>` direct，cookie auth 帶過去）

### Types & dep

- `admin-web/types/api.ts` 加 `OrgCheckin.location_tracking_enabled`、`UpdateOrgSettingsRequest.location_tracking_enabled`、`LocationPingDto`、`LocationListQuery` 的 from/to
- `admin-web/composables/useLocationPings.ts` 新增 — 包 `GET /locations`，`list({ appUserId, from, to })` 一次請求拿完一天
- 新增 dev dep `leaflet` + `@types/leaflet`，import `leaflet/dist/leaflet.css`

### Tests（admin-web vitest，沿用 `add-admin-web-test-infra` 鋪好的 framework）

- `useLocationPings` composable test — 過 `$fetch` mock 驗 from/to 構造正確
- 軌跡頁 empty state（mock `useLocationPings` 回空陣列 → 顯示提示、不 mount Leaflet）
- 軌跡頁 with-data path（mock 回幾筆 ping → polyline 元素存在、map 元素 mount）— Leaflet 在 happy-dom 跑可能要 mock 部分 API（`getBoundingClientRect`、`createPane`），有問題的話降級為「驗 component 內部 state 而非 DOM」
- Org settings 頁 toggle：mock `useOrgSettings` 驗 toggle 啟動正確 PATCH body
- API：unit + integration test for from/to filter — `INVALID_RANGE` 各 4 種觸發、單側 from/單側 to 行為

### 文件 / ROADMAP

- `admin-web/README.md` 加軌跡頁、設定面、匯出說明段落
- `ROADMAP.md` 把 `add-location-tracking-dashboard` 條目刪除（archive 後刪）；連帶把過時的 「CSV streaming」字面修正到正確的 xlsx —（其實這個 ROADMAP 條目本身就會被刪，所以「修正」等於刪掉）

## Capabilities

### New Capabilities

- `admin-trajectory-dashboard`：admin 觀看 / 操作 location tracking 的 dashboard 面 — 軌跡視覺化頁、Org 設定面 location_tracking_enabled toggle、xlsx 匯出流程

### Modified Capabilities

- `location-tracking`：`GET /checkin/users/:id/locations` 加 `from` / `to` 過濾參數

## Impact

- **`api/`**：擴 `LocationListQuery` 與 `list_locations` handler、補 spec、加 unit + integration test
- **`admin-web/`**：新增 trajectory page、`useLocationPings` composable、修改 `pages/index.vue` 加 toggle、修改 `types/api.ts`、修改 `useOrgSettings.ts`（既有就 pass-through，幾乎無動）、加 leaflet deps、加 4-5 個 vitest test
- **`app/`**：完全不動
- **MongoDB schema**：不動（pings 已有所有需要欄位）
- **隱私 / spec narrative**：軌跡頁是 admin-only、隱私政策已涵蓋（admin 可查、僅組織內管理員）
- **依賴關係**：`add-location-tracking-server` ✓、`add-location-tracking-app` ✓、`add-admin-web-test-infra` ✓ 都 archived，dashboard 是這套的收尾
