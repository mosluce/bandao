## Why

`Org.settings.external_auth`（MSSQL 連線設定：host、port、database、username、query 樣板、加密模式——除了連線密碼本身以外幾乎全部欄位）目前透過 `OrgDto` 沒有依角色過濾地回傳給任何拿得到這個 Org 資料的呼叫者。實際影響範圍比單一角色更廣：

- **Dashboard member**（`GET /me`、登入、註冊回應）：非 admin 的一般成員能看到公司內部資料庫的連線細節。
- **AppUser**（`POST /app/auth/login`、`GET /app/me`）：**第一線打卡員工**——一個連 dashboard 都上不去、權限層級遠低於 dashboard member 的角色——同樣拿得到這份設定。

`external-db-auth` spec 已經明確把「設定」與「test-login」兩個端點定義成 admin-only（含 member 被拒的 scenario），但從沒明講「讀取」這一側的可見性規則，這次補上這個一直存在的缺口。

## What Changes

- `OrgDto` 的建構邏輯改成依呼叫者角色決定是否包含 `external_auth`：dashboard admin 看得到，member 與所有 AppUser 情境一律不包含。
- 受影響的兩個組裝路徑：
  - `handlers/auth.rs::build_auth_response`（`GET /me`、`POST /auth/login`、`POST /auth/register`）——依每筆 membership 實際的 `role` 決定。
  - `handlers/app_auth.rs`（`POST /app/auth/login`、`GET /app/me`）——AppUser 沒有 dashboard role 概念，一律視為非 admin，不包含。
- 已經是 `RequireAdmin`-only 端點的既有回應路徑（`POST /orgs/me/owner`、`POST /orgs/me/external-auth`）不受影響——呼叫者本來就必須是 admin 才能觸發那些回應。

## Capabilities

### New Capabilities
（無）

### Modified Capabilities
- `external-db-auth`：新增一條 requirement，明確規定 `external_auth` 設定只在呼叫者是 dashboard admin 時才出現在任何 API 回應裡；member 與 AppUser 情境一律不包含該欄位（不是回傳空物件或遮蔽密碼以外欄位，是整個欄位不存在）。

## Impact

- **api/（Rust）**：`handlers/auth.rs::OrgDto::from_org` 呼叫點改用一個依角色決定的建構方式；`handlers/app_auth.rs` 的兩個呼叫點固定使用「非 admin」路徑。純後端修正，不動資料模型、不動任何既有 endpoint 的路由或權限判斷本身（`RequireAdmin`／`RequireActiveOrg` 的既有守門邏輯完全不變，這次只動「回應要不要帶這個欄位」）。
- **admin-web / app/**：型別上 `external_auth` 從「一定有可能出現」變成「member/AppUser 情境下保證不會出現」，兩邊消費端不需要改動（本來就要處理 `undefined`/`null`）。
- **已知限制**：這次不處理「member 能不能透過 admin-web 的驗證來源頁面看到設定摘要」這個 UI 層問題——那是 `add-admin-web-sidemenu` change 的範圍（member read-only 開放的邊界討論已經把「驗證來源」明確排除在外，維持 admin-only）。這裡只修 API 回應本身的漏洞。
