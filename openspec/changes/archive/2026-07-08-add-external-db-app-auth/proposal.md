## Why

多數目標客戶（台灣中小企業）早已有一套自己的員工／帳號系統，通常是 MSSQL 後端。他們不想在班到重建一份員工名冊、也不想維護兩套密碼。若能讓 App 使用者直接用「原系統的帳密」登入，導入門檻會大幅降低。

本次為每個 Org 提供「外部資料庫驗證」選項：Org 管理員填入連線資訊與一條參數化查詢，App 使用者登入時由 `api/` 向該外部資料庫比對帳密，通過即發 session。打卡／軌跡等既有功能不受影響。

## What Changes

- 每個 Org 可在 `auth_source ∈ { internal, external_db }` 間**二選一**（預設 `internal`，維持現況）。
- 新增 App 使用者驗證的 provider 抽象層；`internal`（現有 Mongo + argon2）與 `external_db` 都走同一條登入路徑。
- 新增 **MSSQL** provider（首發唯一實作，架構保留可擴充其他 driver）。
- 外部驗證採**參數化查詢**：Org 提供帶 `@account` / `@password` 佔位符的 SQL 模板，加上 `key_col`（唯一識別欄）與 `display_col`（顯示名稱欄）。
- 外部使用者第一次登入成功時**即時建立本地影子 AppUser**（`auth_source=external`、無 `password_hash`、以 `(org_id, external_key)` 唯一），後續打卡／session／軌跡照舊掛在此 `app_user_id`。
- admin-web 新增專屬設定子頁：填寫連線設定、查詢模板、欄位對應，並提供**完整「試登入」dry-run**（跑真 query + 欄位對應，但不建 session／影子身份）。
- **BREAKING（僅限切到 external_db 的 Org）**：切換為 `external_db` 後，該 Org 既有的 internal AppUser 無法登入（資料與打卡歷史保留，切回即恢復）；反之亦然。切換時前端跳確認護欄。
- external 模式下，admin-web 的 App 使用者頁隱藏「新增／重設密碼」，改為列出已登入過的影子身份（唯一識別／名稱／最後登入），保留「停用」作為本地封鎖。

## Capabilities

### New Capabilities
- `external-db-auth`: App 使用者驗證的 provider 抽象、MSSQL provider、每個 Org 的外部驗證設定（含密碼加密存放）、參數化查詢契約、影子身份 JIT provisioning、admin 試登入 dry-run 端點。

### Modified Capabilities
- `app-user-mgmt`: 登入改為委派給 Org 設定的驗證來源；external 模式下 JIT 建立影子 AppUser；建立／重設密碼在 external 模式停用；使用者清單納入外部影子身份。
- `org-tenancy`: Org settings 容器新增 `auth_source` 與 `external_auth` 設定區塊。

## Impact

- **api/（Rust）**：新增 `AppAuthProvider` trait 與 registry、MSSQL provider（引入 `tiberius` 相依，`cargo build` 會變重）；`domain::AppUser` 的 `password_hash` 改 optional、新增 `auth_source` / `external_key`；`app_users` 索引與 repository（新增 `(org_id, external_key)` 唯一索引與 `upsert_shadow`）；`Org.settings` 新增 `auth_source` / `external_auth`（密碼用既有 secret 對稱加密）；新增 `POST /orgs/me/external-auth/test-login`（admin-only, dry-run）；改寫 `POST /app/auth/login`。
- **admin-web（Nuxt）**：新增 `pages/settings/auth.vue` + 儀表板入口卡；App 使用者頁 external 模式變體；新增對應 API 型別與 composable。
- **app/（Flutter）**：登入表單三欄不變，無需改動（行為差異全在 server 端）。
- **已知限制**：prod（Tailscale 拓樸）不一定連得到客戶內網 MSSQL——首版假設連得到，網路可達性方案留待後續。
