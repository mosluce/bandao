## Context

argus 專案目前是空殼 — 只有 OpenSpec / agent 工具設定，沒有任何程式碼或 schema。本 change 是專案的第一個落地 change，必須一併決定一些「之後所有 change 都要遵循」的基礎決策（密碼雜湊演算法、session 模型、多租戶隔離模式、monorepo 切法），因此設計文件偏厚一點是合理的。

技術棧依 AGENTS.md：`api/` Rust、`admin-web/` Nuxt 3 + TypeScript（strict）、MongoDB。本 change 不動 `app/`。

## Goals / Non-Goals

**Goals**

- 建立 `Org` 與 `DashboardUser` 兩個基礎實體與其 collection / index。
- 提供 dashboard user 自行註冊（建立新 Org 或以 Org code 加入）、登入、登出、取得 `me` 的能力。
- 立起多租戶資料隔離的 pattern（auth middleware 注入 `(user_id, org_id, role)`，repository 層強制以 `org_id` 過濾）。
- 在 admin-web 上提供最小可用 UI：註冊頁（兩種模式）、登入頁、登入後骨架頁（顯示 Org 名與目前角色）。

**Non-Goals**

- AppUser 帳號系統（下個 change `add-app-user-mgmt`）。
- 打卡事件、軌跡、Org settings toggle（後續兩個 change）。
- Email 驗證、忘記密碼自助 reset、邀請連結過期、簡訊 OTP（皆 ROADMAP）。
- 國際化、無障礙、深淺主題（ROADMAP）。
- App user 的 mobile UI（不在本 change scope）。

## Decisions

### D1. 密碼雜湊：Argon2id

- **選擇**：使用 `argon2` crate（RustCrypto），參數採 OWASP 2024 推薦預設。
- **理由**：argon2id 是目前 OWASP 對新系統的首選；bcrypt/scrypt 仍可接受但屬舊代。
- **替代**：bcrypt（`bcrypt` crate）— 簡單但 work factor 上限受限。

### D2. Session：server-side opaque token + HttpOnly cookie

- **選擇**：登入成功時 server 產生隨機 token（例：32 bytes base64url），寫入 `dashboard_sessions` collection，以 `Set-Cookie` 回給瀏覽器（`HttpOnly; Secure; SameSite=Lax`）。
- **理由**：相較 JWT，token revoke 直接刪 row 即可；對內部 dashboard 來說 stateful session 的擴展負擔可忽略。HttpOnly 防 XSS 取走、SameSite=Lax 緩解 CSRF。對 state-changing endpoint 額外加 double-submit CSRF token。
- **過期**：session TTL 14 天，每次活動 sliding 延長（更新 `expires_at`）。
- **替代**：JWT — 無狀態但 revoke 麻煩、需要 blocklist 或 token version 欄位；MVP 不值。

### D3. Org code 格式：10 字元 nanoid（去除易混字）

- **格式**：10 字元，字典 `23456789ABCDEFGHJKLMNPQRSTUVWXYZ`（去掉 0/O/1/I），約 32^10 ≈ 1.1 × 10^15 組合，夠 MVP 用。
- **生成**：`nanoid` crate；新增時若碰撞（unique index 噴錯）retry 一次。
- **Rotate**：admin 在 dashboard 觸發 `POST /orgs/:id/code/rotate` → 立刻產新碼、舊碼失效。沒有寬限期。
- **邀請連結**：`https://<admin-host>/register?code=<orgCode>`，前端把 code 預填到註冊表單，使用者只需填 email + 密碼。連結本身不含 server 簽章，code 即是 auth 憑據。

### D4. Email 唯一性：全域唯一（global unique）

- **選擇**：`dashboard_users.email` 設 unique index（不分 Org）。
- **理由**：登入只用 email + 密碼，無 Org 切換 UX 設計。MVP 接受「同一個人不能在兩個 Org 各有一個 admin 帳號」的限制。
- **替代**：per-org email unique → 登入需要先選 Org（多一個欄位）。簡單度差太多，不值。
- **限制**：日後若有需要（例如同一人是 A 公司 HR 與 B 公司 admin），改 schema 加切組織 UX → ROADMAP。

### D5. 多租戶隔離：以 `org_id` 為強制 filter，repository 層守護

- **Auth middleware** 解析 cookie → 取得 session → 查 `dashboard_users` → 把 `(user_id, org_id, role)` 放進 request context。
- **Repository 層**所有 Org-scoped 查詢必須帶 `org_id` 條件。Repository 介面以 `org_id` 為第一參數，handler 傳入由 middleware 提供的值，不從 request body 拿。
- **Handler 層**只做協定 / 驗證 / 序列化，不直接拿 collection。
- 本 change 是 pattern 的第一個示範，後續 change 沿用。

### D6. 角色與授權

- 兩種角色：`admin`、`member`。儲存於 `dashboard_users.role`。
- 授權檢查放在 handler 入口（route extractor 或 explicit guard），不下沉到 repository。
- 本 change 中需要 admin 的端點：rotate Org code、把 member 升級成 admin（反之亦可，但不能把最後一位 admin 降級 — 寫一個 invariant）。
- 第一位 admin = 建 Org 的人；不可被其他 admin 移除（避免 Org 失主）→ 軟保護：刪除 user / 降級為 member 時檢查是否還有其他 admin，否則拒絕。

### D7. Monorepo 切法：扁平、各模組獨立

```
argus/
├── api/                 ← 獨立 Rust 專案 (Cargo.toml)
├── admin-web/           ← 獨立 Nuxt 專案 (package.json)
├── app/                 ← (本 change 不動，先佔位 .gitkeep)
├── openspec/
├── AGENTS.md, ROADMAP.md, README.md
└── (no root Cargo workspace, no root package.json)
```

- **理由**：模組之間共享物極少（型別由 OpenAPI codegen 生成、不從 Cargo workspace 共享），引入 workspace 反而把 toolchain 鎖在一起、增加 CI 複雜度。
- **取捨**：未來若 api/ 拆出多個 crate（如 core / web / cli），再升級為 Cargo workspace；不在第一版做。

### D8. API 表面（本 change 範圍）

| Method | Path | Auth | 用途 |
| --- | --- | --- | --- |
| POST | `/auth/register` | none | 建 Org 或加入 Org（請求體 `mode: "create" \| "join"`） |
| POST | `/auth/login` | none | email + password → set cookie |
| POST | `/auth/logout` | session | 刪 session row、清 cookie |
| GET | `/me` | session | 回 `{ user, org, role }` |
| POST | `/orgs/me/code/rotate` | session + admin | 產新 Org code、失效舊碼 |
| PATCH | `/dashboard-users/:id/role` | session + admin | 升級 / 降級角色（守護「至少一位 admin」） |

回傳格式統一 `{ data | error: { code, message } }`。錯誤碼用具名 enum（如 `EMAIL_TAKEN`、`INVALID_ORG_CODE`、`LAST_ADMIN`、`UNAUTHORIZED`）。

### D9. 測試策略

- **整合測試**為主：Rust 端用 `testcontainers-rs` 起一個 ephemeral MongoDB，每個測試案例跑在隔離 db / collection prefix。每個對外端點至少一個 happy path、必要的錯誤路徑（依 AGENTS.md）。
- **單元測試**：密碼雜湊、Org code 生成這類純函式做即可。
- **前端 e2e 不在本 change**：先有後端契約，admin-web 的端對端等到至少有「註冊 → 登入 → 看到 me」可走通的階段再加。

## Risks / Trade-offs

- **Org code 外洩 → 任意人可加入 Org**。Mitigation：admin 可 rotate；UI 上明示 code 的權限意義；ROADMAP 加「邀請連結加過期 / 一次性」。
- **無 email 驗證 → 註冊時打錯字無從救回**。Mitigation：MVP 接受；ROADMAP 補。
- **無密碼複雜度規則**。Mitigation：MVP 接受最低 8 字元；ROADMAP 補強度檢查與 breach detection。
- **Session 仰賴 cookie**：跨子網域（例如未來 admin 與 api 分屬不同網域）需要設定 `Domain` 與 CORS。第一版假設 admin-web 與 api 部在同一個父網域之下。
- **「最後一位 admin」保護**容易在多種端點重複漏寫（刪除 user、降級角色、未來轉移所有權）。Mitigation：寫一個 `try_demote_or_remove` repository 方法統一檢查。

## Migration Plan

- 全新部署，無既有資料。MongoDB collections 在 api/ 啟動時 idempotent 建立索引（如 `orgs.code` unique、`dashboard_users.email` unique、`dashboard_sessions.expires_at` TTL）。
- 回滾：drop 三個 collection；本 change 上線前後資料量為 0 / 個位數，不需要遷移腳本。

## Open Questions

- **(D4) Email 全域唯一是否真的接受？** 若有兼任 / 跨組織需求顯著，需走 per-Org 路線並設計切組織 UX。建議第一版先全域唯一，被使用者打臉再改。
- **Session sliding refresh 的更新頻率**：每個 request 都更新會吵 DB；可改成「離 expires 還剩 < 50% 才更新」之類的 lazy 策略。實作時定一個小常數即可，不卡 design。
- **Rate limiting**：register / login 端點要不要加 rate limit？MVP 可不做，但若上線前感覺太裸，補一個簡單的 IP-based 計數器；本文件不定。
