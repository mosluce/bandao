## Context

```
            App (geolocator)
                  │ pings
                  ▼
            POST /app/checkin/locations  ──── add-location-tracking-server ✓
                  │
                  ▼
            MongoDB location_pings
                  │
                  ├─── add-location-tracking-app ✓ (toggle, consent, batch flush)
                  │
                  ▼
            GET /checkin/users/:id/locations  ──── 已有 cursor pagination
            GET /checkin/users/:id/locations/export ──── xlsx, max 90 天

         ┌──────────── 此 change 補上 ────────────┐
         │                                          │
            admin-web 軌跡頁                         │
            admin-web Org settings toggle UI         │
            admin-web xlsx 下載按鈕                   │
         │                                          │
         └──────────────────────────────────────────┘
```

整套 location tracking 功能差最後這塊 dashboard 給 admin 用。沒這頁的話：
- Admin 拿不到收集到的軌跡（只能 mongosh）
- Admin 沒辦法在 UI 上開 / 關 Org toggle（只能 curl `/orgs/me/settings`）
- Admin 沒有 xlsx 匯出（API 已實作但沒入口）

`add-admin-web-test-infra` 剛 archive，dashboard 端 vitest 已經 ready，這個 change 直接受益。

## Goals / Non-Goals

**Goals:**
- Admin 在 admin-web 上能用日期看任一 AppUser 當天的軌跡
- Admin 能在 Org settings 切換 `location_tracking_enabled`，跟 `transfer_enabled` 相同 UX
- Admin 能匯出指定 AppUser 指定區間的 xlsx
- API 補上 dashboard 撈一天資料所需要的 from/to 過濾
- 軌跡頁照既有 `pages/checkin/[appUserId].vue` 的 url / page 結構慣例

**Non-Goals:**
- 不做 polyline simplification（Douglas-Peucker / Ramer-Douglas-Peucker）
- 不做即時更新（polling / SSE / WebSocket）— 重新整理頁面就好
- 不做多 AppUser 同時顯示（一頁一人）
- 不做地圖樣式切換（單一 CartoDB Positron）
- 不做歷史日期跳轉的 deep navigation（只有 date picker，無「上一天 / 下一天」按鈕）
- 不做地圖 export（截圖、PDF 等）
- 不擴 events list 的 from/to —  events 數量低（~5/天/人），現有 cursor 即可
- 不重構既有 `pages/index.vue` settings 區塊

## Decisions

### D1：admin-web 軌跡頁路由 = `/checkin/:appUserId/trajectory`

**Why**：
- 沿用既有 `pages/checkin/[appUserId].vue` 慣例（Nuxt route 為 `/checkin/:appUserId`）
- 軌跡頁是 user 詳情的「另一個視角」，放在同層 `/checkin/:appUserId/trajectory` 最直觀
- 對應 Nuxt file: `pages/checkin/[appUserId]/trajectory.vue`（會把現有 `[appUserId].vue` 變成 `[appUserId]/index.vue`）

**Alternatives**：
- `/users/:id/trajectory`（API path mirror）— 與 admin-web 既有路由樹不對齊
- `/trajectory/:appUserId`（功能優先）— 跟現有 user-detail 切開，但這頁本質是 user-detail 的延伸

### D2：Leaflet vanilla，不裝 `vue-leaflet`

**Why**：
- `vue-leaflet` 不維護穩定（社群多 fork、API drift）
- Vue 3 + Leaflet vanilla 用 `onMounted` 取 DOM ref → `L.map(el)` 是 5 行 idiom，不需要 wrapper
- bundle 小、依賴最少
- happy-dom 環境下測試只需要 mock `getBoundingClientRect` 與 `createPane`，wrapper 反而黑盒不好 mock

**Alternatives**：
- `vue-leaflet`（前述）
- MapLibre / Mapbox GL（殺雞用牛刀）
- Google Maps / Apple Maps（商業 API key、隱私顧慮）

### D3：CartoDB Positron tiles

**Why**：
- 免費，attribution policy 寬鬆（標 `© OpenStreetMap contributors © CARTO`）
- Positron 是低調 light gray 風格、polyline + markers 視覺乾淨
- OSM 直連的 usage policy 不歡迎 production；CartoDB 可接受 small-scale production

**URL**: `https://{s}.basemaps.cartocdn.com/light_all/{z}/{x}/{y}.png`

**Alternatives**：
- OSM 直連（policy 風險）
- Stadia Maps（需 API key）
- CartoDB Voyager（更彩色，視覺重）

### D4：date 對 Org timezone 的轉換

```
admin-web                            Server
─────────                            ──────
?date=YYYY-MM-DD                     ?from=&to=
   │                                    ▲
   ▼                                    │
[Org timezone, 例 +08:00]                │
date_start = YYYY-MM-DD T00:00 +08      │
date_end   = YYYY-(MM)-(DD+1) T00 +08   │
   │ → toRfc3339()                      │
   └────────────────────────────────────┘
```

**Why**：admin 想看「2026-03-01 這天工人在哪」，這個「這天」是 Org 的工作日定義（Org timezone 的午夜起算）。後端不知道 Org timezone（API 收 RFC3339 instant），所以 client 負責把 `date=YYYY-MM-DD + Org TZ` 換算成兩個 RFC3339 instant。

實作位置：軌跡頁 fetcher 內。可重用既有 `useOrgTime` composable 或直接寫 `Intl.DateTimeFormat` + offset 計算。

### D5：API 從 LocationListQuery 加 from / to

| Combination | 行為 |
|---|---|
| 只 `before` | 既有 cursor pagination |
| 只 `from` | 篩 `occurred_at_client >= from`，仍走 cursor / limit |
| 只 `to` | 篩 `occurred_at_client < to`，仍走 cursor / limit |
| `from` + `to` | 區間查詢（dashboard 主要用法） |
| `from` + `before` | 都套（dashboard 不會用，但合法） |

**驗證**：沿用 export 的 4 條規則 (parse / `to >= from` / span ≤ 90 / from 不能比 90 天前更老 ⇒ TTL 限制)。

**為什麼不做新 endpoint**：擴既有 endpoint 邊界小、不影響舊呼叫。建立 `GET /locations/by-day` 等於把 client-side 日期換算問題搬到 server 但 server 不知道 Org TZ — 反而更糟。

### D6：xlsx 下載走 `<a href>` 直接觸發

```html
<a :href="exportUrl" target="_blank" rel="noopener" download>匯出 xlsx</a>
```

`exportUrl` 是 `${apiBaseUrl}/checkin/users/:id/locations/export?from=...&to=...`。Cookie auth 自然帶過去（同 origin / SameSite=Lax / credentials 在 navigation 時帶上）。

**Alternatives**：
- `fetch` + `Blob` + `URL.createObjectURL`：可以攔 4xx 錯誤顯示，但跑兩次（fetch + download trigger 用 anchor）
- 手動弄 form POST 到 `/locations/export` 然後 server `Content-Disposition: attachment`：太迂迴

direct anchor 簡單夠用。失敗的話瀏覽器原生 alert 也能讓使用者知道（雖然 UX 沒那麼好）。如果驗證失敗（90 天超界）我們會在 client 先擋掉、不送請求。

### D7：軌跡頁 empty state 不顯示地圖

**Why**：
- Empty 地圖（zoom 出去看全島）是錯誤訊號 — 看起來像「資料還在 loading」或「位置在台中外海」
- 直接「該日無軌跡資料」文字 + 一個「換日期」按鈕（聚焦到 picker）反而清楚
- 載入中 → loading spinner；無資料 → 文字提示；有資料 → 地圖

```
┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐
│ ⏳ 載入軌跡...     │  │ 該日無軌跡資料      │  │ [Leaflet 地圖]     │
│                    │  │                    │  │ polyline + markers │
│                    │  │ [📅 換日期]        │  │                    │
└────────────────────┘  └────────────────────┘  └────────────────────┘
```

### D8：Event markers 從既有 events list 來

軌跡頁需要兩種資料：
1. pings (location-tracking) → polyline
2. events (checkin-events) → markers

events list 已存在（`GET /checkin/users/:id/events` cursor），用 client-side filter 取當日（events 量小、單頁覆蓋）。

如果未來 events 量爆大，再考慮給它加 from/to — 不在這次 scope。

### D9：Org settings toggle 重用既有 `transfer_enabled` pattern

`pages/index.vue` 既有 settings 區塊已有 `transfer_enabled` toggle 與其錯誤訊息。新 `location_tracking_enabled` 完全照抄，只改：
- 顯示文字 (`定位追蹤` vs `轉出 / 轉入`)
- 提示文案（關閉後的說明）
- field 名稱

`useOrgSettings.update({ location_tracking_enabled: target })` 會走過既有 PATCH，伺服器一視同仁 state-lock。錯誤訊息直接重用 `STATE_LOCKED` → `'目前有 App 使用者在班，需先全部下班才能調整此設定'`。

### D10：新 capability `admin-trajectory-dashboard` 的範圍

包含 dashboard 端三件事：
1. 軌跡頁的 product behavior（empty state、polyline、event markers、fit-bounds、date picker）
2. Org settings 的 location_tracking_enabled toggle UX（state-locked error message、與 transfer 一致）
3. xlsx 匯出 flow（modal、date range、下載觸發）

不放：
- API behavior（屬於 `location-tracking`）
- Auth / route protection（屬於 `dashboard-auth`）
- 通用測試 infra（屬於 `admin-web-quality`）

## Risks / Trade-offs

- **happy-dom 環境跑 Leaflet**：Leaflet 在 mount 時呼叫 `getBoundingClientRect`、`L.DomUtil.create`，happy-dom 大部分支援但偶有缺角。**Mitigation**：軌跡頁 component 把 Leaflet init 抽到一個 `initMap` 函式 + `mapReady` ref，測試時可以 mock 或跳過 init 的 path，斷言 component-level state（loading/empty/has-data flag）而不是 Leaflet DOM 細節。
- **CartoDB rate limit / outage**：tiles 載不到 → 地圖一片空白但有 marker / polyline。**Mitigation**：可接受降級。如果未來變成問題，考慮 tile fallback chain。
- **單日 ping 量爆掉**：理論上 8h × 60s = 480 個點，1000/page 一次到位。但若 client 60s throttle 失效（bug）或 admin 拉了個多日 query（誤用），polyline 點數可能上千。**Mitigation**：Leaflet 對 ~5k 點還能流暢繪製；超過再說（不在這次 scope）。
- **xlsx 下載 4xx 沒攔**：`<a href download>` 直接走，server 回 401/403/INVALID_RANGE 時會在瀏覽器顯示原始 JSON。**Mitigation**：client 先做 90 天範圍的格式檢查避免最常見 INVALID_RANGE；其他 401/403 屬於 admin 已登入但操作越權，理論上不會發生。
- **route 結構變動**：把 `pages/checkin/[appUserId].vue` 變成 `pages/checkin/[appUserId]/index.vue` 是 Nuxt 慣例搬遷。route 行為不變（`/checkin/:appUserId` 仍解析到 index.vue），但是個 git rename + import path 可能微調。**Mitigation**：搬完 `pnpm dev` 跑一輪確認 route 解析正確。
- **Date picker locale**：`<input type="date">` 在 zh-TW locale 顯示「年/月/日」，submit value 仍是 `YYYY-MM-DD` ISO。所以對 URL 與 server 都 OK。
- **ROADMAP 條目刪除時機**：archive 後從 ROADMAP 刪。已記在 archive flow 的慣例。
- **vue-leaflet 沒選 — 未來重切心理成本**：如果之後想 Vue 慣用語法（slot 標 marker 等），改裝 wrapper 重寫。但 admin tool 不會大改地圖體驗，這個風險很低。

## Migration Plan

純加值。無 schema migration，無 breaking。

1. API 補 from/to → 部署
2. admin-web 軌跡頁、toggle、匯出 → 部署
3. archive 後從 ROADMAP 刪「下一批 changes」條目

**Rollback**：
- API：revert `LocationListQuery` 與 `list_locations` 改動。`from`/`to` 缺席時退回原 cursor 行為，dashboard 會壞掉但不會打到舊 client（admin-web 是同 deploy 單位）
- admin-web：revert 軌跡頁、移除 toggle、移除 leaflet deps
