## Context

`ExternalAuthConfig`（`api/src/domain.rs`）目前存的 `query` 欄位是一個「驗證特定帳密」的查詢，`mssql.rs::resolve_identity()` 把 `@account`/`@password` 換成 tiberius 的 `@P1`/`@P2` 位置參數綁進去，每次登入都即時打一次。這個 query 沒辦法「拔掉帳密條件、變成列出全部」——真實客戶的 query 常常混了業務邏輯篩選（例如 `AND ZLEVEL != '00' AND APSYSNO = 'EPURCSYS'`），沒有安全的字串處理方式能分辨哪些 `AND` 子句是帳密比對、哪些是業務篩選。

`db/app_users.rs::upsert_shadow()` 是現有的「登入時就地建立/更新影子使用者」方法，不論新建還是更新都會把 `last_login_at` 蓋成現在時間——這對登入語意是對的，但同步不能沿用，否則會把「從沒登入過」的人標成「剛剛登入」。

## Goals / Non-Goals

**Goals:**
- Admin 可以在任何人登入之前，主動把外部系統的使用者名單同步進本地 `app_users`。
- 同步的資料寫入行為跟登入時的 `upsert_shadow()` 語意分離——尤其是 `last_login_at` 不能被同步動到。
- 個別列資料錯誤要能容錯（跳過該列），但整體設定錯誤（連線失敗、找不到欄位）要整批失敗，不能寫入部分不可信的資料。

**Non-Goals:**
- 不做「本地存在、同步結果消失的使用者自動停用」——這個 change 刻意選擇純新增/更新，理由見 Decisions D3。
- 不做同步結果的「預覽再確認」流程——`list_query` 本身是唯讀 SELECT，同步動作只會新增/更新本地資料（不刪除），風險可控，直接執行 + 事後摘要就夠。
- 不處理背景排程自動同步——這次只做「admin 手動觸發」，跟 ROADMAP 上還沒做的 queue/scheduler 基礎設施無關，也不依賴它。

## Decisions

### D1. `list_query` 是獨立欄位，驗證規則跟 `query` 相反

```rust
// domain.rs — ExternalAuthConfig 新增欄位
pub struct ExternalAuthConfig {
    // ...既有欄位不動...
    pub query: String,           // 驗證帳密用，必須含 @account / @password
    pub key_col: String,
    pub display_col: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_query: Option<String>,  // 同步用，不帶參數；未設定時同步功能不可用
}
```

`auth/providers/mod.rs` 新增一個 `validate_list_query_settings`，跟現有 `validate_query_settings`的規則相反：

```rust
pub fn validate_list_query_settings(driver: &str, list_query: &str) -> Result<(), String> {
    if driver != SUPPORTED_DRIVER {
        return Err(format!("unsupported driver: {driver}"));
    }
    if list_query.contains("@account") || list_query.contains("@password") {
        return Err("同步查詢不應包含 @account / @password 佔位符".to_string());
    }
    Ok(())
}
```

- **為何獨立欄位、不是同一個 `query` 想辦法兼容兩種用途**：這兩個查詢的執行方式完全不同（一個綁參數驗證單筆、一個不綁參數列出多筆），硬要共用一個欄位只會讓兩種語意互相污染。
- `key_col`/`display_col` 沿用既有欄位，不用為 `list_query` 另外設一組——两個查詢回傳的資料形狀是一致的（同一組「唯一識別欄」「顯示名稱欄」）。

### D2. 同步用一個新的 repository 方法，不沿用 `upsert_shadow()`

```rust
// db/app_users.rs
pub struct SyncOutcome {
    pub external_key: String,
    pub kind: SyncOutcomeKind, // Created | Updated
}

pub async fn sync_upsert_shadow(
    &self,
    org_id: ObjectId,
    external_key: &str,
    display_name: &str,
) -> ApiResult<SyncOutcome> {
    let now = DateTime::now();
    // 只更新 display_name / updated_at，不碰 last_login_at。
    let updated = self.coll.find_one_and_update(
        doc! { "org_id": org_id, "external_key": external_key },
        doc! { "$set": { "display_name": display_name, "updated_at": now } },
    ).await?;
    if updated.is_some() {
        return Ok(SyncOutcome { external_key: external_key.to_string(), kind: SyncOutcomeKind::Updated });
    }
    // 新建：status = active，needs_password_change 對外部使用者本來就無意義（沿用
    // upsert_shadow() 既有的 false 慣例），last_login_at = None。
    let user = AppUser {
        id: ObjectId::new(),
        org_id,
        username: None,
        username_lower: None,
        display_name: display_name.to_string(),
        password_hash: None,
        auth_source: AppUserAuthSource::External,
        external_key: Some(external_key.to_string()),
        status: AppUserStatus::Active,
        needs_password_change: false,
        last_login_at: None,
        created_by_dashboard_user_id: None,
        created_at: now,
        updated_at: now,
    };
    self.coll.insert_one(&user).await?;
    Ok(SyncOutcome { external_key: external_key.to_string(), kind: SyncOutcomeKind::Created })
}
```

- **為何不在 `upsert_shadow()` 裡加一個 `touch_last_login: bool` 參數**：同步跟登入是兩個目的完全不同的呼叫端（一個是 admin 觸發的批次維運操作，一個是使用者登入的即時驗證路徑），把兩者的行為差異壓進同一個函式的參數裡，之後任何一邊改邏輯都要小心不要動到另一邊。分成兩個獨立、各自命名清楚的方法，理解成本更低。

### D3. 同步是純新增/更新，本地多出來的使用者不動

同步結果沒出現的 `external_key`，不做任何處理——不停用、不刪除、不標記。
- **為何**：外部系統仍然是驗證的最終權威，離職員工真的嘗試登入時，`resolve_identity()` 對外部資料庫的即時查詢自然就會找不到人、驗證失敗——這個防線已經存在，不需要同步功能重複負責。如果 `list_query` 設定得比登入用的 `query` 篩選範圍更窄（例如少了某個業務邏輯條件），連動停用反而可能誤傷真正還在職、有效的使用者。純新增/更新是風險比較低的預設行為。

### D4. 個別列容錯，整批設定錯誤則全部失敗

```rust
pub struct SyncResponse {
    pub total_rows: usize,
    pub created: usize,
    pub updated: usize,
    pub skipped: Vec<SkippedRow>,
}
pub struct SkippedRow {
    pub row_index: usize,
    pub reason: String,
}
```

執行順序：先跑 `list_query`拿到整個結果集 → 檢查 `key_col`/`display_col` 這兩個欄位名稱是否存在於結果集的欄位定義裡（用跟 `mssql.rs::column_string()` 一樣的「欄位不存在 vs 欄位是 NULL」判斷）→ 如果欄位名稱本身就不存在，整個請求回錯誤，不寫入任何東西（這是設定問題，不是資料問題）→ 欄位存在的前提下，逐列處理：`key_col` 是 NULL 或空字串 → 記進 `skipped`、跳過該列；否則呼叫 `sync_upsert_shadow()`。

- **為何这樣分層**：欄位名稱不存在代表 `list_query`/`key_col`/`display_col` 三者對不上，是「這次同步的設定本身就是壞的」，寫入任何一筆都不可信，直接整批擋下來，跟 admin 說清楚是設定問題。個別列的 NULL/空值則是「資料本身有髒東西」（外部系統裡某筆記錄本來就缺欄位），不代表設定錯誤，值得容錯跳過、讓其他乾淨的列照常同步進來。

### D5. `POST /orgs/me/external-auth/sync`：admin-only，僅 `auth_source == external_db` 時可呼叫

跟 `configure`/`test_login` 一樣掛 `RequireAdmin`。額外檢查 `org.auth_source() == OrgAuthSource::ExternalDb`，否則回一個新的 `ApiError::ExternalAuthNotEnabled`（`409 EXTERNAL_AUTH_NOT_ENABLED`）。

- **為何不沿用既有的 `ApiError::ExternalAuthMode`**：那個錯誤的既有語意跟 doc comment 講得很明確，是反方向的——「Org 目前是 `external_db` 模式，擋掉內部限定的操作」（`app_users.rs::ensure_internal_auth`）。同步要擋的是相反方向：「Org 目前不是 `external_db` 模式，擋掉外部限定的操作」。兩個方向共用一個變體會讓 doc comment 跟實際觸發情境對不上，之後看 code 的人容易誤解，新增一個名字精確對應情境的變體比較符合這個 codebase 一貫「每個情境各自一個具名錯誤」的風格。

- **為何限制在 `auth_source == external_db`，而不是「只要存了 `external_auth` 設定就能同步」**：這是使用者在探索階段明確拍板的範圍——避免 admin 在 `internal` 模式下，一邊維護內部使用者、一邊誤觸同步把外部系統的名單混進來造成混淆。

## Risks / Trade-offs

- **[Risk] `ExternalAuthMode` 這個既有錯誤被兩個方向重複利用（外部模式下擋內部操作、內部模式下擋同步操作）** → 語意仍然一致（「操作跟目前的 auth_source 不吻合」），不新增一個幾乎重複的錯誤碼；如果之後這兩個方向的訊息需要分開措辭，再拆成兩個變體也不遲。
- **[Risk] `list_query` 沒有 dry-run/預覽機制，設定錯了要等真的按下「同步」才會發現** → 已知取捨（見 Non-Goals）。可以緩解的方式：`skipped` 摘要本身就是一種事後回饋，設定明顯錯誤時（例如整批都被跳過）admin 一眼就能看出來要回頭檢查設定。
