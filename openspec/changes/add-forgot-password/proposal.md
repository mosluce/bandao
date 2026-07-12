## Why

Dashboard user（admin/member 的登入帳號）忘記密碼目前**完全沒有自助流程**——唯一的修復手段是我們直接改 DB（這個 session 已經真的發生過）。這是 ROADMAP 上「忘記密碼」跟「接 Resend 支援寄信」這兩筆記錄的交集，先把最痛的這塊做掉。

同時這是專案第一次需要「系統主動寄信」這個能力，值得順手把寄信抽象做成可替換的（真的打 Resend 的實作 + 測試用替身），讓之後「邀請成員用 email 邀請信」「註冊需驗證信箱」這兩個 ROADMAP 項目能直接複用，不用各自兜一份。

## What Changes

### 新增 email 寄送抽象（`email-delivery` capability）

- 新的 `EmailSender` trait（`api/src/services/email.rs`），精神跟現有的 `ReverseGeocoder` trait 完全一致：一個真的打外部 API 的實作（`ResendEmailSender`）、一個測試用的替身（`NoopEmailSender`，log 但不真的寄）。透過 `AppState` 注入，測試可以換掉。
- 新環境變數 `RESEND_API_KEY`（可選，語意跟現有的 `BANDAO_SECRET_KEY` 一樣：沒設定就用 `NoopEmailSender`，本機開發/測試不需要真的申請 Resend 帳號）。
- 寄信失敗 fail-soft：不影響呼叫端的回應語意，只記 log。

### 新增忘記密碼 / 重設密碼流程（`dashboard-auth` capability）

- `POST /auth/forgot-password { email }`：一律回 204，不透露該 email 是否存在系統裡。若存在，產生一個 60 分鐘內有效、單次使用的 reset token，SHA-256 雜湊後存入新的 `password_reset_tokens` collection，寄一封含重設連結的信。
- `POST /auth/reset-password { token, new_password }`：驗證 token（存在、未過期、未使用過），更新密碼、標記 token 已使用、**踢掉該使用者所有現存的 dashboard session**（沿用既有的 `dashboard_sessions.delete_all_by_user_id`）。成功後導回 `/login`，不自動登入。
- 防洗版騷擾：同一使用者的請求有 60 秒冷卻——冷卻期內的重複請求一樣回 204（不洩漏資訊），但不產生新 token、不寄信。冷卻判斷直接查 `password_reset_tokens` 這個使用者最近一筆的建立時間，不另開一張 marker 表。
- admin-web 新增 `/forgot-password`（輸入 email）、`/reset-password`（從網址帶 token，輸入新密碼）兩個頁面，套用跟 `/login`、`/register` 一樣的 pre-auth 模式（不套 sidebar layout）；`/login` 頁面加一個「忘記密碼？」連結。

## Capabilities

### New Capabilities

- `email-delivery`：`EmailSender` trait、Resend 實作、失敗語意、環境變數設定方式。

### Modified Capabilities

- `dashboard-auth`：新增忘記密碼／重設密碼兩條 requirement，以及請求頻率限制的 requirement。

## Impact

- **api/（Rust）**：新檔案 `services/email.rs`（trait + 兩個實作）；新 collection `password_reset_tokens`（`db/password_reset_tokens.rs` + `domain.rs` 新 struct）；`handlers/auth.rs` 新增 `forgot_password`/`reset_password` handler；`handlers/mod.rs` 新增兩條路由；`config.rs` 新增 `RESEND_API_KEY` 讀取；`state.rs` 新增 `email: SharedEmailSender` 欄位與 `with_email_sender` 建構子（比照 `with_geocoder`）；`error.rs` 新增 `InvalidResetToken` 錯誤。
- **admin-web（Nuxt）**：新頁面 `pages/forgot-password.vue`、`pages/reset-password.vue`；`pages/login.vue` 加連結；新 composable 或擴充 `useAuth.ts` 兩個方法。
- **DEPLOY.md**：補一列 `RESEND_API_KEY` 到環境變數表格，比照 `BANDAO_SECRET_KEY` 的寫法（可選、缺少時的降級行為）。
- **不受影響**：`add-admin-web-sidemenu`／`restructure-admin-web-nav` 定案的 sidebar 不適用於這兩個新頁面（跟 `/login`、`/register` 一樣是 pre-auth 頁面）。

## Non-Goals（明確不做，已記錄到 ROADMAP 供未來排程）

- 不做 per-IP 流量限制、不做 CAPTCHA——只做同一使用者 60 秒冷卻，足以擋最直接的騷擾情境，其餘留給未來的「登入失敗鎖定」機制一起處理。
- 不做寄信失敗的背景重試佇列——目前 production 是單一長跑 process、沒有 worker/scheduler 基礎設施；寄信失敗就是失敗、log 下來，使用者可以再點一次「忘記密碼」重試。「queue / scheduler / worker 基礎設施」已另外記錄一筆 ROADMAP 項目，屬於獨立的、跨多個未來功能受益的基礎建設決定，不在這次範圍內先做。
