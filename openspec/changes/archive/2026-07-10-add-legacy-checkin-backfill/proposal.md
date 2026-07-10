## Why

一個要導入班到的客戶，舊有的打卡系統（自建 MongoDB）留有多年的歷史打卡紀錄。這批歷史資料目前完全沒有辦法進到班到——沒有匯入工具，也沒有任何機制能把舊系統的紀錄接到新系統的員工帳號上。

舊系統的資料形狀跟我們自己的 `checkin_events` 差異不小（欄位名稱不同、動作字串是中文自由詞彙、沒有外部員工 ID，只有 `signer.username` 可以當識別鍵），而且往後可能有其他客戶帶著完全不同形狀的舊系統資料進來（不同的 DB、不同的欄位、不同的動作詞彙）。因此這次不只是要解決「這一個客戶」的匯入，還要讓「怎麼接舊系統」這件事本身是可設定的，不要把任何一個客戶的具體轉換規則寫死進程式碼。

## What Changes

- 每個 Org 可在 `settings.legacy_backfill` 設定一組**舊系統連線 + 欄位對應**：MongoDB 連線字串（加密存放）、資料庫、集合、身份識別欄位（dot-path）、發生時間欄位、經緯度欄位、選填的地址/備註欄位，以及一張「原始動作字串 → event_type」的對應表。
- AppUser 新增一個一次性旗標 `legacy_backfill_done_at`。當這個 Org 有設定 `legacy_backfill`、且 AppUser 這個旗標仍是空的，**第一次登入成功後**（response 已回傳、不影響登入延遲），排入一筆背景回填工作（Mongo 自己的輕量 job queue，非 fire-and-forget）：由一個常駐的背景 worker 認領後，連進舊 MongoDB、用身份識別欄位查出這個人的所有歷史紀錄、依欄位對應與動作對應表轉換、寫入 `checkin_events`，最後回推這個人目前的 `checkin_user_status`（複用既有的 startup 狀態回推邏輯）。失敗會依 backoff 自動重試，達到上限後標記為 `failed`，需要人工介入。
- 未對應的動作字串**直接跳過不匯入**（先不做進階處理，留待未來真的遇到需要時再調整）。
- 發生時間直接採用舊系統欄位值，不做時區/AM-PM 校正；地理資訊直接沿用舊系統既有的地址欄位，不重新呼叫地理編碼服務。
- admin-web 新增一個設定子頁：連線設定 + 欄位對應表單 + 動作對應表，並提供「測試連線＋預覽」——用目前的設定實際連線撈幾筆樣本資料、套用轉換規則顯示結果，但不寫入任何東西，讓管理員在存檔前確認設定是否正確。
- 身份媒合鍵是 `username`：admin 幫員工建立 AppUser 帳號時，就是在人工確認「這個帳號 = 舊系統裡的哪個 `signer.username`」，不需要演算法去猜測歷史資料裡哪些紀錄屬於同一人。

## Capabilities

### New Capabilities
- `legacy-checkin-backfill`: 每個 Org 的舊系統連線與欄位對應設定、admin-web 設定與預覽介面、AppUser 首次登入排入的背景回填 job queue（含 capped retry）、回填後的狀態回推、admin 唯讀的 job 狀態頁面。

### Modified Capabilities
- `org-tenancy`: `Org.settings` 新增可選的 `legacy_backfill` 子文件。
- `app-user-mgmt`: `AppUser` 新增一次性 `legacy_backfill_done_at` 欄位；登入流程在回應之後排入一筆背景回填 job（不影響登入本身的行為與延遲）。

## Impact

- **api（Rust）**：
  - `domain::Org` 新增 `legacy_backfill()` 存取器（比照 `external_auth()`）；新增 `LegacyBackfillConfig` 型別（連線字串加密存放、DB/collection、欄位對應、`action_map`）。
  - `domain::AppUser` 新增 `legacy_backfill_done_at: Option<DateTime>`。
  - 新增一個 provider（直接用既有 `mongodb` crate 連第二個 MongoDB，唯讀），依欄位對應把原始文件轉成 `CheckinEvent` 形狀；`action_map` 沒對應到的值計數後跳過。
  - 新增 `legacy_backfill_jobs` collection（`pending/active/done/failed` 狀態、`attempts`、`next_attempt_at` 等），沿用 `checkin_user_status` 已有的條件式 `find_one_and_update` 慣例做原子性認領——不引入 Redis 或任何外部 queue 系統。
  - `app_auth::login` 在核心登入邏輯後（`app_sessions` 已建立、response 已組好）排入一筆 `pending` job（單純的一次 Mongo 寫入，非 `tokio::spawn`）。
  - 新增一個開機啟動的常駐背景 worker 迴圈（比照現有 `repair_checkin_status_drift` 的啟動方式，但這次持續執行）——**這是這個 codebase 第一次出現長駐背景迴圈**，值得留意。
  - 複用（抽出可獨立呼叫的版本）`startup.rs` 現有的 `repair_one` 邏輯，做回填後的狀態回推——不寫新的狀態推導規則。
  - 新增 `POST /orgs/me/legacy-backfill`（admin-only，存設定；比照 external-auth 用 `POST` 而非 `PUT`——CORS `allow_methods` 沒有 `PUT`）、`POST /orgs/me/legacy-backfill/preview`（admin-only，唯讀，測試連線＋樣本轉換預覽，仿照 external-db-auth 的 `test-login` dry-run 端點）、`GET /orgs/me/legacy-backfill/jobs`（admin-only，唯讀，job 狀態清單）。
- **admin-web（Nuxt）**：新增設定子頁（連線 + 欄位對應 + 動作對應表 + 測試預覽按鈕 + job 狀態清單）；對應的 API 型別與 composable。
- **app（Flutter）**：無變更。
- **Non-Goals（本次不做）**：
  - 不做批次 dry-run 報告（改用設定階段的「測試連線＋預覽」取代）。
  - 不處理「舊系統時間欄位需要 AM/PM 校正」這類個案怪癖——這次的客戶資料不需要，未來真的遇到再議。
  - 不做「最後一筆歷史事件非匯入當天要重置離班」的規則——直接沿用既有 `repair_one` 對最新事件的推導結果。
  - 不支援上傳客戶/自訂 script 執行——目前部署（Zeabur，單一 Rust binary）沒有能安全隔離任意程式碼執行的基礎設施；所有連線與轉換邏輯都由 bandao 自己的程式碼依設定值執行。
  - 不支援 MongoDB 以外的舊系統 driver（架構上用「Org 設定 + 欄位對應」保留擴充空間，但這次只實作 MongoDB）。
  - 不引入 Redis 或任何外部 queue 系統——重試/排程機制完全建立在既有的 MongoDB 之上。
  - Job 狀態頁面本次只做唯讀查看，不做「UI 上手動重試 failed job」的操作。
