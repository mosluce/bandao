## Why

`admin-web` sidebar（`add-admin-web-sidemenu` 上線）目前是一份扁平的 8 項清單（admin 視角）。使用者體感上「加入申請」跟「成員管理」是同一件事的兩面、「驗證來源」是「App 使用者」的設定入口，放在同一層級不好掃視。想依關聯性把選單整理成有主從關係的結構。

同時發現一個既有 bug：`OrgSwitcher` 的下拉選單是為舊版寬版 header 設計的（`absolute right-0` 錨定 + 固定 `w-72` 寬度），搬進 256px 寬、貼齊視窗左緣的 sidebar 後，下拉選單往左展開時會有一部分跑出視窗外——組織名稱那一截被裁掉看不到，只剩最右邊的角色徽章還在視窗內。這個 change 一併修掉。

## What Changes

### 選單改成主從結構

- `打卡看板` 移到選單最上方（獨立項目，無子項）。
- `成員管理`（連到 `/members`）底下掛一個子項 `加入申請`（連到 `/admin/join-requests`，admin-only，維持原本的待審核紅點徽章）。
- `App 使用者`（連到 `/app-users`）底下掛一個子項 `驗證來源`（連到 `/settings/auth`，admin-only）。
- 新增一個非連結的分類標籤 `進階工具`（admin-only），底下掛 `API Token`（連到 `/settings/api-tokens`）與 `冷卻管理`（連到 `/cooldowns`）兩個子項——這兩者彼此對等，沒有一個是「主項目」，所以標籤本身不可點擊、不對應任何頁面。
- `下載 App` 維持在選單最下方。
- 子項一律常駐展開，不做手風琴收合互動。
- 對 `member` 視角：`成員管理`／`App 使用者` 因為唯一的子項是 admin-only 而自然沒有子項可顯示，退化成普通的扁平連結；`進階工具` 整組（標籤 + 兩個子項）完全不出現。
- 每個角色實際看得到哪些頁面**完全不變**（跟 `add-admin-web-sidemenu` 定案的邊界一致）——這次只重新排版、分組，不調整任何存取權限。

### 修正 OrgSwitcher 下拉選單溢出視窗的 bug

- 下拉選單的外層 wrapper 從 `inline-block`（寬度隨按鈕內容縮放）改成 `block w-full`（撐滿 sidebar header 的可用寬度）。
- 下拉選單面板本身從 `absolute right-0 w-72`（固定 288px，錨定右邊界往左展開）改成 `absolute left-0 right-0`（貼合 wrapper 寬度，不設固定 px 值）。
- 效果：下拉選單永遠貼合它所在容器的寬度，不管 sidebar 或視窗多窄都不會有內容跑出可視範圍外。

## Capabilities

### Modified Capabilities

- `admin-web-nav`：「Navigation links are determined by role」這條 requirement 改寫成描述主從巢狀結構；新增一條「Org switcher popup 必須完全落在導覽面板的寬度內」的 requirement。

## Impact

- **admin-web（Nuxt）**：`layouts/default.vue`（`navItems` 的資料結構與樣板渲染邏輯）、`components/OrgSwitcher.vue`（下拉選單的定位/寬度樣式）。
- **api（Rust）**：不受影響——這次純粹是前端呈現與排版調整，`add-admin-web-sidemenu` 定案的角色存取權限（哪些 endpoint 開放給 member）完全不變，不需要新的後端測試。
- **建議套用順序**：無特別相依，`add-admin-web-sidemenu` 已經套用完成，這次是在它之上做的後續調整。
