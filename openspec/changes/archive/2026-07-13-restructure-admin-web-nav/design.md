## Context

`layouts/default.vue` 目前的 `navItems` 是一個扁平陣列：

```ts
interface NavItem {
  to: string
  label: string
  badge?: number
}
```

`OrgSwitcher.vue` 的下拉選單原本是為舊版寬版水平 header 設計的（外層 `relative inline-block`，面板 `absolute right-0 w-72`）。`add-admin-web-sidemenu` 把它搬進 256px 寬、`fixed ... left-0` 貼齊視窗左緣的 sidebar 之後沒有重新檢查這個定位邏輯——面板往左展開的固定 288px 寬度在窄 sidebar 裡會直接衝出視窗左緣，造成組織名稱那一截視覺上消失、只剩右邊的角色徽章還在畫面內。

## Goals / Non-Goals

**Goals:**
- 選單改成有主從關係的巢狀結構，子項一律常駐展開（不做手風琴）。
- member 視角的「扁平化退化」是資料結構的自然結果，不另外寫特殊分支邏輯。
- 修好 OrgSwitcher 下拉選單在窄 sidebar 裡的溢出問題，讓它在任何寬度下都貼合容器。

**Non-Goals:**
- 不調整任何角色的實際存取權限——`add-admin-web-sidemenu` 定案的邊界（哪些頁面/endpoint 開放給 member）維持不變，這次純粹是排版與分組。
- 不做手風琴收合/展開互動——「先常駐展開即可」是使用者明確決定的範圍。
- 不重新設計 OrgSwitcher 下拉選單本身的按鈕外觀/樣式，只修寬度溢出這個 bug。

## Decisions

### D1. NavItem 資料結構加上 `children`，`to` 改為可選

```ts
interface NavItem {
  to?: string          // 不存在 = 純標籤（不可點擊），目前只有「進階工具」用到
  label: string
  badge?: number
  children?: NavItem[]
}
```

`navItems` computed 改寫成：

```ts
const navItems = computed<NavItem[]>(() => {
  const items: NavItem[] = [
    { to: '/checkin', label: '打卡看板' },
    {
      to: '/members',
      label: '成員管理',
      children: auth.isAdmin.value
        ? [{
            to: '/admin/join-requests',
            label: '加入申請',
            badge: pendingJoinCount.value > 0 ? pendingJoinCount.value : undefined,
          }]
        : [],
    },
    {
      to: '/app-users',
      label: 'App 使用者',
      children: auth.isAdmin.value
        ? [{ to: '/settings/auth', label: '驗證來源' }]
        : [],
    },
  ]
  if (auth.isAdmin.value) {
    items.push({
      label: '進階工具',
      children: [
        { to: '/settings/api-tokens', label: 'API Token' },
        { to: '/cooldowns', label: '冷卻管理' },
      ],
    })
  }
  items.push({ to: '/download', label: '下載 App' })
  return items
})
```

- **為何**：member 的「扁平化退化」不需要另外寫判斷邏輯——`成員管理`/`App 使用者`的 `children` 在 member 視角下天然是空陣列（樣板端只要判斷 `children?.length` 決定要不要渲染子清單即可），`進階工具`整個物件在 member 視角下根本不會被 push 進陣列。跟現在既有的「用 `auth.isAdmin.value` 決定 push 哪些項目」寫法一致，沒有引入新模式。

### D2. 樣板渲染：`to` 存在渲染成 `NuxtLink`，不存在渲染成純文字標籤；子項固定緊接在父項下方，不做展開/收合

```html
<template v-for="item in navItems" :key="item.label">
  <NuxtLink v-if="item.to" :to="item.to" ...>{{ item.label }}...</NuxtLink>
  <p v-else class="...text-xs uppercase tracking-wide text-slate-400...">{{ item.label }}</p>

  <NuxtLink
    v-for="child in item.children"
    :key="child.to"
    :to="child.to!"
    class="...ml-4...text-sm..."
  >
    {{ child.label }}
    <span v-if="child.badge">...</span>
  </NuxtLink>
</template>
```

- 子項用左邊縮排（`ml-4` 或等效）+ 較小/較淡的文字樣式跟父項做視覺區分，不畫連接線/框線，維持專案既有的「純 Tailwind、不過度裝飾」風格。
- `進階工具` 標籤本身用 `<p>`（非互動元素）渲染，樣式比照 `OrgSwitcher.vue` 下拉選單裡「我擁有的 / 我加入的」那種 section 標籤的既有寫法（`text-xs font-medium uppercase tracking-wide text-slate-400`），沿用專案裡已經有的視覺語彙，不是新發明一套。

### D3. Badge 留在子項本身，不冒泡到父項

`加入申請` 的待審核紅點徽章維持掛在它自己身上，不往上冒泡到 `成員管理`。
- **為何**：因為子項一律常駐展開（D2 決定），徽章本來就一直看得到，沒有「收合起來看不到提醒」的問題，不需要額外的冒泡邏輯。

### D4. OrgSwitcher.vue：wrapper 從 `inline-block` 改 `block w-full`，面板從固定 `w-72` 改 `left-0 right-0`

```html
<!-- 之前 -->
<div class="relative inline-block text-left">
  ...
  <div class="absolute right-0 z-10 mt-2 w-72 origin-top-right ...">

<!-- 之後 -->
<div class="relative block w-full text-left">
  ...
  <div class="absolute left-0 right-0 z-10 mt-2 origin-top ...">
```

- **為何選這個方向而不是「面板收窄但維持固定寬度」**：固定寬度（不管改成多少 px）永遠要手動猜一個「在最窄的 sidebar 情境下也不會溢出」的數字，之後 sidebar 寬度或字體大小一調整就要重新調一次。改成 `left-0 right-0` 貼合 wrapper 本身寬度，wrapper 又是 `w-full` 撐滿 sidebar header 的可用寬度，兩者掛鉤之後，不管 sidebar 之後怎麼改版，下拉選單永遠貼合容器邊界，不會再需要手動維護一個寫死的寬度值。
- **權衡**：sidebar header 的可用寬度（扣掉 padding 後約 224px）比原本的 288px 窄，長組織名稱在下拉選單列表裡會更早被 `truncate` 省略號截斷。這是可以接受的犧牲——現在的行為（一部分內容直接消失在視窗外、完全看不到）明顯更糟。
- OrgSwitcher.vue 目前只有 `layouts/default.vue` 這一個使用點（其餘檔案裡的「OrgSwitcher」字樣都只是註解），改成 `w-full` 沒有其他呼叫點需要相容考量。
- 切換按鈕本身（`班到 admin` 旁邊那顆組織名稱按鈕）的外觀不在這次調整範圍內——只動 wrapper 的寬度基準跟下拉面板的定位方式，按鈕維持 `inline-flex` 內容自適應寬度不變。

## Risks / Trade-offs

- **[Risk] 長組織名稱在下拉選單裡被截斷得比之前更早** → 已知取捨（見 D4），`truncate` 本來就會處理省略號，不會整段文字消失或破版，只是可視字元變少。
- **[Risk] `進階工具` 這種「純標籤、不可點擊」的節點是選單裡第一個這樣的模式** → 範圍很小（只有一個節點），用既有的「OrgSwitcher 下拉選單裡的 section 標籤」視覺語彙即可，不需要額外設計新元件。
