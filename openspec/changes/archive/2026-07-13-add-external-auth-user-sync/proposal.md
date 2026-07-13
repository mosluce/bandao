## Why

外部資料庫驗證（`external_db`）目前只能「被動」得知使用者存在——本地 `app_users` 只有在某個人真的登入過一次之後，才會透過 `upsert_shadow()` 產生一筆影子記錄。這代表 admin 在任何人登入之前，完全看不到外部系統裡有哪些潛在使用者，也沒辦法在正式切換到 `external_db` 之前先確認名單長什麼樣子。加一個手動同步功能，讓 admin 主動跑一次「列出全部」的查詢，把整批使用者的影子記錄先建起來。

## What Changes

- `ExternalAuthConfig` 新增一個 `list_query` 欄位：一個不帶 `@account`/`@password` 佔位符的唯讀 `SELECT`，跟現有專門驗證帳密用的 `query` 是兩個獨立設定。透過既有的 `POST /orgs/me/external-auth`（`configure`）一併設定，不另開端點。
- 新端點 `POST /orgs/me/external-auth/sync`（admin-only，僅在 `auth_source == external_db` 時可用）：讀取 org 已存的設定連線，執行 `list_query`，逐列比對本地 `app_users`：
  - 本地不存在的 `external_key` → 新增一筆影子記錄（`last_login_at = null`，`status = active`）。
  - 本地已存在的 → 更新 `display_name`，**不動** `last_login_at`。
  - 本地存在、但這次結果沒出現的 → 完全不處理（純新增/更新，不做停用或刪除）。
  - 單一列資料有問題（例如 `key_col` 是空值）→ 跳過該列，其餘列照常處理，最後在回應摘要裡列出被跳過的列與原因。
  - 連線失敗、query 語法錯誤、或整個結果集裡找不到 `key_col`/`display_col` 這兩個欄位名稱 → 整個同步失敗，不寫入任何東西。
- 新增一個獨立於 `upsert_shadow()` 的 repository 寫入路徑，避免同步把「從沒登入過」的使用者的 `last_login_at` 誤標成「剛剛登入」（`upsert_shadow()` 目前不論新建或更新都會蓋這個欄位，同步不能沿用）。
- admin-web `settings/auth.vue`：外部資料庫分支裡加一個「同步查詢」設定欄位，以及一顆「同步使用者名單」按鈕（僅 `auth_source == external_db` 時可按），按下去直接執行、無需確認彈窗（純新增/更新語意，沒有刪除風險），完成後就地顯示結果摘要（新增/更新/跳過筆數）。
- App 使用者清單頁不需要改動——`last_login_at` 為 `null` 時既有的 `formatDate` 已經會顯示「—」，同步進來但還沒真的登入過的使用者自然呈現這個狀態。

## Capabilities

### Modified Capabilities

- `external-db-auth`：新增「admin 可以手動同步外部使用者名單」的 requirement；`ExternalAuthConfig` 的儲存結構新增 `list_query` 欄位，對應的 requirement 也要更新以涵蓋這個新欄位。

## Impact

- **api/（Rust）**：`domain.rs`（`ExternalAuthConfig` 新增 `list_query` 欄位）；`handlers/external_auth.rs`（`ExternalAuthInput` 新增對應欄位、新增 `sync` handler）；`handlers/mod.rs`（新路由）；`auth/providers/mssql.rs` 或新檔案（跑 `list_query`、逐列解析的邏輯，跟 `resolve_identity` 平行但不綁定帳密參數）；`auth/providers/mod.rs`（`list_query` 的驗證函式，跟 `validate_query_settings` 邏輯相反：不能含 `@account`/`@password`）；`db/app_users.rs`（新的「同步用」寫入方法，不動 `last_login_at`）。
- **admin-web（Nuxt）**：`pages/settings/auth.vue`（新欄位、新按鈕、結果摘要 UI）；`composables/useExternalAuth.ts`（新方法）；`types/api.ts`（新的 request/response 型別）。
- **不受影響**：既有的登入流程（`query`/`resolve_identity`/`authenticate`）完全不動；`app-user-mgmt`／`admin-web-nav` 等其他 capability 不涉及。
