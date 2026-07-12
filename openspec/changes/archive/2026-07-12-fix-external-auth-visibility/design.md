## Context

`OrgDto::from_org(org: &Org) -> OrgDto`（`src/handlers/auth.rs`）是唯一一個把 `Org` 轉成對外 DTO 的函式，六個呼叫點共用：

| 呼叫點 | 端點 | 呼叫者已知角色？ | 現況 |
|---|---|---|---|
| `auth.rs::build_auth_response`（membership 迴圈） | `GET /me` 等 | 有——每筆 `(Membership, Org)` pair 裡的 `m.role` | 洩漏 |
| `auth.rs::build_auth_response`（current_org） | 同上 | 有——已算好的 `role` 變數 | 洩漏 |
| `app_auth.rs`（login） | `POST /app/auth/login` | 無 dashboard role 概念，AppUser 一律非 admin | 洩漏 |
| `app_auth.rs`（me） | `GET /app/me` | 同上 | 洩漏 |
| `orgs.rs::transfer_owner` | `POST /orgs/me/owner` | 端點本身 `RequireAdmin` | 安全（呼叫者必為 admin） |
| `external_auth.rs::configure` | `POST /orgs/me/external-auth` | 端點本身 `RequireAdmin` | 安全（呼叫者必為 admin） |

## Goals / Non-Goals

**Goals:**
- `external_auth` 欄位只在呼叫者是該 Org 的 dashboard admin 時出現在任何 API 回應裡。
- 修法要讓「以後新增一個回傳 `OrgDto` 的呼叫點」很難不小心又漏出去——不是每個呼叫點各自記得判斷，而是建構函式本身逼呼叫者交代角色。

**Non-Goals:**
- 不處理 admin-web／app 前端的顯示邏輯——它們本來就要能處理欄位不存在的情況。
- 不改 `RequireAdmin`／`RequireActiveOrg` 的既有守門機制，這次只動「欄位要不要出現在回應裡」。
- 不處理 `add-admin-web-sidemenu` change 裡「member 能不能透過 UI 看到驗證來源摘要」的問題——那邊已經決定維持 admin-only，跟這裡的 API 層修正方向一致，但兩個 change 各自獨立交付。

## Decisions

### D1. `OrgDto::from_org` 簽章改成強制帶角色資訊
把單一 `from_org(org)` 換成兩個明確意圖的建構函式：

```rust
impl OrgDto {
    /// Caller is a confirmed dashboard admin of this Org.
    pub fn from_org_as_admin(org: &Org) -> Self { /* includes external_auth */ }

    /// Caller is anything else (member, AppUser, or role unknown/未定).
    pub fn from_org_as_non_admin(org: &Org) -> Self { /* omits external_auth */ }
}
```

- **為何不做一個 `from_org(org, role: Option<Role>)` 單一函式**：兩個獨立命名的函式讓呼叫點自己要交代「我現在是用哪個身份在組裝」，比傳一個 `Option<Role>` 進去、內部再判斷更難不小心传錯——尤其 `app_auth.rs` 那邊根本沒有 `Role` 型別可傳，硬塞 `None` 語意上不夠直接。呼叫 `from_org_as_non_admin` 不需要呼叫者知道任何 Role 相關型別，讀起來就是「這裡我確定不是 admin 視角」。
- **為何不在 `OrgDto` 序列化時依某個 thread-local／context 隱性過濾**：隱性狀態會讓「這個回應到底有沒有 external_auth」變成要跳到別處才看得懂，違反這個 codebase 一貫「HTTP handler 顯式決定回什麼」的風格；顯式建構函式讓每個呼叫點自己讀起來就交代清楚。

### D2. 舊的 `from_org` 直接移除，不留相容 shim
六個呼叫點全部改成呼叫對應的新函式，舊名字整個刪掉，而不是留一個「預設不含 external_auth」的 `from_org` 相容別名。
- **為何**：留一個「安全預設」的舊名字，看似保守，實際上是把「以後有人手滑用回舊函式」這個風險換成「以後有人手滑忘記改成 admin 版本、資料該顯示卻不顯示」——後者是功能性 bug 容易在測試中被抓到，比前者這種悄悄的安全洩漏更容易被發現，兩者相權取其輕。而且六個呼叫點已知、範圍夠小，一次改完不需要過渡期。

## Risks / Trade-offs

- **[Risk] 未來如果又新增一個回傳 `OrgDto` 的端點，寫的人可能還是選錯函式** → Mitigation：兩個函式命名刻意做到唯讀函式名稱就是決策點，加上這次會補齊的整合測試（member/AppUser 情境斷言回應完全不含 `external_auth` 鍵）能在 CI 抓到任何回歸。
