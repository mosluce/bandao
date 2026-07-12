## Why

Org 的「輪替組織代碼」功能（`POST /orgs/me/code/rotate`）評估後判斷實質沒有使用場景，決定移除。這是刻意接受的取捨：拿掉之後，如果 org code 外流，將沒有任何補救手段（只能重建整個 Org）；這個殘留風險預計由 ROADMAP 中「登入失敗鎖定」（連續失敗 N 次鎖定、admin 可解鎖）機制長期緩解——rate limit 上線後，光是取得 org code 也無法拿來做無限次數的密碼猜測，外流的實際危害大幅降低，拿掉 rotate 這個很少被用到的安全閥是合理的交換。

## What Changes

- 移除 `POST /orgs/me/code/rotate` endpoint 與對應的 repository 方法。
- 移除 admin-web 首頁「管理員工具」區塊裡的「輪替組織代碼」按鈕與確認流程。
- `org.code` 本身**不受影響**——新建 Org 時仍會產生隨機 code，join 流程、App 登入解析 code 的邏輯都維持原樣；這次只拿掉「換發新代碼」這個動作，不動 code 的產生與使用機制。
- `org-tenancy` spec 移除對應的 Requirement；`dashboard-auth` spec 裡拿這個 endpoint 當範例的地方換成其他仍存在的 org-scoped endpoint。

## Capabilities

### New Capabilities
（無）

### Modified Capabilities
- `org-tenancy`：移除「Admin can rotate the Org code」這條 requirement 與對應 scenario。
- `dashboard-auth`：「Org-scoped endpoints reject calls with no active Org」這條 requirement 的 scenario 範例文字換掉引用的 endpoint（原本舉 `POST /orgs/me/code/rotate` 為例，改舉其他仍存在的 org-scoped endpoint），不改變這條 requirement 本身要驗證的行為。

## Impact

- **api/（Rust）**：刪除 `handlers/orgs.rs::rotate_code`、`db/orgs.rs::rotate_code`、`handlers/mod.rs` 裡的路由註冊、`RotateCodeResponse` DTO；刪除 `tests/orgs_rotate.rs`。`auth::org_code` 模組（`generate()` / `is_well_formed()`）**保留**——新建 Org 時仍要用它產生初始 code。
- **admin-web（Nuxt）**：`pages/index.vue` 移除輪替相關的 state（`rotating`、`showRotateConfirm`、`rotateError`）、`rotateCode()` 函式、對應的按鈕與確認 UI；`types/api.ts` 移除 `RotateCodeResponse`。
- **建議套用順序**：這個 change 動到的 `pages/index.vue`「管理員工具」區塊，跟 `add-admin-web-sidemenu` change 要整塊重寫的區塊**完全重疊**。建議**先套用這個 change、再套用 sidemenu 那個**——sidemenu 那邊的改動範圍本來就會整段搬移/重寫這塊 markup，如果順序反過來，這個 change 要動的程式碼可能已經被 sidemenu 改動搬到別處或刪掉，diff 會對不上。
