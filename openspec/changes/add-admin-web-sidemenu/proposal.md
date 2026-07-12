## Why

admin-web 完全沒有共用的 layout（`layouts/` 目錄不存在），header／`OrgSwitcher`／「回首頁」這組導覽 markup 目前手動複製貼上在至少 10 個頁面裡。管理員工具那排橫向按鈕已經長到 8 顆、一行快排不下（成員管理／App 使用者／驗證來源／API Token／打卡看板／冷卻管理／加入申請／下載App），每加一個新功能就要在每個頁面各補一次連結，是持續累積的維護債。

同時，`member` 角色目前的 admin-web 體驗幾乎是空的——`members.vue`、`checkin/*`、`app-users/*`、`cooldowns.vue`、`settings/*`、`admin/join-requests.vue` 每一頁最上面都寫死「非 admin 就導回首頁」，即使後端某些讀取（例如成員清單）其實早就沒有擋 member。藉這次重新設計選單的機會，把「member 應該能唯讀看到哪些東西」一併定案、一起交付——不然 sidemenu 上線時 member 版本還是空的，又要再等一次改動才補齊。

## What Changes

### 前端：導覽整併進共用 layout

- 新增 `layouts/default.vue`：左側 sidemenu（`OrgSwitcher` + 導覽項目列表）+ 右側頁面內容 `<slot />`。導覽項目依角色（admin/member）動態決定要顯示哪些。
- 支援窄螢幕：sidebar 預設收合、有開關按鈕（漢堡選單樣式），跟現有 hand-rolled Tailwind 風格一致，不引入 UI 元件庫。
- 「加入申請」的待審核紅點徽章（30 秒輪詢）從現在的 pill 按鈕搬進 sidemenu 項目，行為不變。
- 一次遷移所有現有頁面（`index.vue`、`members.vue`、`cooldowns.vue`、`app-users/index.vue`、`checkin/index.vue`、`checkin/[appUserId]/index.vue`、`checkin/[appUserId]/trajectory.vue`、`settings/auth.vue`、`settings/api-tokens.vue`、`admin/join-requests.vue`、**`orgs/new.vue`、`orgs/join.vue`**）——移除各自複製貼上的 header/`OrgSwitcher`/「回首頁」markup，改用新 layout。後面這兩頁雖然屬於「沒有 active Org 也能到」的 org-agnostic 頁面，但使用者可能已經有其他 org、只是還沒切過去，套 layout 讓 OrgSwitcher 常駐合理。`login.vue`、`register.vue`、`no-org.vue`、`privacy.vue`、`download.vue` 維持現狀，不套用這個 layout（真正沒有 org 語境的 pre-auth／zero-Org 頁面）。

### 後端：member 開放特定讀取權限

依「資料開放、config 鎖住」原則鬆綁部分 GET endpoint 從 `RequireAdmin` 降級為 `RequireActiveOrg`（任何已登入成員，不限角色），對應的異動類 endpoint 全部維持 `RequireAdmin` 不變：

| 能力 | GET（開放給 member） | 異動（維持 admin only） |
|---|---|---|
| 成員管理 | `GET /dashboard-users`（已經開放，無需改動） | 移除成員、改角色、清冷卻 |
| App 使用者 | `GET /app-users`（**這次鬆綁**） | 建立、編輯、重設密碼 |
| 打卡看板／歷史／軌跡 | `GET /checkin/users`、`GET /checkin/users/:id/events`、`GET /checkin/users/:id/locations`（**這次鬆綁**） | 強制收班、`PATCH /orgs/me/settings`、`GET /checkin/users/:id/locations/export`（xlsx 匯出） |

以下維持 admin-only、**不開放 member 讀取**（已確認的邊界）：冷卻管理、加入申請列表、API Token、驗證來源（含 `fix-external-auth-visibility` 那個 change 要修的 `/me` 洩漏問題——兩個 change 方向一致）。

### 前端：member 版頁面呈現

`members.vue`、`app-users/index.vue`、`checkin/index.vue`（+ 對應子頁）移除「非 admin 一律導回首頁」的守衛，改成頁面本身可以進得去、正常渲染列表，但所有異動用的按鈕／表單依 `auth.isAdmin.value` 隱藏——沿用 `app-users/index.vue` 現有處理 external-auth 模式時「唯讀 vs 可操作」的既有寫法（`isExternal`-style 條件渲染），不是新發明一套模式。

## Capabilities

### New Capabilities
- `admin-web-nav`：共用 layout、依角色決定的導覽結構、RWD 收合行為。

### Modified Capabilities
- `app-user-mgmt`：`GET /app-users` 開放給 member 讀取；異動端點的角色限制不變。
- `checkin-events`（或對應的 admin board capability）：`GET /checkin/users`、`GET /checkin/users/:id/events` 開放給 member 讀取；異動端點不變。
- `location-tracking`：`GET /checkin/users/:id/locations` 開放給 member 讀取（個人軌跡頁的資料來源）；`GET /checkin/users/:id/locations/export`（xlsx 匯出）維持 admin-only。

## Impact

- **api/（Rust）**：`handlers/app_users.rs::list`、`handlers/checkin.rs::list_users`、`handlers/checkin.rs::list_user_events`、`handlers/location_tracking.rs::list_locations` 的 extractor 從 `RequireAdmin` 換成 `RequireActiveOrg`，handler 內部原本假設「呼叫者一定是 admin」的邏輯要重新檢查（目前應該沒有，這幾個都是純讀取）。
- **admin-web（Nuxt）**：新增 `layouts/default.vue` + 可能的 sidebar 子元件；上述 10 個頁面移除重複 header markup、改用 layout；`members.vue`／`app-users/index.vue`／`checkin/*` 移除 admin-only 守衛、改成條件式渲染異動按鈕。
- **建議套用順序**：先套用 `remove-org-code-rotation`，再套用這個 change——原因見那個 change 的 proposal（兩者動到同一塊即將被整段重寫的 markup）。
- **不受影響**：`fix-external-auth-visibility` 各自獨立處理 API 層的洩漏修正，這次不重複處理，但兩者對「驗證來源 admin-only」這個邊界的認定一致。
