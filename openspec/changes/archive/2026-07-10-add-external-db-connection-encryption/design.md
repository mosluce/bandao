## Context

MSSQL provider（`api/src/auth/providers/mssql.rs`）建 tiberius `Config` 時：設了 host/port/database/auth，呼叫了 `cfg.trust_cert()`，但**沒呼叫** `cfg.encryption(...)`。tiberius `default-features = false, features = ["tds73","rustls"]` 下，`Config::new()` 的加密預設是 `EncryptionLevel::Required`。因此現況＝**強制 TLS + 無條件信任憑證**，且皆不可設定。

真實客戶 KLCC 的舊 MSSQL 不接受強制 TLS，`Required` 握手失敗。`ExternalAuthConfig`（`api/src/domain.rs`）目前欄位 `{ driver, host, port, database, username, password_encrypted, query, key_col, display_col }`，沒有加密相關設定。

## Goals / Non-Goals

**Goals:**
- 讓每個 Org 可設定 MSSQL 連線的加密層與憑證信任，涵蓋不支援 TLS 的舊機。
- 預設值對台灣 SMB 舊 MSSQL「開箱即連」，同時保留進階收緊空間。
- 既有 `external_auth` 文件無痛相容（serde default）。
- 試登入 dry-run 能用來試出對的加密組合。

**Non-Goals:**
- 不支援 MSSQL 以外 driver（維持唯一實作）。
- 不做 TDS 8.0 `Strict`（pre-login TLS）——tiberius rustls 不保證支援，且需求未出現。
- 不加自訂 CA / client cert 上傳（超出目前範圍）。
- app（Flutter）不變。

## Decisions

### D1. `encrypt` 用三值 enum，對映 tiberius
`encrypt: EncryptMode { Off, Optional, Required }` → `tiberius::EncryptionLevel::{ Off, On, Required }`。
- `Off`：不加密（舊機、TLS 關閉）。
- `Optional`（= tiberius `On`）：能加密就加密、不強制 —— 對映 ADS 的 Optional，當**預設**。
- `Required`：強制加密（現代 / 合規）。
- **為何 enum 而非 bool**：bool 只能表達 off/required，缺了「negotiate」這個對舊機最友善、最適合當預設的檔位；DBA 也熟悉 ADS 的三態心智。

### D2. `trust_server_certificate` 為 bool，取代寫死的 trust_cert
provider 改為 `if cfg.trust { tiberius_cfg.trust_cert() }`；否則走 rustls 正常憑證驗證。
- **為何**：現況無條件信任是安全 smell（等於接受任何憑證、MITM 風險）。變成明確 per-Org 開關即是改善——意圖明示，且可對有正式憑證的客戶收緊。
- 當 `encrypt = Off` 時此欄無作用（沒有 TLS 就沒有憑證）。

### D3. 預設值與遷移（serde default）
兩欄都加 `#[serde(default = ...)]`：`encrypt` 預設 `Optional`、`trust_server_certificate` 預設 `true`。
- **為何這組預設**：現況（`Required`+trust）本身就是連不上 KLCC 的元兇，沒有「保留現況」的價值。`Optional`+`trust=true` 讓舊機「能連就連」、自簽也放行，最大化 SMB 開箱可用；連不上再由 admin 調 `Off`。
- 舊文件缺這兩個 key → 反序列化套 default，零遷移腳本。

### D4. 兩欄為非機密，照常回吐
不像連線密碼（可逆加密存放、只回 `password_set`），`encrypt` / `trust_server_certificate` 是純設定值，直接存明文、在 `ExternalAuthSummaryDto` 回吐、admin-web 顯示與編輯。
- **為何**：它們不洩漏任何憑證；隱藏反而讓 admin 無法確認目前設定。

### D5. 驗證與試登入
- 存檔驗證（`validate_query_settings` 或並列的檢查）：`encrypt` 必須是三個合法值之一；`trust_server_certificate` 為 bool 天然受型別約束。
- 試登入端點不需改邏輯——它本來就用整份 `external_auth` 設定連線，新增的兩欄自動生效，等同 ADS 的「測試連線」。

## Risks / Trade-offs

- **`trust=true` 預設偏寬鬆** → 對自簽憑證接受、有 MITM 風險。以「明確開關 + 文件說明 + 進階可關」緩解；預設值優先照顧 SMB 可用性。
- **`encrypt=Optional` 的語意** → tiberius `On` 是「盡量加密」；對只支援明文的機器仍可能需手動設 `Off`。試登入可快速判定。
- **enum 值字串契約** → api（Rust enum，serde rename 成 `off`/`optional`/`required`）與 admin-web（下拉值）須一致；spec 明列三個字面值。

## Migration Plan

1. `ExternalAuthConfig` 兩欄加 serde default；既有文件反序列化即得 `optional` + `trust=true`。
2. 上線後既有 external Org（如 KLCC）預設就以 `optional` 重試——多半直接連上；若仍失敗，admin 於設定頁改 `Off` 並用試登入確認。
3. Rollback：欄位為 additive，移除即回到寫死行為；無資料破壞。

## Open Questions

- `encrypt=Optional`（tiberius `On`）在完全不支援 TLS 的機器上是否仍會嘗試握手而變慢/失敗？若實測如此，文件建議這類機器直接設 `Off`。（待實機驗證，傾向以試登入診斷。）
