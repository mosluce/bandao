## Why

一個客戶需要讓外部系統（廠商的 HR 系統）用排程方式定期呼叫 API 拉取資料。目前系統只有兩種認證：dashboard session cookie（給人用瀏覽器登入）與 AppUser bearer token（給手機 App 用）。兩者都不適合「機器對機器、長期、無人值守排程呼叫」的情境。需要一種新的、Org 管理員可自行簽發／管理的長效憑證，作為未來所有「外部系統呼叫班到 API」情境的共通基礎設施，第一個消費者是即將接著做的震旦雲打卡匯出（`add-zhengdan-checkin-export`）。

## What Changes

- 新增 `org_api_tokens` collection：每個 Org 可以同時擁有**多個具名 token**，各自獨立管理。
- 每個 token 綁定一組 **scope**（權限範圍），採最小權限原則——不等同 admin 全權限，只能存取 token 本身被授予的能力。首發只開放一個 scope 值 `checkin:read`（供後續震旦雲匯出使用），機制本身可擴充。
- Token **無到期時間**（不會自動過期），但 admin 可以隨時對個別 token 做：**rotate**（產生新密鑰、舊密鑰立即失效）、**停用／啟用**、**刪除**。
- Token 明碼只在**建立當下**與**rotate 當下**顯示一次，之後系統只保留雜湊值與供辨識用的字首；介面不再顯示明碼。
- 新增 `Authorization: Bearer <token>` 的機器認證管道，與既有 AppUser bearer token 用同一個 header、但用可辨識的字首（`bandao_at_...`）區分，解析出 `(org_id, scopes)` 供 handler 檢查。
- admin-web 新增「API Token」管理頁（admin-only）：列表（名稱／scope／狀態／建立時間／最後使用時間）＋ 建立／rotate／停用啟用／刪除操作，UX 沿用既有「一次性顯示密碼」的 modal 模式（App 使用者建立初始密碼那套）。

## Capabilities

### New Capabilities
- `org-api-tokens`：Org 範圍的 API token 資料模型、CRUD 端點、scope 授權機制、bearer-token 認證解析、admin-web 管理介面。

### Modified Capabilities
（無——這是全新、獨立的能力，不改動既有 capability 的 requirement。）

## Impact

- **api/（Rust）**：新增 `domain::OrgApiToken`（含 `ApiTokenScope` enum，首發只有 `CheckinRead`）；新增 `db::org_api_tokens` repository；新增 token 產生／雜湊模組（SHA-256，非 argon2——token 本身已是高熵隨機值，不需要慢雜湊）；新增 `auth::api_token` 的 `ApiTokenAuth` extractor，依 `bandao_at_` 字首與現有 AppUser token 解析分流；新增 `handlers::org_api_tokens`（`GET/POST /orgs/me/api-tokens`、`POST /orgs/me/api-tokens/:id/rotate`、`PATCH /orgs/me/api-tokens/:id`、`DELETE /orgs/me/api-tokens/:id`，皆 admin-only）。此變更**不修改**任何既有 endpoint 的認證行為——目前沒有任何既有 endpoint 會接受 `ApiTokenAuth`，要等 `add-zhengdan-checkin-export` 才會第一次被消費。
- **admin-web（Nuxt）**：新增 `pages/settings/api-tokens.vue` + 對應 composable／型別；儀表板新增入口卡（比照 `驗證來源` 入口卡的模式）。
- **app/（Flutter）**：不受影響。
- **已知限制**：token 無到期時間是刻意設計（客戶排程腳本不應該因為 token 過期而突然停擺、又沒有自動續期機制），代價是外洩風險完全依賴「客戶／admin 有沒有及時發現並 rotate」，沒有系統性的兜底。
