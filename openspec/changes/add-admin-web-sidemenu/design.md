## Context

`admin-web` 目前沒有 `layouts/` 目錄，每個需要導覽的頁面各自在自己的 `<template>` 裡手刻一份 `<header><OrgSwitcher />...回首頁...</header>`，至少 10 個頁面重複這個模式。「管理員工具」那排橫向 pill 按鈕（在 `pages/index.vue`）現在有 8 顆，一行快放不下，而且只有 admin 看得到——member 現在唯一能碰的是首頁的「組織資訊」（唯讀）跟「離開組織」兩塊，其他管理頁面一律 `if (!auth.isAdmin.value) navigateTo('/')` 彈回首頁，即使某些讀取後端其實沒擋（`GET /dashboard-users` 已經是 `RequireActiveOrg`）。

`fix-external-auth-visibility` 這個平行的 change 處理的是 API 層「`external_auth` 不該外流給非 admin」的漏洞，這裡採用同一個「驗證來源對 member 不可見」的邊界認定，但不重複實作。

## Goals / Non-Goals

**Goals:**
- 一個共用 `layouts/default.vue` 取代目前 10 個頁面各自複製的 header markup，一次遷移完，不留過渡期兩套並存。
- Sidemenu 導覽項目依角色（admin／member）動態決定，member 版本不是空的——能讀取的頁面（成員管理、App 使用者、打卡看板）都要能從 sidemenu 連過去。
- 支援窄螢幕（RWD），sidebar 可收合。
- 後端對應鬆綁：讓 sidemenu 承諾能連到的頁面，實際打得開、讀得到資料。

**Non-Goals:**
- 不重新設計每個頁面「內容」本身的呈現（表格、表單樣式維持現狀），這次只動導覽層跟「member 能不能碰到這個頁面」。
- 不處理 `輪替組織代碼` 的移除——那是 `remove-org-code-rotation`，本 change 假設它已經先套用。
- 不修 `external_auth` 的 API 層洩漏——那是 `fix-external-auth-visibility`，本 change 只確保 UI 層一致地不對 member 顯示驗證來源頁面。
- 不做「member 可以唯讀看到打卡設定 toggle 目前狀態」——沿用先前討論的決定，打卡設定整頁維持 admin-only，member 完全不可見（不是唯讀顯示）。

## Decisions

### D1. 一次遷移所有頁面，不做漸進式並存
10 個頁面在同一個 change 裡全部改用新 layout、移除自己的 header markup。
- **為何**：漸進式遷移代表有一段時間「有些頁面用新 sidemenu、有些頁面還是舊的橫向按鈕」，使用者體驗不一致、程式碼也要同時維護兩套導覽邏輯。範圍雖然大，但每個頁面的改動都是「刪掉重複的 header、其餘內容不動」，機械性高、風險可控。

### D2. Sidemenu 導覽清單是 layout 裡的單一計算屬性，不是各頁面各自決定
```
layouts/default.vue
  navItems = computed(() => {
    const base = [
      { to: '/members', label: '成員管理' },
      { to: '/app-users', label: 'App 使用者' },
      { to: '/checkin', label: '打卡看板' },
    ]
    if (auth.isAdmin.value) {
      base.push(
        { to: '/cooldowns', label: '冷卻管理' },
        { to: '/admin/join-requests', label: '加入申請', badge: pendingJoinCount },
        { to: '/settings/auth', label: '驗證來源' },
        { to: '/settings/api-tokens', label: 'API Token' },
      )
    }
    base.push({ to: '/download', label: '下載 App' })
    return base
  })
```
- **為何**：現在的問題就是「每加一個功能要在 N 個頁面各補一次連結」，把清單收斂到一個地方，之後加新功能只改一處。
- Member 看到的清單天然就是 admin 清單的子集（成員管理／App 使用者／打卡看板／下載App），不需要另外維護一份「member 專用清單」。

### D3. RWD：收合式 sidebar，不做兩套獨立版面
Desktop（`md:` 以上）sidebar 常駐展開；窄螢幕預設收合，頂部一顆漢堡按鈕切換，用 Tailwind 的 `translate-x` + 遮罩層做滑出效果（跟現有專案「純 Tailwind、不引入元件庫」的慣例一致，不用額外裝 headless UI 套件）。
- **替代方案**：desktop 跟 mobile 各寫一套完全獨立的導覽元件——否決，維護兩套邏輯的成本高於做一個響應式版本。

### D4. 後端鬆綁範圍：只動四個讀取端點，異動端點完全不碰
`app_users.rs::list`、`checkin.rs::list_users`、`checkin.rs::list_user_events`、`location_tracking.rs::list_locations` 的 extractor 從 `RequireAdmin` 換成 `RequireActiveOrg`（第四個是 apply 階段補上的，見下方 Resolved Questions 的補充說明）。其餘所有 endpoint（含這四個能力底下的異動操作，以及 `location_tracking.rs::export_locations`）不變。
- **為何範圍卡得這麼緊**：`RequireActiveOrg` 這個「任何已登入成員」的 extractor 已經存在、已經在用（`dashboard-users` 清單），不是新建權限機制，改動被壓縮成「换一行 extractor」等級，審查與測試都容易對焦。
- **為何不順便檢查這三個 handler 內部有沒有偷偷假設「呼叫者是 admin」**：需要，但這是機械性檢查（read 路徑通常不會有這種假設），tasks.md 會列成明確項目，不是設計層級的不確定性。

### D5. Member 唯讀渲染：沿用既有的條件渲染模式，不新發明機制
`app-users/index.vue` 已經有處理 `auth.currentOrg.value?.auth_source === 'external_db'` 時「隱藏建立/重設密碼、只顯示唯讀欄位」的 `isExternal` 條件渲染寫法。`members.vue`／`app-users/index.vue`／`checkin/*` 改成用同樣的模式，把判斷條件換成 `!auth.isAdmin.value`：異動按鈕、表單一律 `v-if="auth.isAdmin.value"`，資料列表本身維持渲染。
- **為何**：這個 codebase 已經有一次處理過「同一個頁面，依某個條件決定能唯讀還是能操作」的先例，直接套用比發明新模式一致性更高。

### D6. 移除「回首頁」，頁面內容不再需要自己的 `<header>`
Sidebar 常駐可見，「回首頁」這個顯式連結變得多餘——首頁本身就是 sidebar 裡的一個項目（或 logo/組織名稱的點擊目標）。各頁面原本 `<header>` 裡的頁面標題（例如「成員管理」的 `<h1>`）保留，但不再包 `OrgSwitcher` 或「回首頁」，退化成單純的頁面標題列。

## Risks / Trade-offs

- **[Risk] 範圍大，一次改十個頁面，PR 審查與測試面都不小** → Mitigation：每個頁面的改動模式高度一致（拔掉重複的 header、套 layout），tasks.md 會逐頁列出，方便分段驗證；`pnpm typecheck` + `pnpm test` + `pnpm build` 在 apply 流程裡就會抓大部分結構性錯誤。
- **[Risk] Member 讀取權限鬆綁後，如果哪個 handler 內部其實有未察覺的「假設呼叫者是 admin」邏輯，會變成資料外洩** → Mitigation：D4 already 限定範圍在三個確定是純讀取的 handler；tasks.md 會要求逐一確認 handler 內部邏輯後才动手，並補上「member 呼叫時回應內容跟 admin 呼叫時只差在有沒有異動按鈕，資料本身完全一致」的整合測試。

## Resolved Questions

- **`orgs/new.vue` / `orgs/join.vue` 要不要套用新 layout？** → **Yes。** 這兩頁雖然是 `ORG_AGNOSTIC_PATHS`（沒有 `current_org` 也能到），使用者可能已經有其他 org、只是還沒切過去或正在多加一個，套 layout 讓 OrgSwitcher 常駐是合理的。D1 的頁面遷移清單擴大到 12 頁（原 10 頁 + 這兩頁）；`admin-web-nav` spec 的「哪些頁面套用 layout」規則調整為以「是否為 pre-auth／zero-Org landing 頁」界定排除清單，而不是「是否需要 active Org」，因為這兩頁技術上屬於 org-agnostic 但仍應顯示導覽。
- **`checkin/[appUserId]/trajectory.vue`（個人軌跡）要不要跟打卡看板一樣開放給 member 讀取？** → **Yes。** 跟打卡看板／事件歷史視為同一個能力一起開放，member 能看到跟 admin 完全一致的個人軌跡內容，異動類操作（目前這頁沒有）維持無。
  - **Apply 階段補充（D4 範圍修正）**：這頁實際讀取的資料來自 `GET /checkin/users/:id/locations`（軌跡點），不是 `checkin/list_user_events`——D4 原本列的三個 endpoint 漏了這個，若不補上，member 打開這頁會因為抓不到軌跡點資料而卡住。已在 apply 過程中一併鬆綁這個 endpoint（`RequireAdmin` → `RequireActiveOrg`），維持「member 看到內容跟 admin 完全一致」的決定。`GET /checkin/users/:id/locations/export`（xlsx 匯出）**不**跟著開放——批次匯出原始定位資料視為比看地圖更敏感，維持 admin-only；trajectory 頁的「匯出 xlsx」按鈕相應地用 `auth.isAdmin.value` 隱藏。
