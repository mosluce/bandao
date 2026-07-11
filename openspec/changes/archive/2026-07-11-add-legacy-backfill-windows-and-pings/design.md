## Context

舊系統是一個獨立的 MongoDB，`checkin_events` 集合中每筆文件形狀大致如下（已從實際客戶資料截圖確認）：

```js
{
  _id: ObjectId('61684e940090865e17878901'),
  action: '上班',              // '上班' | '下班' | '轉出' | '轉入' | '路徑'
  at: ISODate('2021-10-14T03:36:51.806Z'),
  domain: ObjectId('5599d1c1...'),   // 舊系統的客戶／租戶 ID
  signer: { displayName: '張正芳', username: 'fang' },
  comment: 'office',
  geo: { lat: 22.588..., lng: 120.362... },
  address: '高雄市鳳山區頂庄路',
  addressMeta: { postal_code, country, administrative_area_level_1/3/4, route, street_number },
  createdAt: ISODate(...),
  updatedAt: ISODate(...),
}
```

先前一版（`add-legacy-checkin-backfill`，PR #40，未合併）把「怎麼接舊系統」做成一個通用、可自助設定的產品功能：Org 層級的加密連線設定、admin-web 逐欄位 dot-path 對應表單、動作對應表、Mongo-backed job queue、常駐背景 worker。使用者回饋是「工具太過混亂不易使用」——通用化本身不是問題，但把「一次性資料搬遷」包裝成長期存在的自助產品功能，複雜度不成比例。且該版完全沒有處理 `action: '路徑'`，只把四種打卡動作寫進 `checkin_events`。

這次重新設計把定位收斂為「開發者對已知客戶跑的一次性腳本」，欄位形狀直接寫死在程式碼裡（不做通用 dot-path 對應機制）——因為目前只有一個已知客戶、已知形狀；真的出現第二個形狀不同的客戶時，再視情況決定要不要抽象。

## Goals / Non-Goals

**Goals:**
- 把舊系統的 `checkin_events` 集合資料，依 `action` 分流匯入班到自己的 `checkin_events`（四種打卡動作）與 `location_pings`（路徑）。
- 匯入過程可安全地重複執行（開發者上線切換期間手動多次重跑），不會產生重複資料。
- 不需要任何持久化的連線設定、背景服務或管理介面。

**Non-Goals:**
- 不支援其他形狀的舊系統（沒有通用欄位對應機制）。
- 不做排程自動觸發（cron/systemd timer 等）——本次執行方式是開發者手動重複執行。
- 不做 `location_pings` 的 rotate／封存機制（只移除現行 90 天硬性刪除）。
- 不修正 App／admin-web 上「90 天後自動清除」的文案（留待 rotate 機制定案時一併處理，見 proposal.md 的已知暫時不一致說明）。
- 不做「找不到對應 AppUser」的獨立報表，只在腳本結束時印出摘要計數。

## Decisions

### 1. 欄位形狀寫死在程式碼裡，不做通用對應機制

**選擇**：`api/examples/legacy_backfill.rs` 直接對應上述已知文件形狀（`action`/`at`/`domain`/`signer.username`/`geo.lat`/`geo.lng`/`address`/`comment`）解析成 Rust struct，欄位名稱是常數，不是可設定值。

**替代方案**：沿用前一版的「Org 設定 + dot-path 欄位對應」模式，但只是拿掉 UI，改成讀一個 YAML/JSON 設定檔。

**理由**：目前只有一個已知客戶、已知形狀。欄位對應機制本身不是複雜度的根源，但它預先為「未來可能有形狀不同的客戶」付出的抽象成本，在只有一個客戶的當下無法驗證是否選對了正確的抽象邊界。真的出現第二個客戶時，屆時能看到兩種真實形狀的差異，才是決定「該不該做成設定檔／該用什麼粒度做對應」的好時機。

### 2. `legacy_source_id` + partial unique index 做冪等 upsert

**選擇**：`checkin_events`、`location_pings` 新增 `legacy_source_id: Option<ObjectId>` 欄位，存舊系統文件的 `_id`。建立 partial unique index：

```js
db.checkin_events.createIndex(
  { legacy_source_id: 1 },
  { unique: true, partialFilterExpression: { legacy_source_id: { $exists: true } } }
)
// location_pings 比照
```

寫入時用 `find_one_and_update` + `$setOnInsert` + `upsert: true`，filter 為 `{ legacy_source_id: <舊 _id> }`。

**替代方案**：
- (a) 每次執行前先用時間戳記做「高水位游標」（只查詢上次執行之後的新紀錄）。
- (b) 不做冪等處理，要求開發者自己保證只執行一次。

**理由**：選 (b) 已被使用者明確排除——初期會手動重複執行（例如每 30 分鐘一次）觀察匯入狀況。(a) 高水位游標對「新增」的紀錄有效，但無法處理舊系統對既有紀錄的修改／補登（`updatedAt` 晚於原始 `at`），且游標狀態需要額外持久化在某處（腳本是無狀態的一次性執行，不該自己維護狀態檔）。`legacy_source_id` unique index 讓 MongoDB 自己保證冪等性，腳本本身完全無狀態，重跑語意最簡單：「舊系統現在有什麼，跑完之後班到就有什麼，且不重複」。

### 3. 直接寫 DB，不經過線上 API 的狀態機／順序驗證

**選擇**：腳本用 repository 層直接 upsert 文件，不呼叫 `POST /app/checkin/events` 或任何 handler。匯入完成後，人工重啟一次 API process，讓既有的 `repair_checkin_status_drift`（`api/src/startup.rs`，每次啟動都會跑、idempotent、best-effort）掃描並修正 `checkin_user_status`。

**理由**：`checkin-events` spec 的狀態機／`OUT_OF_ORDER` 順序驗證是為了保護「即時打卡請求」的資料完整性，不是歷史資料匯入的合理限制——舊系統的資料本來就是既成事實，不需要重新驗證是否為合法的即時轉換。`repair_checkin_status_drift` 已經處理「`checkin_events` 是 source of truth，`checkin_user_status` 是可能落後的投影」這個問題，重複利用它，腳本就不需要自己實作狀態推導。

**風險**：如果匯入的歷史事件本身有資料品質問題（例如同一人同一時間有兩筆矛盾動作），`repair_one` 只看「最新一筆事件」決定狀態，不會偵測或報錯這類異常——腳本應該在寫入前後都印出每個 AppUser 匯入的事件數與時間範圍，方便人工肉眼檢查明顯異常，但不做嚴格的合法性驗證。

### 4. `location_pings` 移除 90 天 TTL index

**選擇**：`api/src/db/mod.rs` 移除 `location_pings_ttl` 索引的建立程式碼。

**風險** → **緩解**：
- [風險] TTL 是整個 collection 共用，移除後所有 AppUser 的即時打卡路徑都變成無上限保留，不只是匯入的舊資料 → 這是使用者明確接受的暫時狀態，未來由獨立的 rotate 機制取代目前的 TTL。
- [風險] 集合會無上限成長，可能影響效能／儲存成本 → 本次不處理，記在 Open Questions；rotate 機制上線前是已知的技術債。
- [風險] App／admin-web 上「90 天後自動清除」的文案跟實際行為不一致 → 使用者已確認本次刻意不改文案，留到 rotate 機制定案時一併處理。

### 5. `EventSource::LegacyBackfill` 新變體

**選擇**：`checkin_events` 的 `source` 欄位新增 `LegacyBackfill`，跟既有的 `App`／`AdminForce` 並列。

**理由**：稽核軌跡要誠實反映資料實際來源——這批事件既不是 AppUser 自己打卡送出的，也不是 admin 強制操作，混進 `App` 會讓後續任何依賴 `source` 的統計／稽核邏輯失真。

## Risks / Trade-offs

- [風險] 欄位形狀寫死在程式碼——如果客戶端舊系統形狀後續有變（例如同一個客戶的舊系統本身也在演進），腳本需要跟著改 → 目前只有一個已知客戶、一次性搬遷，可接受；真的需要處理第二種形狀時再抽象。
- [風險] `--since-days` 預設 365 天可能漏掉更久以前的紀錄 → 這是刻意的範圍限制，可由參數覆寫成更長區間做一次性全量匯入。
- [風險] 找不到對應 AppUser 的紀錄被靜默跳過（只算數量，不留清單）→ 如果需要事後追查是哪些 username 對應不到，目前唯一的線索是重新對照舊系統與班到的 AppUser 清單；本次刻意不做報表，之後真的需要再補。
- [Trade-off] 拿掉通用欄位對應機制，換來的是「多一個形狀不同的客戶」時需要改程式碼而非改設定——這是有意識地用「目前只有一個客戶」的現況換取當下的簡單性。

## Migration Plan

1. 部署包含 `EventSource::LegacyBackfill`、`legacy_source_id` 欄位、兩個 partial unique index、TTL index 移除的 API 版本。
2. 開發者在本機（或有網路權限的環境）執行 `cargo run --example legacy_backfill -- --org-id <id> --legacy-uri <mongodb-uri> --legacy-domain <id> --dry-run` 先跑 dry-run，確認匹配到的 AppUser 數量、跳過筆數、各動作類型的筆數是否合理。
3. 移除 `--dry-run` 正式執行；上線切換期間可依需要手動重跑（同一批舊紀錄重跑是安全的 no-op）。
4. 手動重啟一次 API process，讓 `repair_checkin_status_drift` 接上 `checkin_user_status`。
5. 在 admin-web 的既有畫面（`/checkin` 狀態板、`/checkin/[appUserId]/trajectory`）人工抽查幾位 AppUser 的匯入結果。

**Rollback**：`legacy_source_id` 讓匯入的文件可被精確識別與清除（`db.checkin_events.deleteMany({ legacy_source_id: { $exists: true }, org_id: <id> })`，`location_pings` 比照）；TTL index 移除若需要回復，重新建立同名 TTL index 即可（但需先決定所有已無上限保留的既有資料要如何處理）。

## Open Questions

- `location_pings` 拿掉 TTL 後的集合成長／效能影響有多大？未來 rotate 機制的設計時機與具體形式尚未決定。
- 何時、以什麼形式修正 App／admin-web 的「90 天後自動清除」文案？取決於 rotate 機制定案。
- 如果未來真的出現第二個形狀不同的舊系統客戶，欄位對應要不要抽象化、抽象到什麼粒度？留待實際遇到時再決定。
