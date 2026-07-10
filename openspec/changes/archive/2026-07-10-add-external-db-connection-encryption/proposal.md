## Why

`add-external-db-app-auth` 上線後，第一個真實客戶（KLCC，`erp.klcc.com.tw`）就連不上。原因是 MSSQL provider 的連線加密設定**寫死**：

- 完全沒設 `encryption` → tiberius 在 `rustls` feature 下預設 `Required`（強制 TLS 握手）。
- 無條件呼叫 `cfg.trust_cert()`（信任任何憑證）。

台灣中小企業的舊 MSSQL 常常不支援 / 不允許 TLS。強制 `Required` 的握手在這些機器上直接 `database handshake failed` —— 也就是要「Encrypt=Optional/false」才連得上（客戶用 Azure Data Studio 正是選 Optional 才成功）。目前 admin 無從調整，這個功能對這類客戶等於不能用。

本 change 把 ADS 連線對話框那兩個旋鈕（**Encrypt** 下拉 + **Trust server certificate** 勾選）搬進每個 Org 的 `external_auth` 設定，讓 admin 依自家 MSSQL 實況調整並用「試登入」試出對的組合。

## What Changes

- `ExternalAuthConfig` 新增兩個**非機密**欄位：
  - `encrypt`: `off | optional | required`（對映 tiberius `Off / On / Required`），預設 **`optional`**。
  - `trust_server_certificate`: bool，預設 **`true`**（取代寫死的 `cfg.trust_cert()`）。
- MSSQL provider 依設定套用 `cfg.encryption(...)` 與**條件式** `cfg.trust_cert()`（僅在 `trust=true` 時）。
- 儲存設定時驗證 `encrypt` 為三個合法值之一。
- 兩欄非機密，照常在 `OrgDto` 的 `external_auth` summary 回吐、admin-web 顯示與編輯（不加密、不像連線密碼）。
- admin-web 設定頁新增 Encrypt 下拉 + Trust server certificate 勾選。
- 試登入 dry-run 自動吃到這兩欄（走同一份設定），讓 admin 自助試連。

## Capabilities

### Modified Capabilities
- `external-db-auth`: 連線設定新增 `encrypt` 與 `trust_server_certificate`（含預設與驗證），provider 據以設定加密層與憑證信任；兩者為非機密、可在設定回應中呈現。

## Impact

- **api（Rust）**：`domain::ExternalAuthConfig` 加兩欄（serde default 讓舊文件相容）；`auth/providers/mssql.rs` 套 `encryption` + 條件式 `trust_cert`；`providers::validate_query_settings` 納入 `encrypt` 合法值檢查；`ExternalAuthSummaryDto` 回吐兩欄；`handlers/external_auth.rs` 存檔帶入兩欄。
- **admin-web（Nuxt）**：`pages/settings/auth.vue` 加 Encrypt 下拉 + Trust server cert 勾選；`types/api` 的 `ExternalAuthInput` / summary 補兩欄。
- **app（Flutter）**：無變更。
- **遷移**：serde default → 既有 `external_auth` 文件視為 `encrypt=optional` + `trust_server_certificate=true`；KLCC 這類舊機因此**預設就更可能連上**（`optional` 能加密就加密、不強制），連不上再讓 admin 調成 `off`。
- **安全取捨**：`trust_server_certificate` 預設 `true` 沿用現況（對自簽憑證寬鬆、有 MITM 風險）；改為明確 per-Org 開關本身即是改善（明示意圖），進階 admin 可關閉走正常憑證驗證。
