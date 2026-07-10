## Context

`legacy-checkin-backfill`（已上線）的設定頁（`admin-web/pages/settings/legacy-backfill.vue`）要求管理員在存檔前手動打出舊系統文件的 dot-path（`identity_field`、`timestamp_field`、`lat_field`、`lng_field`、選填的 `region_name_field`/`manual_label_field`、`action_field`），而現有的 `POST /orgs/me/legacy-backfill/preview` 端點又反過來要求這些欄位路徑都先填好、且帶一個 `test_username` 才能連線試跑——兩者合起來等於「先盲打路徑，再驗證路徑」，管理員完全沒有機會在填之前先看一眼舊系統文件實際長怎樣。

後端 `services/legacy_backfill/provider.rs` 已經有 `get_by_path`（給定 dot-path 讀值）等工具，但沒有反向操作（列舉一份文件裡所有的 dot-path）。`admin-web` 目前也沒有任何 drag-and-drop 的既有寫法或套件——這會是這個 codebase 第一次出現拖放 UI。

## Goals / Non-Goals

**Goals:**
- 让管理員在填欄位對應之前，先用連線資訊實際採樣舊系統文件，看到真實欄位與示意值。
- 讓「身份/時間/緯度/經度/地址/備註/動作」欄位可以用拖放的方式從樣本欄位指定，同時保留文字輸入框作為手動輸入/覆蓋的管道。
- 採樣支援一個選填的原始 Mongo JSON query，讓管理員能把樣本集中在已知的某個人/某種紀錄上，而不是隨機撈到不相關的文件。

**Non-Goals:**
- 不處理動作對應表（`action_map`）的 UX——不做「從樣本 distinct 動作值自動列候選列」，維持現有純手動輸入的表格。
- 不新增「這個樣本欄位已經被指定給哪個目標欄位」的視覺標記或防重複拖放邏輯——拖放單純是「把 dot-path 字串複製進文字輸入框」的捷徑。
- 不處理舊系統資料中非打卡事件（例如「路徑」GPS 軌跡點）的匯入——現有「動作對應表沒列到的值就跳過並計數」已經正確涵蓋，且 `location_pings` 集合有 90 天 TTL，回填歷史軌跡點本來就會被清掉，不在這次範圍內。
- 不做採樣結果的持久化/快取——每次「採樣」都是即時連線撈取，不寫入任何資料庫。

## Decisions

### D1: 新增獨立的採樣端點，不共用 `preview`

新增 `POST /orgs/me/legacy-backfill/sample`，語意上與 `preview` 明確分工：

| | `sample`（新） | `preview`（既有） |
|---|---|---|
| 輸入需求 | 連線字串/DB/集合 + 選填 query | 完整欄位對應 + `test_username` |
| 是否套用欄位對應 | 否——回傳原始文件 | 是——回傳映射後的 `CheckinEvent` 樣子 |
| 目的 | 驅動拖放 UI（讓管理員先看文件長相） | 驗證「這組完整設定套用在某個人身上」的結果 |
| 呼叫時機 | 填完連線資訊、欄位對應之前 | 欄位對應填好之後的最終確認 |

不把兩者合併是因為職責不同：`sample` 存在的理由就是「還不知道欄位對應是什麼」，如果沿用 `preview` 的 `validate_config` 前置檢查，會反過來要求管理員先打好欄位路徑才能採樣——正是這次要解決的問題。

**Alternative considered**：讓 `preview` 的欄位對應參數全部變成選填、內部依「有沒有填」分岔成兩種行為。拒絕——會讓 `PreviewRequest`/`PreviewResponse` 的欄位語意變得模糊（一個端點兩種輸出形狀），不如兩個小端點各自單純。

### D2: 採樣查詢用原始 Mongo JSON filter，而非拆成欄位/值兩個輸入

`sample` 的 `query` 參數是一段管理員直接貼上的原始 Mongo query 文件（JSON 字串），例如 `{"signer.username": "test_user"}`，後端解析後直接當 `collection.find(query)` 的 filter。

理由：管理員本來就直接持有整條 Mongo 連線字串（既有設計早就把「管理員完全信任、能直接操作這個舊資料庫」當作既定信任邊界——見 `legacy-checkin-backfill` 的 non-goal「不做沙盒隔離」），讓他們多填一段查詢條件不是新增信任邊界，只是同一等級的存取多開一個入口。拆成「欄位路徑 + 值」的簡化輸入只能做單一等值查詢，管理員如果需要更複雜的條件（例如時間範圍、`$or`）就完全做不到；原始 JSON 沒有這個限制。

前端送出前用 `JSON.parse` 驗證格式（避免打錯字送出無意義的請求）；後端再次驗證是否為合法的 JSON 物件（不是陣列或純量），否則回 `VALIDATION` 錯誤。`query` 留空等同 `{}`（不過濾，撈集合裡任意前 N 筆）。

**Risk**: 直接把管理員輸入的 JSON 轉成 Mongo filter 執行，理論上可以塞入 `$where`（伺服器端任意 JS 執行）等運算子。
**Mitigation**: 這條連線本來就是管理員自己客戶的唯讀舊系統（不是 bandao 自己的正式資料庫），且整支請求鏈路已經是 admin-only（`RequireAdmin`）— 攻擊者要能打這支 API，早就已經是取得 admin session 的內部威脅，跟直接讓管理員自己貼一條惡意連線字串本質上是同一等級的風險，非本次新增。若未來要收斂，可以在 D2 之上加一層「拒絕 filter 文件中出現 `$where`/`$function`」的檢查，但這次先不做（YAGNI——目前沒有真的需要更複雜查詢運算子的案例）。

### D3: 巢狀文件攤平成 dot-path 清單，邏輯放前端

`sample` 端點直接把 Mongo 文件轉成 `serde_json::Value` 陣列回傳（不裁切、不攤平），前端收到後用一個遞迴函式把每份文件攤平成 `{path, value}[]`（例如 `{signer: {username: "a"}}` → `{path: "signer.username", value: "a"}`），多份樣本文件之間的路徑取聯集（因為某些欄位可能是稀疏的，不是每筆都有）。

放前端而不是後端 Rust 的理由：這純粹是「怎麼把 JSON 展示成好拖放的清單」的顯示邏輯，不影響回填時真正的欄位語意（那部分邏輯早就在 `provider.rs::get_by_path` 裡，且完全獨立）。放前端可以直接操作已經拿到手的 JSON、不需要為了單一 UI 需求在 Rust 端新寫一支「列舉文件所有路徑」的函式並維護對應測試。

**Alternative considered**：後端攤平，直接回傳 `{path, sample_value}[]`。拒絕——後端還要決定「同一路徑多筆文件時要不要合併/怎麼合併」這種純展示邏輯的細節，放前端改起來更快，也不會增加 Rust 測試負擔。

### D4: 拖放用原生 HTML5 Drag and Drop API，不引入套件

`admin-web` 目前完全沒有拖放需求的先例。這次的拖放場景很單純——一份不超過 ~20 個 chips 的靜態清單，拖到 7 個固定的目標欄位（沒有排序、沒有巢狀清單、沒有觸控裝置手勢的特殊需求）——用原生 `draggable`/`@dragstart`/`@dragover.prevent`/`@drop` 就足夠，不需要為此新增一個 npm 依賴。

**Alternative considered**：引入 `vuedraggable` 或類似套件。拒絝——那類套件是為「可排序清單」設計的（拖曳重新排序同一個 list），這裡的場景是「從清單複製一個值到另一個輸入框」，用不到排序套件的核心能力，只會多一個依賴。

### D5: 拖放不取代文字輸入框，只是快速填入的捷徑

7 個目標欄位（身份/時間/緯度/經度/地址/備註/動作）維持原本的 `<input>` + `v-model`，拖放的 `@drop` handler 就是把對應的 dot-path 字串寫進同一個 ref。管理員仍可以直接手動打字或修改。

理由：樣本只抓 N 筆文件，如果某個目標欄位在樣本裡剛好都是空值/不存在（稀疏欄位，例如某些歷史紀錄沒有 `address`），純拖放會讓管理員完全填不出那個欄位；保留文字輸入是必要的退路，不是可有可無的裝飾。

## Risks / Trade-offs

- **[Risk]** 採樣端點與 `preview` 端點高度相似（都連線、都限制筆數），未來要是行為需要同步變更（例如連線逾時秒數調整），容易漏改其中一個。
  **Mitigation**: 兩者共用同一個連線建立輔助函式（`provider.rs` 既有的連線邏輯抽出可複用的部分），只有查詢/回傳形狀不同。

- **[Risk]** 原始 JSON query 讓管理員可以打出效能很差的查詢（例如對非索引欄位做正規表示式掃描），拖慢舊系統資料庫。
  **Mitigation**: 這條連線是唯讀操作、`limit` 上限本身就控制了取回筆數；查詢慢的風險由管理員自負（他們對自己接的舊系統資料庫規模與索引狀況知情），不在這次設計範圍內處理。

- **[Trade-off]** 前端攤平巢狀文件的邏輯（D3）如果舊文件裡有陣列欄位（例如 `tags: ["a","b"]`），目前只會處理物件巢狀，不特別展開陣列——只會把整個陣列當一個值顯示，不會攤平成 `tags.0`/`tags.1`。這次的舊系統資料形狀（screenshot 顯示的）沒有陣列欄位，暫不處理；如果未來遇到有陣列欄位的客戶資料，屆時再擴充攤平邏輯。

## Open Questions

（無——本次範圍內的技術決策已在上面定案；動作值處理與陣列欄位攤平明確列為 non-goal / 已知限制，留待未來真的遇到需求再處理。）
