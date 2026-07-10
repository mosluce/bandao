## Why

`legacy-checkin-backfill` 上線後的設定頁要求管理員直接手打舊系統文件的巢狀 dot-path（例如 `signer.username`、`geo.lat`）——等於要求對方先懂 Mongo 文件結構，容易打錯、也難以判斷像 `at`/`createdAt`/`updatedAt` 這種相似欄位該選哪個。管理員多半只知道連線資訊（連線字串/DB/集合），並不清楚文件實際長怎樣。

## What Changes

- 設定頁流程調整：填完連線資訊（連線字串/DB/集合，選填一個原始 Mongo JSON query 讓採樣更集中）後，先「採樣」——連進舊系統撈幾筆原始文件（不套用任何欄位對應），再從樣本文件攤平出的欄位路徑，用拖放的方式指定到「身份／時間／緯度／經度／地址／備註／動作」欄位。
- 新增一個輕量採樣端點，只需要連線資訊（+ 選填 query），不要求任何欄位對應設定就能跑；跟現有 `preview`（要求完整欄位對應 + `test_username`，驗證實際回填結果）分工不同、互不取代。
- 欄位對應輸入從「只能打字」改成「拖放區 + 文字輸入框並存」——拖進去自動填入,文字框仍可手動輸入/修改，覆蓋樣本沒撈到的稀疏欄位。
- 動作對應表（原始動作字串 → event_type）維持現狀不動，這次不處理。

## Capabilities

### New Capabilities
（無）

### Modified Capabilities
- `legacy-checkin-backfill`: 新增「admin 可採樣舊系統原始文件（不套用欄位對應）」的需求；設定頁的欄位對應輸入方式從純文字改為採樣後可拖放指定。

## Impact

- **api（Rust）**：`handlers/legacy_backfill.rs` 新增 `POST /orgs/me/legacy-backfill/sample`（admin-only）：接受 `connection_string?`（留空＝用已存的，比照現有慣例）、`database`、`collection`、選填的原始 Mongo JSON `query`、`limit`；不呼叫 `validate_config`，直接 `collection.find(query.unwrap_or_default()).limit(N)`，回傳原始文件（BSON→JSON，未套用任何欄位對應）。
- **admin-web（Nuxt）**：`pages/settings/legacy-backfill.vue` 新增「採樣」按鈕與選填 query 輸入框；前端攤平樣本文件（多筆文件路徑取聯集）成可拖放的 chips（顯示 dot-path + 示意值）；身份/時間/緯度/經度/地址/備註/動作欄位改成拖放區+文字輸入框並存。`useLegacyBackfill.ts` 新增 `sample()` composable 方法；`types/api.ts` 新增對應型別。
- **Non-Goals（本次不做）**：
  - 動作對應表（action_map）的 UX 不變——不做「從樣本 distinct 動作值自動列候選列」這件事，留待未來。
  - 不處理舊系統資料裡「路徑」（GPS 軌跡點）這類非打卡事件的特殊匯入——現有「未列在對應表就跳過並計數」的行為已經正確涵蓋這個情況，且 `location_pings` 有 90 天 TTL，回填歷史軌跡點本來就會被清掉，這次不解這個問題。
  - 不做「樣本文件已指定給哪個欄位」的視覺標記/防重複拖放——單純把 dot-path 字串填進文字框，沒有更多狀態追蹤。
