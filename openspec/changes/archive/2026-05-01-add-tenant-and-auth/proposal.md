## Why

argus 是一個多租戶（multi-tenant）的簽到系統，所有後續功能（AppUser 管理、打卡事件、軌跡）都必須先有「組織（Org）」與「能登入 dashboard 的人」這兩個基礎物件，否則無從談權限歸屬與資料隔離。這次 change 只負責立起這個地基，讓後續三個 change（AppUser 管理、打卡事件、軌跡）能在其上開展。

## What Changes

- 新增 `Org` 實體作為租戶邊界，含長期 Org code（可由 admin rotate）與 organization-level settings 容器。
- 新增 `DashboardUser` 帳號系統，採 email + 密碼登入；自行註冊。
- 註冊流程支援兩種落地：
  - **建立新 Org**：註冊者自動成為該 Org 的第一位 admin。
  - **加入既有 Org**：透過 invite link（其底層為 Org code 的 URL 包裝）或註冊時手填 Org code，落入該 Org，預設角色 `member`。
- 兩種角色：`admin`（可管 Org settings、可在後續 change 中 CRUD AppUser、強制收班、改 toggle）與 `member`（讀取為主，無 Org 管理權）。Member 升級成 admin 由既有 admin 操作。
- DashboardUser 與 AppUser 完全分離（兩套帳號系統、兩條認證路徑），AppUser 的部分由後續 change 處理，本次不做。
- 第一版不做 email 驗證、不做密碼自助 reset、不做邀請連結過期；以最小可用為目標。

## Capabilities

### New Capabilities

- `org-tenancy`: 組織（租戶）實體、Org code、Organization-level settings 容器、admin rotate code 的能力。
- `dashboard-auth`: DashboardUser 註冊（建立 Org 或以 Org code 加入）、登入、登出、session 管理；admin/member 角色與升級。

### Modified Capabilities

<!-- None. This is the first change; no existing capabilities to modify. -->

## Impact

- **新增模組**：`api/`（Rust）首次落地，包含 HTTP 路由、認證中介層、Org 與 DashboardUser repository、密碼雜湊、session/JWT。
- **新增前端**：`admin-web/`（Nuxt）首次落地，含註冊頁、登入頁、Org 切換 / 邀請碼入口、首頁骨架。
- **新增資料層**：MongoDB 中新增 `orgs`、`dashboard_users` collections 與必要索引（如 `dashboard_users.email` 唯一、`orgs.code` 唯一）。
- **後續依賴**：`add-app-user-mgmt`、`add-checkin-events`、`add-location-tracking` 都將依賴本 change 提供的 Org context 與 admin 認證；本 change 上線前其他 change 無從實作。
- **不影響**：`app/`（Flutter）本次不動。
