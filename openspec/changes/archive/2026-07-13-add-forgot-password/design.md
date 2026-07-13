## Context

`DashboardUser` 目前只有 `email` / `password_hash` / `created_at` / `updated_at` 四個欄位——沒有任何跟密碼重設相關的儲存位置。忘記密碼現在唯一的補救手段是我們直接改 DB。

系統目前也沒有任何「主動寄信」的能力，但 ROADMAP 上有三個功能都需要它（忘記密碼、email 邀請成員、註冊驗證信箱），彼此想共用同一套 provider 抽象。

架構上有一個現成、精神完全一致的先例可以直接照搬：`services/reverse_geocoder.rs` 的 `ReverseGeocoder` trait——一個真實實作（`NominatimGeocoder`）、一個測試替身（`StaticReverseGeocoder`），透過 `AppState::with_geocoder()` 注入。`config.rs` 的 `BANDAO_SECRET_KEY` 也示範了「`Option<T>`，沒設定就讓相關功能優雅降級」的既有慣例。

`dashboard_sessions.rs` 已經有 `delete_all_by_user_id(user_id)`——原本是給「刪除整個身份」的情境用的，但重設密碼後「踢掉所有現存 session」語意完全吻合，直接複用。

Session token 的產生方式（`auth::session_token::generate()`，32 bytes random、base64url、43 字元）也直接複用在 reset token 上。

## Goals / Non-Goals

**Goals:**
- 忘記密碼 → 重設密碼的完整自助流程，不再需要人工改 DB。
- `EmailSender` 抽象現在就做對，讓未來的「email 邀請成員」「註冊驗證信箱」可以直接複用，不用重新設計。
- 不洩漏任何帳號存在性資訊（`forgot-password` 端點的回應語意跟現有 `INVALID_CREDENTIALS` collapse 的哲學一致）。
- 陽春但足夠的濫用防護（同使用者 60 秒冷卻）。

**Non-Goals:**
- **不**現在就把 reset token 的儲存結構設計成給另外兩個未設計的 email 功能（邀請信、註冊驗證）共用。`EmailSender` 這個寄信介面確定三者都要用，值得現在做；但 token 的 schema（各自的過期規則、單次用途語意可能不同）現在硬要收斂成一張共用表，是在幫兩個還沒設計的功能猜規格——之後真的發現高度雷同，重構也不遲。
- **不**做 per-IP 流量限制或 CAPTCHA（見 proposal.md 的 Non-Goals）。
- **不**做寄信失敗的背景重試佇列——這需要 worker/scheduler 基礎設施，目前完全不存在，已另外記錄一筆獨立的 ROADMAP 項目。
- **不**處理 Resend 網域驗證（SPF/DKIM）本身——這是營運層的一次性設定，只在 DEPLOY.md 留一筆待辦。

## Decisions

### D1. `EmailSender` trait：跟 `ReverseGeocoder`同構

```rust
// api/src/services/email.rs
#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send(&self, to: &str, subject: &str, html_body: &str) -> Result<(), EmailSendError>;
}

pub type SharedEmailSender = Arc<dyn EmailSender>;

#[derive(Debug, thiserror::Error)]
pub enum EmailSendError {
    #[error("resend api error: {0}")]
    Provider(String),
    #[error("request failed: {0}")]
    Transport(#[from] reqwest::Error),
}

/// 打 Resend REST API（POST https://api.resend.com/emails）。reqwest 已經是既有
/// 依賴，Resend 沒有官方 Rust SDK，直接發 JSON 請求即可，不需要新套件。
pub struct ResendEmailSender {
    client: reqwest::Client,
    api_key: String,
    from_address: String,
}

/// 測試 / 沒設定 RESEND_API_KEY 時的替身：log 一行，不真的寄信，永遠回 Ok。
pub struct NoopEmailSender;
```

- **為何回 `Result` 而不是 `ReverseGeocoder` 那種 `Option`**：geocode 失敗只是少一個顯示欄位，呼叫端完全不需要知道失敗原因。寄信失敗我們自己（維運者）想在 log 裡看到「是 API key 錯、還是 Resend 那邊出問題、還是網路逾時」，所以用一個帶原因的 `Result`，呼叫端仍然一律 fail-soft 處理（`if let Err(e) = ... { tracing::warn!(?e, ...) }`），不改變外部 API 的回應語意。

### D2. `AppState` 注入方式，比照 `with_geocoder`

```rust
pub struct AppState {
    pub db: Arc<Db>,
    pub config: Arc<Config>,
    pub geocoder: SharedReverseGeocoder,
    pub email: SharedEmailSender,
}

impl AppState {
    pub fn new(db: Db, config: Config) -> Self {
        let email: SharedEmailSender = match &config.resend_api_key {
            Some(key) => Arc::new(ResendEmailSender::new(key.clone(), config.email_from_address.clone())),
            None => Arc::new(NoopEmailSender),
        };
        // ...
    }

    pub fn with_email_sender<E>(db: Db, config: Config, email: E) -> Self
    where E: EmailSender + 'static { /* 測試用 */ }
}
```

`config.rs` 新增：
```rust
pub resend_api_key: Option<String>,       // RESEND_API_KEY，未設定 → NoopEmailSender
pub email_from_address: String,           // RESEND_FROM_ADDRESS，給一個開發預設值
```

### D3. `password_reset_tokens` collection：hash-at-rest，同一張表兼職冷卻判斷

```rust
// domain.rs
pub struct PasswordResetToken {
    pub id: ObjectId,
    pub user_id: ObjectId,
    pub token_hash: String,   // SHA-256 hex digest of the raw token
    pub expires_at: DateTime, // created_at + 60 minutes
    pub used_at: Option<DateTime>,
    pub created_at: DateTime,
}
```

- **為何 hash 而不是像 session token 那樣明文存 `_id`**：session token 只活在 cookie/Authorization header 裡，reset token 會被寄到 email——經過的管道更多（信箱本身、可能的轉寄、mail server log），是更容易外洩的路徑。Token 本身已經是 256-bit 高熵、不需要 bcrypt 那種刻意慢的雜湊（bcrypt 是為了拖慢對低熵人類密碼的暴力破解，這裡不適用），SHA-256 足夠——雜湊本身不是為了防暴力破解，是為了讓「DB 外洩」跟「reset 連結外洩」變成兩個獨立事件，其中一個外洩不會自動導致另一個可被利用。
- **冷卻判斷不另開表**：`POST /auth/forgot-password` 要判斷「這個使用者是不是 60 秒內剛請求過」，直接查 `password_reset_tokens` 裡這個 `user_id` 最新一筆的 `created_at`（不限 `used_at`/是否過期），比對是否 `>= now - 60s`。不需要像 `removed_memberships` 那樣另開一張 marker collection——這張表本身的 `created_at` 就是唯一需要的時間戳。
- **不主動作廢舊 token**：使用者在冷卻期外多次請求，會累積多筆各自獨立有效（60 分鐘內）的 token。這不是安全問題（每個 token 互相獨立、不可預測），只是每封信的連結各自有效，不特別處理。

### D4. `POST /auth/forgot-password`：一律 204，fail-soft 寄信

```rust
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(req): Json<ForgotPasswordRequest>, // { email: String }
) -> ApiResult<StatusCode> {
    let email = req.email.trim().to_ascii_lowercase();
    if let Ok(Some(user)) = state.db.dashboard_users.find_by_email(&email).await {
        let recently_requested = state.db.password_reset_tokens
            .find_latest_for_user(user.id).await
            .ok().flatten()
            .is_some_and(|t| t.created_at > sixty_seconds_ago());
        if !recently_requested {
            let raw_token = session_token::generate();
            let token_hash = sha256_hex(&raw_token);
            let _ = state.db.password_reset_tokens.insert(user.id, &token_hash, RESET_TOKEN_TTL).await;
            let link = format!("{}/reset-password?token={}", state.config.admin_web_base_url, raw_token);
            if let Err(e) = state.email.send(&user.email, "重設班到密碼", &render_reset_email(&link)).await {
                tracing::warn!(?e, user_id = %user.id, "forgot-password email send failed");
            }
        }
    }
    Ok(StatusCode::NO_CONTENT)
}
```

- 找不到 user、DB 查詢失敗、冷卻中、寄信失敗——這四種情況的 HTTP 回應完全一樣（204），呼叫端（包含攻擊者）無法從回應本身區分任何一種。

### D5. `POST /auth/reset-password`：驗證 token、換密碼、踢 session

```rust
pub async fn reset_password(
    State(state): State<AppState>,
    Json(req): Json<ResetPasswordRequest>, // { token: String, new_password: String }
) -> ApiResult<StatusCode> {
    let token_hash = sha256_hex(&req.token);
    let record = state.db.password_reset_tokens.find_by_hash(&token_hash).await?
        .filter(|t| t.used_at.is_none() && t.expires_at > DateTime::now())
        .ok_or(ApiError::InvalidResetToken)?;

    if req.new_password.chars().count() < MIN_PASSWORD_LEN {
        return Err(ApiError::Validation(format!("new_password must be at least {MIN_PASSWORD_LEN} characters")));
    }

    let new_hash = password::hash(&req.new_password)?;
    state.db.dashboard_users.update_password_hash(record.user_id, &new_hash).await?;
    state.db.password_reset_tokens.mark_used(record.id).await?;
    state.db.dashboard_sessions.delete_all_by_user_id(record.user_id).await?;

    Ok(StatusCode::NO_CONTENT)
}
```

- 這裡**不**做「不洩漏資訊」的處理——呼叫端已經持有一個實體上只能從 email 收到的 token，token 無效/過期/用過直接回 `INVALID_RESET_TOKEN`（`400`）讓前端顯示「連結已失效，請重新申請」，跟忘記密碼端點的匿名威脅模型不同。
- 沿用 `handlers/app_users.rs::password_reset`、`handlers/app_auth.rs::change_password` 已經在用的密碼長度驗證慣例（`MIN_PASSWORD_LEN = 8`）。
- 成功後**不**核發新 session、**不**自動登入——前端收到 `204` 後導去 `/login`，帶一個「密碼已重設，請重新登入」的提示訊息。

### D6. 前端：兩個新的 pre-auth 頁面，`/login` 加一個連結

`pages/forgot-password.vue`、`pages/reset-password.vue` 比照 `pages/login.vue`／`pages/register.vue` 的既有模式：`definePageMeta({ middleware: 'guest', layout: false })`（未登入才能到，不套 sidebar layout——這兩頁本來就是無 Org 語境的 pre-auth 頁面）。

`pages/login.vue` 在密碼欄位下方、送出按鈕上方加一個「忘記密碼？」連結（`text-xs text-slate-500 hover:text-slate-900`，比照現有其他次要連結的視覺慣例）。

`reset-password.vue` 的 token 從網址 query string（`?token=...`）帶入，不做任何前端驗證（過期/無效交給後端判斷，前端只顯示後端回傳的錯誤訊息）。

### D7. Rate limiting 範圍：只做同使用者 60 秒冷卻

已在 proposal.md 的 Non-Goals 說明——per-IP 限制需要處理 Zeabur 後面拿真實來源 IP 的問題（`X-Forwarded-For` 之類），是額外的基礎設施決定；CAPTCHA 對這個規模的內部工具太重。60 秒冷卻已經能擋掉「對某人狂按忘記密碼」這種最直接的騷擾情境。

## Risks / Trade-offs

- **[Risk] 沒有 per-IP 限制，仍可以從單一來源對大量不同 email 各自請求一次** → 每個 email 各自只能觸發一次（60 秒內），要騷擾大量帳號仍然需要付出對應數量的請求，不是免費的放大攻擊。之後「登入失敗鎖定」機制若涵蓋更廣的濫用防護，可以回頭補強。
- **[Risk] 寄信失敗沒有重試，使用者可能永遠收不到信而不自知** → 使用者可以重新點一次「忘記密碼」（受 60 秒冷卻限制但仍然可行），且 API 端會把失敗原因記進 log，方便我們主動察覺 Resend 設定/額度問題。真正的持久化重試留給未來的 queue/scheduler 基礎設施。
- **[Risk] `RESEND_API_KEY` 忘記設定，功能悄悄失效（一律回 204 但沒人收到信）** → 這是刻意的 fail-soft 取捨（避免回應洩漏設定狀態），但代表我們需要**主動**盯 log 裡的 `forgot-password email send failed` 警告，而不是等使用者回報。上線前 DEPLOY.md 要把這個環境變數列進檢查清單。
