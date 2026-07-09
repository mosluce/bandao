## Context

App 使用者目前僅能用內建驗證登入：`POST /app/auth/login` 解析 `org_code` → 在 Mongo `app_users` 以 `(org_id, username_lower)` 查人 → argon2 驗證密碼 → 發 `app_sessions` token。所有打卡資料（`checkin_events`、`location_pings`、`app_sessions`）都以 `app_user_id`（Mongo ObjectId）為外鍵錨點。

目標客戶多半已有自己的員工／帳號系統（常見為 MSSQL），希望員工用原帳密登入而不必在班到另建名冊。本設計在不動搖「`app_user_id` 是資料錨點」前提下，讓每個 Org 可改用外部資料庫驗證。

相關 explore 已定調：可擴充但首發只出 MSSQL；密碼預設明碼比對；使用者清單只列登入過的影子身份；每個 Org 在 internal / external_db 二選一；欄位對應用設定指定 `key_col` + `display_col`；admin-web 用專屬設定子頁 + 完整試登入。

## Goals / Non-Goals

**Goals:**
- 每個 Org 可獨立選擇 `internal` 或 `external_db` 驗證，預設 `internal`，完全向後相容。
- 外部驗證不破壞既有資料模型：外部使用者以「影子 AppUser」形式取得穩定的本地 `app_user_id`。
- 驗證邏輯抽象成 provider，internal 與 external 走同一條登入路徑；新增其他 driver 只需擴充 registry。
- 杜絕 SQL injection：Org 提供的是帶佔位符的模板，帳密一律參數綁定。
- Org DB 連線密碼加密存放，不落 log、不回吐明文。
- 管理員能透過「試登入」自助驗證整條設定是否正確。

**Non-Goals:**
- 不支援 MSSQL 以外的 driver（架構保留擴充點，但不實作）。
- 不處理外部 DB 存 hash 的比對規則（首版只做明碼比對；hash 留待後續）。
- 不主動同步外部名冊（使用者清單只含登入過的影子身份）。
- 不解決 prod 連不到客戶內網 MSSQL 的網路可達性（首版假設連得到）。
- 不引入 per-org 連線池（首版每次登入現連現斷 + timeout）。
- App（Flutter）端不改動；登入表單三欄與行為不變。

## Decisions

### D1. Provider 抽象，internal 也包進來
新增 `AppAuthProvider` trait：`authenticate(&cfg, account, password) -> Result<ExternalIdentity, AuthProviderError>`，回傳解析後的 `{ external_key, display_name }` 或具名錯誤。登入 handler 依 Org 的 `auth_source` 從 registry 取 provider。`InternalProvider` 把現有 Mongo + argon2 流程包起來，`MssqlProvider` 為新實作。
- **為何**：讓登入路徑單一化，未來加 driver 不動 handler；也讓「影子身份」與「session 發放」邏輯在兩種模式間共用。
- **替代**：在 handler 內 `match auth_source` 直接分支——會把外部連線細節漏進 handler，違反 `api/` 的分層規範。

### D2. 影子身份 JIT provisioning
外部驗證成功後，以 `(org_id, external_key)` upsert 一筆本地 AppUser：首次建立（`auth_source=external`、`password_hash=None`、`display_name` 取自 `display_col`、`needs_password_change=false`），後續沿用同一 `_id` 並更新 `display_name` / `last_login_at`。
- **為何**：打卡／session／軌跡全都需要穩定的 `app_user_id`；JIT 讓「使用者清單只列登入過的人」自然成立，無需同步名冊。
- **替代**：預先同步整份外部名冊——需要第二條查詢與排程，且與「只列登入過」的定調相悖，first version 不做。

### D3. 資料模型調整
- `domain::AppUser`：`password_hash: Option<String>`；新增 `auth_source: AppUserAuthSource`（`Internal` | `External`）與 `external_key: Option<String>`。
- 索引：保留 `(org_id, username_lower)` unique（internal）；新增 `(org_id, external_key)` unique partial（僅 external）。
- `Org.settings`：新增 `auth_source`（頂層字串，預設 `internal`）與 `external_auth` 子文件。
- `external_auth`：`{ driver, host, port, database, username, password_encrypted, query, key_col, display_col }`。
- **為何 optional password_hash**：external 使用者沒有本地密碼；用 Option 明確表達，避免塞空字串。

### D4. 密碼／secret 處理
加密對象是**我們連進客戶 MSSQL 的連線帳號密碼**（`external_auth.password`），不是員工登入密碼。兩者處理方式不同：
- **員工登入密碼**：external 模式下不儲存，丟給 provider 綁參比對完即丟；只在記憶體傳遞，不寫 log、不入錯誤訊息。
- **連線密碼**：每次登入／試登入都要還原成明文才能連 MSSQL，因此**必須用可逆的對稱加密（AEAD）而非 argon2 雜湊**（雜湊單向、還原不了）。加密後存 Mongo 的 `password_encrypted`；API 回應永不含明文，只回 `password_set: bool`。
- **現況盤點**：`api/` 目前**沒有**任何對稱加密——只有 `argon2`（單向）與 `rand`，config 也無加密金鑰 env。因此本項需**新引入** AEAD 依賴（如 `chacha20poly1305` 或 `aes-gcm`）+ 一把金鑰來源（如 `BANDAO_SECRET_KEY` env，並補進 DEPLOY.md 的 env 矩陣）。這不是沿用既有工具。
- **為何要加密**：Org DB 連線憑證是別人系統的高價值 secret；Mongo dump／備份外流／DB 層入侵時不應以明文躺著。
- **替代**：明文存放——一旦 Mongo 外洩即等於交出客戶 DB 存取權，否決。

### D5. 參數化查詢契約
Org 的 `query` 必須含 `@account` 與 `@password` 佔位符，由 MssqlProvider 以 tiberius 參數綁定執行，永不字串拼接。查詢預期回傳 0 或 1 列；取 `key_col` 為 `external_key`、`display_col` 為 `display_name`。
- 驗證規則：儲存前檢查 query 含兩個佔位符、`key_col`/`display_col` 非空。
- **為何**：字串拼接帳密是典型 SQLi。佔位符 + 綁參是唯一安全作法。
- **替代**：讓 Org 寫完整 SQL 由我們字串取代——直接開 SQLi 後門，否決。

### D6. 錯誤語意
- 帳密比對失敗（查無列）→ `INVALID_CREDENTIALS`（與 internal 一致，不洩漏）。
- 連線失敗／查詢執行錯誤／設定缺失 → `EXTERNAL_AUTH_UNAVAILABLE`（可與「打錯密碼」區分，讓使用者知道是系統問題）。
- 試登入端點回傳更細的診斷（連不上／SQL parse 錯／欄位不存在／查無帳密），僅 admin 可見。
- **為何**：登入面對終端使用者要收斂不洩漏；試登入面對 admin 要能除錯。

### D7. 試登入 dry-run 端點
`POST /orgs/me/external-auth/test-login`（admin-only）：接受一組測試帳密 + 待測設定，跑完整 provider 流程，回傳解析出的 `external_key` / `display_name` 或具名錯誤，但**不建 session、不建影子身份、不寫任何資料**。測試密碼不落 log／DB。
- **為何**：設錯時終端使用者只會默默登不進、管理員無從查起。完整 dry-run 是這功能能落地的關鍵。
- **張力**：等於在後台開了「用任意帳密查外部 DB」的介面——嚴格綁 org scope + admin、dry-run 不寫入，降低被當探測工具的風險。

### D8. 連線策略
首版每次登入 / 試登入現連現斷，帶連線 timeout（如 5s）。
- **為何**：登入不頻繁；per-org 連線池的生命週期與失效處理複雜度，first version 不值得。
- **替代**：per-org pool——留待有效能需求再說。

### D9. admin-web 專屬設定子頁 + 模式切換護欄
新增 `pages/settings/auth.vue`（非 admin redirect），儀表板放輕量入口卡。切換 `auth_source` 時跳確認 modal，明示「N 個現有帳號將無法登入、資料保留、可切回」。密碼欄 write-only（顯示「已設定」+ 變更）。external 模式下 App 使用者頁隱藏新增／重設密碼、改列影子身份、保留停用。
- **為何**：外部設定欄位多（連線 + query + 欄位對應 + 試登入），塞進儀表板卡片會過高；子頁有空間展開。切換是會讓一批人登不進來的動作，需要明確護欄。

## Risks / Trade-offs

- **tiberius 相依重量** → 引入 MSSQL driver 會拉不少 crate、拖慢首次 `cargo build`。接受，記錄於 proposal Impact；若日後成負擔可 feature-gate。
- **prod 連不到客戶內網 MSSQL** → 首版假設連得到；design 明列為已知限制，避免上客戶時才發現。後續可能需 VPN / 隧道方案或客戶端 agent。
- **外部 DB 掛掉 → 登入跟著掛** → 可用性耦合本質存在。以 `EXTERNAL_AUTH_UNAVAILABLE` 明確回報 + 連線 timeout 控制延遲；不做本地 fallback（會破壞二選一語意）。
- **明碼比對把員工明碼帶進我們系統** → 只在記憶體傳遞、不落 log；文件明示此模式的安全前提。hash 模式列為後續。
- **試登入被濫用為探測工具** → 嚴格 admin + org scope、dry-run 不寫入、可加基本速率限制。
- **切換模式讓一批人登不進** → 前端護欄 modal + 資料保留可逆；不刪任何既有 AppUser。

## Migration Plan

1. Schema 向後相容：`auth_source` 缺省視為 `internal`，既有 Org 無感。
2. `AppUser.password_hash` 由 required 改 Option——既有文件皆有值，反序列化相容；新 external 使用者才會是 None。
3. 上線後 Org 需自行在設定頁切換並填外部設定，才會啟用外部驗證；預設不影響任何人。
4. Rollback：Org 隨時可切回 `internal`，既有 internal AppUser 立即恢復可登入。程式層面此 change 若需整體回退，因預設路徑未變，風險低。
5. 新增索引 `(org_id, external_key)` 於 migration 建立（partial / sparse，僅 external 文件）。

## Open Questions

- ~~Org DB 憑證加密要沿用哪個既有 secret？~~ **已釐清**：`api/` 無現成對稱加密，需新引入 AEAD 依賴 + 金鑰 env（見 D4）。待定的是金鑰來源命名與部署注入方式（傾向 `BANDAO_SECRET_KEY`，DEPLOY.md env 矩陣補一格）、以及是否要支援金鑰輪換。
- 試登入是否需要速率限制門檻？（傾向加基本限制，數值待定。）
- external 模式下「停用」影子身份的語意是否要在 App 端也立即斷線？（傾向沿用既有 disable → session 失效邏輯。）
