## Context

現有兩種認證管道：

- **Dashboard session**（`dashboard-auth`）：cookie-based，人用瀏覽器登入，帶 `(user_id, current_org_id)`，middleware 每 request 重查 role。
- **AppUser session**（`app-checkin` 底下）：`Authorization: Bearer <token>`，token 是 32-byte OsRng 隨機值、base64url 編碼（43 字元），**明碼直接存 Mongo**、`find_by_token` 直接比對，無到期（依賴登出／停用清除）。

兩者都是「代表一個登入中的人／裝置」的憑證，語意上不適合給「排程執行、無人值守、放在客戶自己機器上」的機器呼叫用——尤其是外洩風險：客戶的 Windows Server 上會有一個組態檔案長期存著這個憑證，這比瀏覽器 session cookie 或手機上的 secure storage 更容易被複製走（隨便一個有檔案系統存取權的人都拿得到）。因此需要一種語意與生命週期都不同的憑證：**長效、Org 管理員自主簽發與撤銷、範圍受限（scope）**。

第一個消費者是 `add-zhengdan-checkin-export`（客戶排程呼叫 API 匯出打卡紀錄），但這個能力本身要設計成通用機制，不綁死震旦雲。

## Goals / Non-Goals

**Goals:**
- Org 可以建立多個具名、各自獨立生命週期的 API token。
- Token 綁定 scope，遵循最小權限——被消費的 endpoint 各自宣告需要哪個 scope，不是「有 token 就等於 admin」。
- Token 密鑰只在建立／rotate 當下明碼顯示一次，之後系統不再吐出明碼，只存雜湊。
- 認證解析要能跟現有 AppUser bearer token 共存於同一個 `Authorization: Bearer` header，不互相誤判。
- admin-web 提供完整的自助管理（建立／rotate／停用／啟用／刪除／查看最後使用時間）。

**Non-Goals:**
- 不做到期時間／TTL（客戶明確要求無限效期，靠人工 rotate）。
- 不做 IP allowlist、rate limiting 等額外防護（沒人要求，不預先設計）。
- 不做「每個 token 精細到單一 endpoint」的授權粒度——粒度停在 scope（一組相關能力的集合），不是逐 endpoint 開關。
- 這次不把任何既有 endpoint 改成接受 token 認證；`org-api-tokens` 只交付機制本身。

## Decisions

### D1. 獨立 collection，不塞進 `Org.settings`
`org_api_tokens` 是獨立 collection（`{ _id, org_id, name, token_hash, token_prefix, scopes, status, created_at, created_by, last_used_at, rotated_at }`），不像 `external_auth` 那樣塞進 `Org.settings` 子文件。
- **為何**：一個 Org 可以有多顆、各自有獨立生命週期（rotate/停用/刪除）與高頻寫入欄位（`last_used_at`）。塞進 `Org.settings` 會讓每次 token 使用都要更新整個 Org 文件，且「陣列裡的其中一個元素」在 Mongo 做部分更新／唯一性索引都比獨立 collection 麻煩。這跟 `dashboard_memberships` 獨立於 `dashboard_users` 是同樣的理由。
- **替代方案**：`Org.settings.api_tokens: []` 陣列——否決，理由如上。

### D2. Token 格式：`bandao_at_` 字首 + 高熵隨機字串
產生規則沿用 `session_token::generate()` 的做法（`OsRng` 32 bytes、base64url no-pad），但**加上 `bandao_at_` 字首**再回傳給使用者，例如 `bandao_at_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`。
- **為何要字首**：(1) 讓 `ApiTokenAuth` 的解析可以在觸碰資料庫前先用字首快速分流，不用兩種認證都各查一次資料庫；(2) 讓外洩的 token 在 log／support ticket／客戶的 script 裡一眼可辨識是哪種憑證，方便事後排查與撤銷；(3) 現有 AppUser token 是純 base64url、沒有字首，兩者天然不會混淆。
- **替代方案**：跟 AppUser token 走一樣的裸 base64url、認證時兩個 collection 都查一次（先查 app_sessions 再查 org_api_tokens）——可行但每個請求多一次不必要的資料庫查詢，且失去可辨識性，否決。

### D3. 儲存雜湊值，不存明碼——這是刻意偏離現有 session token 的作法
`org_api_tokens.token_hash` 存 **SHA-256** 雜湊（不是 argon2）。這跟現有 `app_sessions` / `dashboard_sessions` 明碼直接存 Mongo 的作法不同，是刻意的偏離。
- **為何要雜湊，而現有 session token 不用**：session token 生命週期短（登出或到期就失效），外洩窗口有限；API token **無到期時間**，一旦 Mongo 被 dump（備份外流、資料庫層入侵），一顆明碼存放、永不過期的 token 等於永久後門。雜湊之後，Mongo 外洩不會直接洩漏可用憑證。
- **為何用 SHA-256 不是 argon2**：argon2 的「刻意變慢」是為了防禦人類選的低熵密碼被暴力破解；這裡的 token 本身就是 32-byte 高熵隨機值，暴力枚舉不可行，用慢雜湊只會拖慢每一次 API 呼叫的驗證延遲，沒有對應的安全收益。這也是 GitHub／Stripe 這類機器 token 的標準做法。
- **`token_prefix`**：另外存一個明碼欄位（token 的前 N 碼，例如 `bandao_at_xxxxxxxx`），只給 UI 顯示辨識用（類似信用卡末四碼的概念），不足以重建完整 token。

### D4. Scope 是伺服器端已知清單，不是自由文字
`ApiTokenScope` 是 Rust enum（不是任意字串），首發只有 `CheckinRead`。建立／編輯 token 時，admin-web 用 checkbox 勾選已知 scope，不是自由輸入框。
- **為何**：自由文字 scope 會讓管理員手動輸入時打錯字（例如 `checkin:reed`），現有 endpoint 永遠不會宣告需要這個 scope，token 建出來就是個沉默失效的憑證，只有實際打 API 失敗才會發現。closed enum 讓「這個 scope 存在且有對應能力」在編譯期就成立。
- **代價**：以後每加一個新的外部串接用途，都要在 `org-api-tokens` 這個 capability 上補一個 MODIFIED delta 去擴充 enum——但這個成本很小（加一個 enum variant + 一行 admin-web 標籤），換來「token 不會建出一個實際上沒用的 scope」的保證，值得。

### D5. 認證解析：字首分流，不建 session
新增 `ApiTokenAuth` extractor（平行於現有 `AppAuthContext`）：從 `Authorization: Bearer` 取出 token，若字首是 `bandao_at_` 走 `org_api_tokens` 查找（雜湊比對）；否則維持現有 AppUser session 解析路徑不變。命中後檢查 `status == active`，回傳 `(org_id, scopes)`，**不寫任何 session/cookie**——每次請求都是獨立的無狀態驗證，沒有「登入」這個概念。
- `last_used_at` 在每次成功認證後更新。v1 先每次都寫（這個 token 的呼叫頻率預期是「每小時一次」等級，不是高 QPS 場景，直接更新不構成效能問題；如果未來有更高頻的消費者，再考慮節流寫入）。
- **為何不建立 session**：session 概念（登出、TTL、單一使用者身份）不適用於機器憑證；每個 scope 檢查應該是無狀態、純粹基於 token 本身的屬性。

### D6. Rotate 語意
Rotate = 同一筆 `org_api_tokens` row 產生新的 `token_hash`／`token_prefix`／`rotated_at`，**名稱與 scope 不變**，舊密鑰立即失效（沒有新舊並存的過渡期）。
- **為何不做新舊並存的過渡期**：客戶端排程呼叫失敗會在下一次執行（一小時內）重試，沒有「多裝置同時用同一顆 token、需要平滑換發」的情境，過渡期只是增加狀態機複雜度。
- 停用（`status = disabled`）與刪除是分開的動作：停用可逆（重新啟用即恢復，token 值不變），刪除不可逆（連 row 都移除，之後只能重新建立一顆全新 token）。

## Risks / Trade-offs

- **[Risk] 無到期時間的憑證，一旦外洩且沒被發現，永久有效** → Mitigation：`last_used_at` 讓 admin 至少有辦法定期稽核「這顆 token 還在用嗎」；長期可以考慮加一個「N 天沒使用就標示警示」的被動提醒（這次不做，先記錄可能性）。
- **[Risk] Scope 目前只到「一組能力」的粗粒度，不是逐 endpoint** → Mitigation：目前只有一個 scope、一個消費者，粗粒度不構成問題；等有第二個 scope 出現時再視情況決定要不要往下切。
- **[Trade-off] SHA-256 雜湊代表任何拿到 `token_hash` 的人可以用彩虹表／暴力枚舉理論上逆推**，但因為原始 token 是 32-byte 均勻隨機值（不是人類密碼），這個攻擊面在實務上不可行，屬於可接受的 trade-off。
