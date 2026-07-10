## Why

軌跡路徑（app「我的工作日記」與 admin-web 員工軌跡頁）目前都是**單一顏色**的一條線，看不出「哪一段是早上走的、哪一段是傍晚走的」。把路徑依**時段**上色（早上偏暖 → 晚上偏冷），能一眼讀出一天的時間流向。

另外，現在起點只在有 location ping 時才畫；但只要員工**打了上班卡**就該看到起點，不必等定位軌跡累積。

## What Changes

- **時段漸層上色（app + admin-web）**：路徑線改為依每個點的 `occurred_at_client` **時段（time-of-day）**取色，沿線漸層過渡。
  - 色標錨定**絕對時鐘時間**（非路程起訖）：`06:00` 最暖 → `22:00` 最冷，域外 clamp。只上半天班的人整條線落在對應時段色域，不會被誤導成走了一整天。
  - 色標為**雙極暖→冷連續 ramp**（走紅-紫側、非彩虹、全程有彩度以在淺底圖上可見），5 個錨點經 dataviz validator 通過（light surface）：`06:00 #ea580c → 10:00 #e11d48 → 14:00 #c026d3 → 18:00 #7c3aed → 22:00 #4338ca`。
  - 兩端共用**同一份色標定義**（見 spec）；app 用 `flutter_map` Polyline 的逐點 `gradientColors`，admin-web 用分段 polyline。
  - 兩端新增**「色→時間」legend**（橫向漸層條，標 6a/12p/6p/10p），疊在地圖淺底情境上。
- **起點錨定打卡（app + admin-web）**：起點座標改取**當日 clock-in 事件**的位置，有打卡即畫起點 dot（不等 pings）。起點 dot 上「打卡時間」對應的色；終點**維持最後一個 ping**。
  - app：trajectory controller 除了 pings，另抓當日事件（既有端點 `GET /app/checkin/events`）取 clock-in 座標。
  - admin-web：已載入事件（畫 event marker），直接沿用。

## Capabilities

### Modified Capabilities
- `app-personal-trajectory`: `/trajectory` 的 polyline 改時段漸層上色 + legend；起點錨定當日 clock-in 事件、有打卡即畫（不等 pings）。
- `admin-trajectory-dashboard`: 員工軌跡頁 polyline 改時段漸層上色 + legend；起點錨定 clock-in 事件。

## Impact

- **app（Flutter）**：`trajectory_screen.dart` 的 Polyline 改逐點 `gradientColors`（依時段取色）；起點 marker 改取 clock-in 事件座標；`trajectory_controller.dart` / repository 多抓當日事件（`GET /app/checkin/events`）；新增時段色標 util + legend widget。
- **admin-web（Nuxt/Leaflet）**：`checkin/[appUserId]/trajectory.vue` 的單色 `L.polyline` 改為依時段分段上色；起點取 clock-in 事件座標；新增色標 util + legend。
- **共用契約**：時段色標（domain 06–22、5 錨點、clamp、插值規則）在 spec 定一份，app（Flutter `Color`）與 admin-web（CSS/hex）各自複製實作，需一致。
- **api**：預期無變更（app 復用既有 `GET /app/checkin/events`；admin 復用既有事件查詢）。若當日事件查詢有缺口於 design 再評估。
- **Non-Goals**：
  - **datepicker highlight（標示有紀錄的日期）不在本 change** —— 另開獨立 change（需 active-days 端點、tz 日界、可能引日曆套件）。
  - 不處理跨午夜 / 夜班超出 06–22 的特殊色域（一律 clamp）。
  - 終點不錨定 clock-out（維持最後一個 ping）。
