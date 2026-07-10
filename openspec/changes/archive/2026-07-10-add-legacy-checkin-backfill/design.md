## Context

新客戶的舊打卡系統是一個獨立的 MongoDB，文件形狀類似：

```
{ _id, action: "上班", at: <ISO>, domain: <ObjectId>, signer: { displayName, username },
  comment: "office", createdAt, geo: { lat, lng }, address, addressMeta: {...}, updatedAt, __v }
```

跟我們自己的 `domain::CheckinEvent { org_id, app_user_id, event_type, occurred_at_client, occurred_at_server, source, initiated_by_kind, initiated_by_id, location: { coordinates, accuracy_meters, region_name, manual_label }, reason }` 形狀不同，且沒有任何外部員工 ID——`signer.username` 是唯一能拿來跟我們自己 `AppUser.username` 媒合的鍵。

現有的即時打卡端點（`POST /app/checkin/events`）有嚴格的狀態機（僅 5 種合法轉換）與嚴格遞增的時間戳檢查（`OutOfOrder`），加上每筆同步呼叫地理編碼——這條路不適合拿來重播歷史資料：真實世界的歷史打卡幾乎必然有序列異常（忘記下班、重複打卡等）。

已有三個現成、值得直接複用的模式：
- `ExternalAuthConfig` + `MssqlProvider`（`add-external-db-app-auth`）：每個 Org 存一組外部連線設定，登入時同步連線查詢。這次的 `legacy_backfill` 設定與 provider 是同一種形狀，只是用途不同（撈歷史紀錄而非驗證密碼）且後端是 MongoDB（可直接用現有 `mongodb` crate，不需要像 MSSQL 那樣引入新 driver）。
- `startup::repair_one`（`checkin-events`）：已經是「用某位 AppUser 最新一筆事件回推 `CheckinUserStatus`」的現成邏輯，且天生就是逐一 AppUser 操作——直接抽出來複用，不需要為回填另寫狀態推導規則。
- `checkin_user_status` 的條件式 `find_one_and_update`（「match 條件成立才更新，否則視為被搶先」當原子性保證）：這正是任何 job queue「原子性認領一筆工作」所需要的機制，這個 codebase 已經很熟悉這個慣例，只是還沒包裝成正式的佇列。

repo 裡沒有 Redis 或任何 queue 相關套件（`docker-compose.yml` 只有 `mongodb`），這決定了背景任務的執行方式該怎麼設計（見 D6）。

## Goals / Non-Goals

**Goals:**
- 讓每個 Org 能設定「怎麼連舊系統、怎麼把舊文件轉成我們的事件形狀」，設定值存在 Org 身上，不寫死進程式碼。
- 員工第一次登入時自動觸發回填，登入本身的延遲與行為不受影響。
- 沿用既有的狀態回推邏輯，不重新發明。
- admin 存設定前能先預覽轉換結果，不需要等員工登入才發現設定錯了。

**Non-Goals:**
- 不支援 MongoDB 以外的舊系統（架構上用 Org 設定保留擴充空間，這次只做 Mongo-to-Mongo）。
- 不支援上傳客戶/自訂執行程式碼（見 Risks，部署環境不支撐安全隔離執行）。
- 不引入 Redis 或任何外部 queue 系統（見 D6，改用 Mongo 自己的佇列）。
- 不做批次 dry-run 報告、不做「未對應動作值」的進階處理、不做時間欄位校正、不做「非當日重置離班」規則（見下方個別決策）。

## Decisions

### D1. 身份媒合鍵是 `username`，由 admin 建帳號時人工指定
不對歷史資料做「猜測誰是誰」的演算法。`AppUser.username` 本身就是媒合鍵——admin 建立 AppUser 帳號時（沿用既有 `POST /app-users`），就是在確認「這個帳號對應舊系統的哪個 `signer.username`」。
- **為何**：舊資料的 `displayName` 有同名同姓風險（沒有其他外部 ID 可區分），`username` 至少在建帳號當下是人工一次性確認過的，不會被演算法搞混。

### D2. bandao 自己連線查詢，不做上傳/執行客戶腳本
Provider 直接用既有 `mongodb` crate 連第二個唯讀連線，依 Org 設定的欄位對應轉換文件。
- **為何**：曾考慮讓客戶/ops 上傳一個「連線+轉換」腳本以支援任意 DB driver，但目前部署（Zeabur，單一 Rust binary container）沒有能安全隔離、跑任意程式碼（含其自身相依套件）的執行環境——沒有 job queue、沒有 container orchestration、`api` 本身也沒有任何 subprocess/exec 先例。這件事的投入會比整個功能都大。改用「聲明式設定值」——連線字串 + 欄位對應 + 動作對應表——由 bandao 自己可信的程式碼執行，滿足「不寫死進程式碼」的核心訴求，同時不需要新的執行環境。
- **代價**：目前只支援 MongoDB-shaped 來源；下一個客戶如果是 MySQL/REST API，需要另外設計（見 Non-Goals）。

### D3. 設定值形狀：`Org.settings.legacy_backfill`
```
{
  connection_string_encrypted: String,   // mongodb://... ，比照連線密碼用既有對稱加密存放
  database: String,
  collection: String,
  identity_field: String,       // dot-path，例如 "signer.username"
  timestamp_field: String,      // 例如 "at"
  lat_field: String,            // 例如 "geo.lat"
  lng_field: String,            // 例如 "geo.lng"
  region_name_field: Option<String>,   // 例如 "address"，選填
  manual_label_field: Option<String>,  // 例如 "comment"，選填
  action_field: String,         // 例如 "action"
  action_map: { <原始值>: CheckinEventType },  // 例如 {"上班":"clock_in", ...}
}
```
- **為何加密連線字串**：連線字串內含帳密，跟 `external_auth.password_encrypted` 同等敏感，比照既有對稱加密機制（`SecretBox`），不落明文、不回吐。
- **為何用 dot-path 字串**：巢狀欄位（`signer.username`、`geo.lat`）用簡單的 dot-path 表示已足夠這次需求，不需要更複雜的查詢語言。

### D4. 時間戳與地理資訊：直接信任、不校正、不重打
`occurred_at_client` 直接採用 `timestamp_field` 的值（本次客戶資料已確認可信任，不需要跟其他欄位交叉核對）；`region_name` 直接採用 `region_name_field`（若有設定），不呼叫既有的 `ReverseGeocoder`。
- **為何**：既有地理編碼是為「即時打卡、無既有地址」設計；歷史資料本身通常已經有地址資訊，重打既浪費又沒有必要。時間戳校正這次客戶不需要，列為 Non-Goal，避免為假設情境過度設計。

### D5. 未對應的動作值：跳過，計數，不做進階處理
`action_map` 沒有列出的原始動作字串，該筆事件直接跳過不匯入；跳過的筆數會記錄下來（供之後查看），但不阻擋其餘資料匯入，也不特別建 UI 呈現。
- **為何**：這次客戶的動作詞彙已經在 admin-web 設定時列出對應表；真的遇到「跳過筆數異常多」的情況再回頭補進階處理，不預先設計用不到的東西。

### D6. 觸發機制：Mongo 自己的輕量 job queue，不是 fire-and-forget、不引入 Redis
新增 `legacy_backfill_jobs` collection：`{ _id, org_id, app_user_id, status: pending|active|done|failed, attempts, next_attempt_at, locked_at, last_error, created_at, updated_at }`，`app_user_id` 建唯一索引避免重複排入。

流程：
- **登入時（enqueue）**：`POST /app/auth/login` 核心邏輯完成之後，若 `Org.legacy_backfill` 已設定且該 `AppUser.legacy_backfill_done_at` 為空，做一次快速的 `upsert`（`status: pending`，若已存在則不重複建立）。這是一次單純的 Mongo 寫入（ms 等級），不需要 `tokio::spawn`，登入延遲幾乎不受影響。
- **背景 worker（處理）**：開機時啟動一個常駐的背景迴圈（比照 `repair_checkin_status_drift` 的啟動方式，但這次是持續執行、不是跑一次就結束）。每隔固定間隔，用條件式 `find_one_and_update`（match `status: pending && next_attempt_at <= now`，改成 `status: active`）原子性地認領一筆工作——跟 `checkin_user_status` 轉換用的是同一個慣例。認領後執行實際回填（連線 → 查詢 → 轉換 → 序列檢查 D7 → bulk insert → 狀態回推 D8），成功則設 `status: done` + `AppUser.legacy_backfill_done_at`；失敗則 `attempts += 1`、依 backoff 算出 `next_attempt_at`、退回 `pending`（見 D9 的上限規則）。

- **為何不用 fire-and-forget（`tokio::spawn`)**：process 重啟/redeploy 會讓進行中的工作直接消失、無法恢復；也沒有任何地方能看到「現在有幾個 pending/failed」。
- **為何不引入 Redis**：這次的量級（每個 Org 的員工數、一次性觸發）遠用不到成熟 queue 系統的規模；Mongo 已經是唯一的資料庫依賴，且 codebase 已經有現成的原子性慣例可以直接複用，不需要多背一個要維運的服務。
- **為何不需要新的部署單元**：worker 就是同一個 `api` process 裡的一個常駐迴圈，跟現有的 boot-time repair 任務同源，只是這次不會結束。

### D9. 重試策略：capped exponential backoff，超過上限標記 failed
`attempts` 達到上限（如 5 次）後，`status` 設為 `failed`，不再自動重試，需要人工介入（查看 `last_error`，決定要不要手動把 job 重設回 `pending`）。上限之前，每次失敗依 backoff（例如 1 分鐘、5 分鐘、30 分鐘…）計算 `next_attempt_at`，讓 worker 之後再次嘗試——跟使用者登入的時機完全脫鉤，不再是「靠下次登入才重試」。
- **為何要設上限**：避免一個永遠連不上的舊系統讓 job 無限期停留在 pending、每個 backoff 週期都白工。`failed` 是一個明確的終止狀態，讓人知道這裡需要介入，而不是無聲無息地一直重試。

### D7. 序列異常：記錄，不阻擋
歷史事件可能違反現行狀態機的合法轉換（例如連續兩次 ClockIn）。回填時**繞過**即時端點的狀態機驗證，直接依轉換後的事件寫入；異常的轉換記錄下來（做為之後除錯線索），但不因此整批失敗或跳過該員工其餘資料。
- **為何**：這是離線／背景寫入，不是即時使用者操作；歷史資料的目的是留存紀錄，不需要每筆都通過即時系統的嚴格業務規則。
- **替代方案**：整批驗證失敗就整批不寫入——太脆弱，一個異常就讓整個人的歷史都進不來，否決。

### D8. 狀態回推：複用既有 `repair_one`，不寫新邏輯
回填寫入事件後，呼叫從 `startup.rs` 抽出（改為可獨立呼叫、不只在開機時跑一次）的 `repair_one(db, app_user_id, org_id)` 邏輯，用該員工「最新一筆事件」回推 `checkin_user_status`。
- **為何**：這個邏輯天生就是「給一個 AppUser id，用其最新事件推導應有狀態」，回填後的情境（有事件、沒有狀態列）正好落在它已經處理過的分支（`(None, Some(latest))`）。不需要另外實作「最後事件非當日要重置離班」這類規則——已明確列為 Non-Goal。

### D9. 失敗即自動重試（透過旗標語意，不建專門的重試機制）
`legacy_backfill_done_at` 只有在整個回填流程成功完成後才會被設定。若連線失敗、查詢出錯等導致流程中途失敗，旗標維持空值——**下一次這個人登入時會自然再次觸發**，不需要額外的排程或重試佇列。
- **為何**：這個語意本身就隱含了重試機制，不需要另外設計。
- **代價**：如果某個員工很久才登入第二次，失敗會拖到那時候才重試——可接受，因為回填不是阻斷性功能。

### D10. Admin-web 設定子頁 + 「測試連線＋預覽」
新增 `pages/settings/legacy-backfill.vue`：連線設定表單 + 欄位對應表單 + 動作對應表（可新增列）；一顆「測試連線＋預覽」按鈕，呼叫唯讀的 `POST /orgs/me/legacy-backfill/preview`，用目前（可能尚未存檔）的設定值實際連線撈幾筆樣本、套用轉換規則，顯示轉換後的結果——不寫入任何東西。
- **為何**：這是設定階段的信心檢查，仿照 external-db-auth 的 `test-login`（dry-run 但不寫入）——讓 admin 存檔前就能發現欄位名稱打錯之類的低級錯誤,不用等員工登入才踩雷。

### D11. Admin-web 加一個唯讀的 job 狀態頁面
新增 `GET /orgs/me/legacy-backfill/jobs`（admin-only，唯讀）：回傳該 Org 底下 `legacy_backfill_jobs` 的清單（狀態、對應的 AppUser、`attempts`、`last_error`、時間戳）。admin-web 設定子頁下方加一個簡單的列表區塊呈現，讓管理員知道有哪些人回填失敗、原因是什麼。
- **為何**：D9 引入了「失敗會停在 `failed` 需要人工介入」的狀態，若完全沒有地方能看到,這個狀態就形同虛設——admin 得知道要去哪裡查。
- **範圍克制**：這次只做唯讀查看，不做「在 UI 上手動重試」的操作——需要重試時先直接改 Mongo（`status: failed → pending`, `attempts: 0`），UI 上的手動重試按鈕留待真的有需要再加。

## Risks / Trade-offs

- **只支援 MongoDB 來源** → 下一個客戶若是不同 DB，需要新增另一種 provider 實作（架構上是加一個新 driver，不是重新設計）。
- **不支援自訂腳本，彈性受限於「連線字串 + 欄位對應」這個聲明式模型** → 真的遇到需要條件邏輯的個案（像 AM/PM 校正）時，這次選擇不處理；未來若頻繁出現，可能要重新評估是否投資執行環境基礎設施。
- **失敗仍是被動發現，沒有主動通知**（admin 得自己去看 job 狀態頁，不會收到 email/通知）→ 這次量級可接受，未來若客戶數變多可能要加主動告警。
- **worker 迴圈是單一 process 內的常駐任務** → 若 api 有多個 replica 同時跑，需要確認條件式 `find_one_and_update` 的原子性足以避免多個 worker 重複認領同一筆 job（同一個慣例已經在 `checkin_user_status` 的併發場景下驗證過，可直接沿用同樣的信心）。
- **繞過即時端點的狀態機驗證** → 歷史資料的完整性依賴欄位對應設定是否正確，「測試連線＋預覽」是這裡唯一的把關手段。
- **舊系統連線可達性** → 與 external-db-auth 相同的已知限制：prod 環境不一定連得到客戶內網的舊 MongoDB。

## Migration Plan

1. Schema 向後相容：`Org.settings.legacy_backfill`、`AppUser.legacy_backfill_done_at` 都是新增的可選欄位，缺省不影響任何現有行為；新增的 `legacy_backfill_jobs` collection 是全新集合，不影響既有資料。
2. 新客戶 onboarding 流程：admin 在設定頁填入舊系統連線與欄位對應 → 測試預覽確認無誤 → 存檔 → 之後為該客戶建立的 AppUser 帳號，只要 username 對應到舊系統的 `signer.username`，第一次登入就會自動回填。
3. Rollback：清空 `Org.settings.legacy_backfill` 即可讓該 Org 完全不觸發任何回填行為；已回填的 `checkin_events` 不會被自動撤銷（歷史資料寫入是加法操作，不做自動反向遷移）。

## Open Questions

- （目前討論已收斂，暫無待決問題。）
