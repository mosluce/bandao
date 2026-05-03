## Why

The auto-generated 10-character `org_code`（例如 `HFQWMS7VZB`）對 admin 來說既難記也難在口頭分享、印刷品、行銷文案上使用。同時這個 code 又是 join 的安全屏障（32^10 ≈ 10¹⁵，brute-force join URL 不可行），單純把它換成「自選短字串」會犧牲安全層。

把「security identifier」與「人類介面」拆開：保留 random `org_code` 不動，新增可選的 vanity `slug`，admin 可隨時設定 / 變更 / 清除。slug 落地後 join 兩種 input 都吃，invite link 預設用 slug，看起來更體面。

## What Changes

- 新增 `Org.slug`（可選、lowercase `^[a-z0-9]{2,24}$`、跨 active + grace 全域唯一）
- 新增 `Org.slug_changed_at`（rate limit 用）
- 新增 slug grace history（30 天 grace period 期間舊 slug 仍可 join、且鎖在原 Org）
- 新增 endpoint `POST /orgs/me/slug`（admin only，set / update）
- 新增 endpoint `DELETE /orgs/me/slug`（admin only，清除 → 進 grace）
- 修改 `POST /auth/register {mode: "join"}`：`org_code` 欄位同時接受 random code、active slug、grace slug，依 input format 路由 lookup
- 新增 ApiError variants：`InvalidSlugFormat` / `SlugReserved` / `SlugTaken` / `SlugChangeTooSoon`
- 新增 reserved word list 拒絕 API path 第一層 / 系統保留字 / 專案名（`argus`）作為 slug
- admin-web `/`：並列顯示 code（不可變）+ slug（可編輯/清除），invite link 在有 slug 時優先用 slug
- 限制：first-time SET 免限制；後續 SET / CHANGE / DELETE 每 30 天只能一次（與 grace 對齊，Org 同時最多持有 2 個 slug）

## Capabilities

### New Capabilities
（無）

### Modified Capabilities
- `org-tenancy`: 新增 vanity slug 子能力（slug 設定 / 清除 / grace period / rate limit）；`POST /auth/register` 的 join input 行為由「只接受 code」放寬成「接受 code 或 slug」

## Impact

- **API**：`api/src/domain/org.rs` 加欄位；`api/src/db/orgs.rs` 加 slug 操作 + lookup；`api/src/handlers/orgs.rs` 加兩個 endpoint；`api/src/handlers/auth.rs` 的 join 路徑改走新 lookup；`api/src/error.rs` 加 4 個 variants；可能新增 `api/src/auth/slug.rs` 放 reserved list + format validate
- **DB**：`orgs` collection 加 `slug`、`slug_changed_at`，加 sparse unique index on `slug`；`org_slug_history` 新 collection（或 embedded array）+ TTL index 自動清 grace
- **admin-web**：`pages/index.vue` 新 UI 區塊；`composables/useApi.ts` 加 `setOrgSlug` / `clearOrgSlug`；`types/api.ts` 同步 DTO；error handling 對應新 variants
- **測試**：set-slug 各分支（happy / format / reserved / taken / rate-limit）、change → grace → 第三方搶被擋、grace 過期釋出、register-by-slug、register-by-grace-slug、UI 顯示
- **Spec**：`openspec/specs/org-tenancy/spec.md` 加 4 個新 requirements + 改 1 個既有 requirement
- **不影響**：`code` 本身、`POST /orgs/me/code/rotate`、dashboard-auth spec、login / logout 流程
