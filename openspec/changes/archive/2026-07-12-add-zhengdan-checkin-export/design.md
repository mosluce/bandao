## Context

客戶端現有流程：某台舊系統把出勤紀錄吐成固定寬格式的 txt，放進 Windows Server 2016 Datacenter 上的一個資料夾，震旦雲監看該資料夾、讀取後自行匯入並清理。實際格式已從客戶端震旦雲匯出過的真實範例檔逐 byte 反推確認：

```
郭文賓                 20260707064744上班
```

= `姓名`（右補空白到固定 20 字元寬，UTF-8 計字元數不計 byte）+ `YYYYMMDDHHmmss`（14 位數字，+8 時區）+ 事件中文字（`上班`/`下班`，兩者間無分隔符）。整份檔案 UTF-8（無 BOM）、`CRLF` 換行、檔案結尾無多餘換行。客戶確認震旦雲那邊會對匯入內容做 dedupe，所以「每小時吐一次當日整天的完整內容」不會造成重複出勤紀錄。姓名比對（不是員工代號）是震旦雲現行的既有限制，本次不處理同名同姓風險。

這是接在 `add-org-api-tokens` 之後的第二個 change：那個 change 交付了 token 資料模型、CRUD、`ApiTokenAuth` extractor（含 `checkin:read` scope），但**沒有任何 endpoint 真的使用它**。這個 change 是第一個消費者，同時也是第一次把 `ApiTokenAuth` middleware 真的掛到路由上。

## Goals / Non-Goals

**Goals:**
- 提供一個 `checkin:read` scope 保護、**格式通用**的 JSON endpoint，回傳「某個時區窗口下的當日」`clock_in`/`clock_out` 事件——不綁死震旦雲的文字格式，讓以後其他客製整合可以重用同一個 endpoint。
- 「一天」的邊界可由呼叫端透過 `utc_offset` 參數指定，不指定時預設 UTC；伺服器永遠用自己的時鐘計算「現在」，只是用呼叫端給的 offset 換算成哪個日期範圍，不信任呼叫端回報的日期本身。
- 提供一支可以在 Windows Server 2016 Datacenter 上、透過 Task Scheduler 每小時執行的 PowerShell script，呼叫上述 endpoint（帶 `utc_offset=+08:00`），把回應的事件組裝成震旦雲能直接吃的 UTF-8 固定寬文字，寫入指定資料夾，檔名為執行時間戳記。
- 客戶端排程失敗時不寫出殘缺／空白檔案，只記本機 log，等下一次排程自然重試。

**Non-Goals:**
- 不做通用「可插拔匯出格式」系統——這支 PowerShell script 本身、Task Scheduler 部署、目標資料夾規劃仍然是綁震旦雲這個目標客戶的客製部署；只有底層 JSON endpoint 是通用的。
- 不在 Rust 端寫任何震旦雲專屬的文字格式化邏輯（補寬、CRLF、事件中文字組裝）——這些下放到 PowerShell script，見 D4。
- 不處理姓名同名同姓的匹配風險——震旦雲既有限制，不在這次範圍內解決。
- 不做 `transfer_out`/`transfer_in` 的匯出——客戶明確只要上下班。
- 不做失敗重試／告警機制——排程失敗就等下一次整點重試，這次不建置監控或通知管道。
- 不使用 `Org.timezone` 欄位——該欄位目前定義是「display-only」，不影響任何後端查詢邊界計算；時區窗口改由呼叫端透過 `utc_offset` 參數顯式指定（見 D3），跟 `Org.timezone` 完全無關。

## Decisions

### D1. 只接受 API token 認證，不接受 dashboard session
這個 endpoint 掛在新的 router 群組下，middleware 是 `add-org-api-tokens` 交付的 `ApiTokenAuth`（要求 `checkin:read` scope），**不**同時允許 dashboard session cookie。
- **為何**：這個 endpoint 存在的主要理由是給客戶機房的排程腳本用；允許 session cookie 認證只是多一條沒有消費者的路徑，徒增測試與維護面。要在瀏覽器手動驗證時，直接用 `curl -H "Authorization: Bearer <token>"` 測，不需要 session 路徑。
- **替代方案**：雙認證（session OR token）——否決，YAGNI。

### D2. 查詢範圍：org-wide，`clock_in`/`clock_out`，`occurred_at_client` 落在指定窗口
新增 repository 方法（例如 `list_by_org_in_range_for_export`）：依 `org_id` + `occurred_at_client ∈ [day_start, day_end)` + `event_type ∈ {ClockIn, ClockOut}` 查詢，不分頁（一天的量級不需要）。
- **為何用 `occurred_at_client` 而非 `occurred_at_server`**：這是 AppUser 自報的操作時間，是既有 `checkin-events` capability 裡「信任於排序/顯示」的欄位（`occurred_at_server` 只用於 skew 警告與稽核），下游 HR 系統要的是「這個人幾點打卡」，不是「伺服器幾點收到」。
- **為何排除 transfer 事件**：客戶明確表示震旦雲那邊只要上下班；範例檔裡確實也只看到 `上班`/`下班` 兩種字。

### D3. 時間窗口：呼叫端透過 `utc_offset` 參數指定，預設 UTC
新增兩個選填 query 參數：
- `utc_offset`（`+HH:MM` / `-HH:MM` 格式，例如 `+08:00`）：決定「一天」對應到 UTC 的哪個區間。不指定時預設 `+00:00`——也就是「當日」直接對應 UTC 的 00:00～23:59。指定 `+08:00` 時，「當日」對應 UTC 的**前一天 16:00 ～ 當天 15:59:59**。
- `date`（`YYYY-MM-DD`）：指定要查哪一天。不指定時預設「今天」，「今天」的判斷方式是伺服器拿自己當下的 UTC 時鐘、依 `utc_offset` 換算後取日期部分——不是讓呼叫端自己算好日期字串傳進來。
- **為何用顯式參數而不是寫死 +8**：這是通用 export endpoint，不該把單一客戶的時區需求寫死進 Rust——不然下一個客製整合如果時區不同，又要回來改這支程式。呼叫端（這裡是 PowerShell script）自己知道要哪個時區，直接透過參數表達最直接。
- **為何不用 `Org.timezone`**：該欄位的文件明確寫「Display-only — DB stores absolute UTC regardless of this value」，目前沒有任何後端邏輯依賴它做真正的日期切界計算。把它接上這裡的日窗計算等於是第一次讓這個欄位產生真正的行為後果，這是一個獨立於本次需求的決定，不該在一個單客戶客製功能裡順手做掉。
- **為何伺服器算「今天」而不是讓客戶端腳本自己算後用 `date` 參數傳**：客戶機房那台 Windows Server 的系統時鐘/時區設定我們無法保證正確；把「今天是哪一天」的權威判斷留在伺服器（一個已知正確的 UTC 時鐘），排程呼叫只需要傳一個固定不變的 `utc_offset=+08:00`（純粹是「位移多少小時」的靜態參數，不是「相信客戶端說現在是幾號」），消去一整類「客戶端機器時間跑掉」的失敗模式。`date` 參數保留給人工補跑特定過去日期用，正常排程呼叫不帶它。

### D4. 格式化下放到 PowerShell script，API 只回通用 JSON
API 端只回傳結構化 JSON（例如 `{ date, utc_offset, events: [{ app_user_display_name, event_type, occurred_at_client }] }`），姓名補寬、`YYYYMMDDHHmmss` 組字串、事件中文字對應、CRLF 組裝、UTF-8 without BOM 寫檔，全部由 PowerShell script 完成。
- **為何**：震旦雲的固定寬文字格式是「特定廠商的呈現格式」，不是「哪些資料屬於這次匯出」的業務邏輯——後者才該留在後端決定。把格式化下放到 script，讓 API 端維持通用，未來其他客製整合可以重用同一個 JSON endpoint，不用每個廠商格式都在 Rust 開一個新 endpoint。這跟 `admin-web`／Flutter app 各自把 API 回傳的資料格式化成自己畫面要的樣子是同一種性質，不是把業務邏輯塞進客戶端。
- **代價（刻意接受）**：格式細節（補寬、編碼、換行）從有 Rust 型別檢查與單元測試保護，變成活在客戶機房那台機器上的 PowerShell 裡，出錯的偵測與修正週期都變長。最主要的具體風險是 **PowerShell 5.1（Windows Server 2016 內建版本）的 UTF-8 BOM 陷阱**：`Out-File -Encoding utf8` / `Set-Content -Encoding UTF8` 在 PS 5.1 都會自動加 BOM（`utf8NoBOM` 選項要 PowerShell 6+ 才有），必須改用 `[System.IO.File]::WriteAllText($path, $content, (New-Object System.Text.UTF8Encoding($false)))` 才能寫出無 BOM 的檔案。這個寫法會明確寫進 script 與 README，並且「跟真實範例檔逐 byte 比對」在 apply 流程裡是不可省略的驗證步驟（見 tasks 7.x）。
- **姓名補寬用「字元數」不是「byte 數」**：PowerShell 的 `[string]::PadRight(20)` 是以 .NET `Char`（UTF-16 code unit）計數——中文字在 BMP 範圍內剛好都是 1 個 UTF-16 code unit，所以對這批姓名而言 `PadRight(20)` 的行為等同「補到 20 個字元」，跟原本設想的 Rust `chars().count()` 語意一致，不需要額外處理。
- **替代方案**：格式化留在 Rust（原設計）——優點是有測試保護、風險集中在我們能控制的地方；缺點是 endpoint 綁死震旦雲格式、以後每個新客製整合都要回 Rust 加東西。這次選擇下放到 script，用「多一點客戶端維護面」換「API 端保持通用」。

### D5. 檔名：每次執行時間戳記，不覆蓋固定檔名
PowerShell script 每次執行都用「執行當下的本機時間」組出 `yyyyMMddHHmmss.txt` 當檔名寫新檔，不覆蓋前一次的檔案。
- **為何**：從真實範例檔的檔名（`20260712094513.txt`）反推，震旦雲原本的資料來源就是這種「每次一個新檔、讀完自己清掉」的慣例，沿用同樣的慣例風險最低——不用去猜震旦雲監看資料夾的邏輯是「檔名變化觸發」還是「檔案內容變化觸發」。

### D6. 失敗語意：不寫殘缺檔案
PowerShell script 呼叫 API 失敗（非 2xx、逾時、網路錯誤）時，SHALL NOT 在目標資料夾寫入任何檔案；把錯誤訊息附加寫進一個本機 log 檔（例如同目錄下的 `export.log`），結束本次執行，等待下一次 Task Scheduler 觸發。同樣地，如果 API 回應成功但 script 自己組裝格式時發生例外（理論上不該發生，但防禦性處理），也不該寫出部分內容的檔案。
- **為何**：如果失敗時寫一個空檔案或部分內容，震旦雲讀到後可能會把「當日全部出勤紀錄消失」誤判成真的沒人打卡，比「這一小時沒有新檔案」更危險。

## Risks / Trade-offs

- **[Risk] 格式化邏輯移到 PowerShell，失去 Rust 端的測試保護** → Mitigation：README 明確寫死 BOM-safe 的寫檔寫法；apply 流程要求跟真實範例檔逐 byte 比對，不能只靠肉眼或記事本開得起來判斷。
- **[Risk] 排程失敗沒有告警**——如果客戶機房斷線超過數小時，震旦雲會持續讀不到新資料，沒人會主動被通知 → Mitigation：這次先不做，log 檔至少保留排查線索；如果未來這類客製整合變多，值得回頭替 `org-api-tokens` 或這支 script 補一個「N 小時沒有成功執行」的被動告警機制。
- **[Risk] 姓名比對可能同名同姓衝突**——不是這次要解決的問題，但值得在 PowerShell script 的 README 或交付文件裡明確提醒客戶這個既有限制，避免出勤異常時查半天以為是班到這邊的 bug。

## Open Questions

- 客戶機房那台 Windows Server 的目標資料夾路徑、PowerShell 執行原則（Execution Policy）現況、是否已有防毒軟體會擋 Task Scheduler 執行未簽章 script，這些要 apply 前跟客戶 IT 確認一次，可能影響 script 的實際部署步驟（例如要不要用 `-ExecutionPolicy Bypass` 包在排程動作裡，而不是改整台機器的系統原則）。
