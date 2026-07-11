## Why

客戶的舊打卡系統（自建 MongoDB）留有多年歷史紀錄，目前完全沒有辦法進到班到。先前一版做法（`add-legacy-checkin-backfill`，未合併即被否決）把它做成一個通用、可由 admin 在網頁上自助設定連線與逐欄位對應的產品功能（加密連線字串、dot-path 欄位對應表、動作對應表、Mongo-backed job queue、常駐背景 worker），結果是介面太過混亂不易使用。這次重新設計，把它收斂回「開發者對已知客戶跑一次性腳本」的定位，並補上前一版完全沒處理的缺口：舊系統的 `action: '路徑'` 紀錄（GPS 軌跡點）。

## What Changes

- 舊系統紀錄依 `action` 分流匯入兩個既有集合：
  - `'上班' | '下班' | '轉出' | '轉入'` → `checkin_events`（新增 `EventSource::LegacyBackfill` 標示來源，跟 `App`／`AdminForce` 區分開來，稽核紀錄誠實反映資料實際來源）
  - `'路徑'` → `location_pings`
- 兩個目標集合的文件上新增 `legacy_source_id`（存舊系統文件的 `_id`），並建立 partial unique index。匯入採 upsert（`$setOnInsert` + `legacy_source_id` 唯一鍵），同一批舊紀錄重跑任意次都是安全的 no-op——這是刻意設計，因為上線切換期間會由開發者手動重複執行（例如初期每 30 分鐘手動跑一次），不接排程基礎設施。
- 新增一支開發者專用的一次性匯入腳本（`cargo run --example legacy_backfill -- --org-id <id> --legacy-uri <mongodb-uri> --legacy-domain <id> [--since-days 365] [--dry-run]`）：
  - 依 `signer.username` 比對 Org 底下已存在的 AppUser（帳號需 admin 事先手動建立，比對邏輯本身不做模糊媒合）；找不到對應帳號的紀錄直接跳過，執行結束印出跳過筆數摘要。
  - 預設只拉最近 365 天的舊紀錄（`at >= now - 365d`），可用參數覆寫查詢區間。
  - 直接寫入資料庫，不經過線上 API 的狀態機／順序驗證（這是歷史資料匯入，不是即時打卡請求）；匯入完成後由既有的 `repair_checkin_status_drift`（每次 API process 啟動都會跑）自動把 `checkin_user_status` 接上最新事件，腳本本身不需要重算狀態。
  - 不做加密連線字串持久化、不做 admin-web 設定頁、不做背景 worker／job queue——連線資訊只在單次腳本執行的參數／環境變數中存在。
- `location_pings` 拿掉 90 天 TTL index。**BREAKING（產品承諾層面）**：MongoDB TTL 是整個 collection 共用同一份設定，無法只對匯入的舊路徑資料放寬保留期，因此這個改動影響的是所有 AppUser 的即時打卡路徑，不只是本次匯入的歷史資料。拿掉 TTL 之後，資料保留期限變成「無上限，直到未來的 rotate 機制上線」；`admin-web/pages/privacy.vue` 與 App 端定位同意對話框裡「90 天後自動清除」的文案本次刻意不動，留到 rotate 機制定案時再一併修正——這段期間文案與實際保留行為不一致，是已知且刻意接受的暫時狀態。

## Capabilities

### New Capabilities
- `legacy-checkin-backfill`: 開發者一次性、可重複安全執行的腳本，將客戶舊系統的打卡事件與路徑紀錄分流匯入 `checkin_events` 與 `location_pings`。

### Modified Capabilities
- `location-tracking`: 移除 `location_pings` 的 90 天 TTL 保留期要求（原 Requirement「Location pings are persisted with dual timestamps and 90-day server-time TTL」的 TTL 部分不再成立；雙時間戳欄位本身不變）。

## Impact

- **api（Rust）**：
  - `domain::EventSource` 新增 `LegacyBackfill` 變體。
  - `domain::CheckinEvent`、`domain::LocationPing` 新增 `legacy_source_id: Option<ObjectId>` 欄位。
  - `db/mod.rs` 移除 `location_pings_ttl` 索引建立；`checkin_events`、`location_pings` 新增 `legacy_source_id` 的 partial unique index。
  - 新增一支獨立的一次性匯入程式（`api/examples/legacy_backfill.rs`），唯讀連線舊 MongoDB，不透過既有 handler／HTTP API。
  - `startup.rs` 的 `repair_checkin_status_drift` 邏輯不變，直接複用。
- **admin-web / app**：無程式碼變更（同意文案本次不動，見上）。
- **文件／spec**：`openspec/specs/location-tracking/spec.md`、`openspec/specs/org-privacy-policy/spec.md` 需要反映 TTL 移除；隱私政策頁與 App 端文案的實際修正留待未來 rotate 機制的變更處理。
- **Non-Goals（本次不做）**：
  - 不做 admin-web 自助設定介面。
  - 不做背景常駐 worker 或 Mongo-backed job queue。
  - 不支援 MongoDB 以外的舊系統來源。
  - 不做 `location_pings` 的 rotate／封存機制（僅移除現行 90 天硬性刪除，未來另案處理）。
  - 不修正同意文案與隱私政策頁的「90 天」用字（留待 rotate 機制定案）。
  - 不做「找不到對應 AppUser」的獨立報表，只在執行 log 印出摘要計數。
