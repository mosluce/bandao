## Why

單一客戶（下稱該 Org）現行是靠另一套 HR 系統「震旦雲」讀取本機資料夾裡的純文字檔來匯入出勤紀錄。該客戶的實體出勤資料來源正逐步換成班到，需要讓班到定期把當日的上/下班紀錄，以震旦雲能吃的固定格式吐成 txt，放到客戶自己一台 Windows Server 2016 Datacenter 機房裡的指定資料夾，讓震旦雲照原本的方式繼續匯入，不需要客戶重新導入震旦雲那一側。

格式已經拿到客戶端震旦雲實際匯出過的真實範例檔並逐 byte 分析確認（不是憑記事本能不能開就用猜的）。

## What Changes

- 新增 `GET /orgs/me/checkin/events/export` endpoint：回傳「某個日期窗口」該 Org 所有 AppUser 的 `clock_in`／`clock_out` 事件（不含 `transfer_out`／`transfer_in`），格式是**通用 JSON**，不綁死任何特定廠商格式。
  - 支援 `date`（`YYYY-MM-DD`，選填）與 `utc_offset`（`+HH:MM`/`-HH:MM`，選填）兩個 query 參數：`utc_offset` 決定「一天」的邊界怎麼切——例如 `+08:00` 對應到 UTC 的 16:00～隔日 15:59:59；不指定時預設 `+00:00`（UTC 當日 00:00～23:59）。`date` 不指定時預設「今天」，「今天」用伺服器自己的 UTC 時鐘換算成 `utc_offset` 所指定的時區來判斷，不信任呼叫端自己回報的日期。
  - 認證走 `add-org-api-tokens` 剛建好的機制：只接受帶 `checkin:read` scope 的 API token，**不接受**現有的 dashboard session cookie（這是機器對機器的 endpoint，不是給瀏覽器用的）。
- 新增一支 PowerShell script（放在新的 `integrations/zhengdan-checkin-export/` 目錄），部署在客戶的 Windows Server 2016 Datacenter 上，透過 Windows 工作排程器（Task Scheduler）**每小時**呼叫一次上述 endpoint（帶 `utc_offset=+08:00`），把回應的 JSON 事件**在 script 端組裝**成震旦雲要的固定寬 UTF-8 文字格式，寫入本機指定資料夾，檔名為執行當下的時間戳記（`yyyyMMddHHmmss.txt`，比照震旦雲原本自己吐檔的命名慣例）。
- **格式化的責任邊界**：API 端只決定「哪些事件、屬於哪一天、排除 transfer」這類業務邏輯與權限；「怎麼把這些事件轉成震旦雲那個特定廠商吃的固定寬文字」是 script 端的責任，不寫進 Rust。這樣 export API 本身是通用的——以後如果有第二個客戶要串別的廠商、別的格式，可以直接重用同一個 JSON endpoint，換一支新 script 組裝即可，不用再回 Rust 加一個新的格式化 endpoint。
- PowerShell script 本身、Task Scheduler 排程、目標資料夾這些操作面設定，仍然是**單一客戶的客製部署**——這點沒有變。

## Capabilities

### New Capabilities
- `checkin-export-zhengdan`：通用的當日 clock_in/clock_out JSON export endpoint（含可設定時區窗口）、API token scope 授權，以及消費這個 endpoint 的震旦雲專屬 PowerShell 排程工具（含其固定寬格式的組裝規則）。

### Modified Capabilities
（無——`checkin:read` scope 已經是 `add-org-api-tokens` 交付內容的一部分，這裡只是第一個實際要求它的 endpoint，不需要再對 `org-api-tokens` 的 requirement 做任何變更。若 apply 時發現 `add-org-api-tokens` 最終沒有把 `checkin:read` 納入初始 scope 清單，才需要回頭補一個 delta。）

## Impact

- **api/（Rust）**：新增 `handlers::checkin_export` handler，回傳通用 JSON；新增 `db::checkin_events` 的日窗查詢方法（org-wide、依 `utc_offset` 換算的時間範圍、限定 `clock_in`/`clock_out`）；`handlers/mod.rs` 新增一個以 `ApiTokenAuth` 為 middleware 的新 router 群組（首次真正掛載 `add-org-api-tokens` 交付的 extractor）。**這次不在 Rust 端寫震旦雲的文字格式化邏輯**——那是 script 的責任。
- **新增目錄**：`integrations/zhengdan-checkin-export/`——PowerShell script（讀本機設定檔取得 API base URL / token / 目標資料夾路徑，呼叫 JSON endpoint 後自行組裝震旦雲格式並寫檔）＋ README（Task Scheduler 設定步驟、Windows Server 2016 Datacenter 的 PowerShell 執行原則注意事項、**PowerShell 5.1 的 UTF-8 BOM 陷阱與正確寫法**）。這個目錄不屬於 `api/` / `admin-web/` / `app/` 任何現有模組，是純營運工具，性質類似 `infra/mongo-host/`。
- **admin-web / app/**：不受影響。
- **已知限制／風險**：
  - 客戶端排程呼叫失敗（網路中斷、token 被誤停用等）目前設計為「這次不寫檔、等下一次排程」，沒有失敗重試或告警機制——如果客戶機房斷線超過一小時以上，會有一段時間震旦雲讀不到新資料，這次不處理告警，只記錄本機 log。
  - 格式化邏輯移到 PowerShell 端之後，**編碼／換行／補寬正確性不再有 Rust 端的型別檢查與單元測試保護**，改成要跟真實範例檔逐 byte 比對驗證（見 tasks），且 PowerShell 5.1 對 UTF-8 without BOM 的處理不直覺，是這個設計選擇明確要付出的代價。
