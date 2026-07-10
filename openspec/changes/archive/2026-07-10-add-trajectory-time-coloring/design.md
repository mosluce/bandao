## Context

兩個軌跡畫面都畫單色路徑：
- **app** `trajectory_screen.dart`：`flutter_map` 一條 `Polyline(color: primary)`，點來自 `GET /app/checkin/me/locations`（`trajectory_controller` 只抓 pings）；起點/終點 dot 取 `points.first/last`。
- **admin-web** `checkin/[appUserId]/trajectory.vue`：Leaflet 一條 `L.polyline({color:'#1f2937'})`；另有依 event_type 上色的 `circleMarker`（上/下班/轉場），事件資料已載入。

需求：路徑依**時段**上色（早暖→晚冷、漸層過渡），且起點以**打卡**為準即時顯示。

## Goals / Non-Goals

**Goals:**
- 路徑線依每點 time-of-day 上色，兩端視覺一致、共用一份色標定義。
- 色標錨定絕對時鐘時間（06–22 clamp），淺底圖上可讀、非彩虹、通過 dataviz 檢核。
- 起點錨定當日 clock-in 事件，有打卡即畫。
- 兩端加「色→時間」legend。

**Non-Goals:**
- datepicker highlight（另開 change）。
- 跨午夜/夜班特殊色域（clamp）。
- 終點錨定 clock-out（維持最後 ping）。
- 不改 api（復用既有端點）。

## Decisions

### D1. 色標＝絕對 time-of-day，非路程 recency
每點顏色 = `scale(clamp(timeOfDay, 06:00, 22:00))`，`timeOfDay` 為當日經過的分鐘數（Org 時區的本地時鐘），與路徑起訖無關。
- **為何**：使用者要「早上偏暖、晚上偏冷」是絕對語意；只上半天班者整條線落在該時段色域，不會像 recency 漸層那樣被拉滿全譜而誤導。

### D2. 色標＝雙極暖→冷連續 ramp，走紅-紫側
5 錨點（經 `dataviz` validator，light surface 全通過）：

| 時鐘 | hex | |
|------|-----|---|
| 06:00 | `#ea580c` | 橙（最暖）|
| 10:00 | `#e11d48` | 玫瑰 |
| 14:00 | `#c026d3` | 洋紅（暖冷橋接）|
| 18:00 | `#7c3aed` | 紫 |
| 22:00 | `#4338ca` | 靛（最冷）|

錨點間線性插值（RGB 或 HSL 皆可，兩端需一致）；`< 06:00` 取 06:00 色、`> 22:00` 取 22:00 色。
- **為何走紅-紫側不經綠**：dataviz 禁彩虹（感知不均、色盲不友善）。暖→冷若經黃綠青會變彩虹；改走橙→玫瑰→洋紅→紫→靛，是乾淨的雙極過渡。
- **為何不用 diverging 的灰中點**：地圖線畫在 CARTO Positron **淺灰底**上，低彩度灰線會消失（validator 對灰中點 chroma FAIL 佐證）。故全程維持彩度。
- **為何只驗 light**：兩端地圖固定用 Positron 淺底，路徑線恆在淺色情境；不需 dark ramp。legend 疊在地圖上同屬淺色。
- **CVD WARN 的處理**：連續 ramp 相鄰錨點本就相近（validator WARN 在 8–12 帶）；secondary encoding＝legend 對照條 + 這是連續刻度非離散類別。

### D3. 渲染方式
- **app**：`Polyline(points: pts, gradientColors: perPointColors)`，`perPointColors[i] = scale(pts[i].time)`，flutter_map 於相鄰點間插值。（需確認安裝的 flutter_map 版本 `gradientColors` API；若該版本語意是「沿長度均分」而非逐點，退回「分段多條 Polyline」策略。）
- **admin-web**：Leaflet 無原生漸層 → 相鄰兩點畫一段 `L.polyline`，色取該段中點時間；單日數百 ping ⇒ 數百段，效能可接受（必要時用 `preferCanvas`）。

### D4. 起點錨定 clock-in 事件
起點座標＝當日第一個 `clock_in` 事件的 `EventLocation.coordinates`；有此事件即畫起點 dot（即使 0 pings），dot 色＝該打卡時間的時段色。終點維持最後一個 ping。
- **app 資料**：`trajectory_controller` 除 pings 外，另以 `GET /app/checkin/events`（既有）抓當日事件，取 clock-in 座標；狀態容器新增 `events`（或僅 `startAnchor`）。
- **admin-web**：事件已載入，直接取 clock-in。
- **空狀態調整**：現況「0 pings → `該日無軌跡資料`、不建圖」需放寬為「0 pings 但有 clock-in → 仍建圖、只畫起點 dot（無線）」。無 pings 且無 clock-in 才顯示無資料文字。

### D5. Legend
兩端加橫向漸層條，左右端標 `6:00 / 22:00`，中間標 `12:00 / 18:00`（或 6a/12p/6p/10p）。疊在地圖角落（淺底情境，沿用 light 色標）。
- **為何**：時間上色若無對照，viewer 無法解讀色→時間。

## Risks / Trade-offs

- **flutter_map `gradientColors` 語意** → 若非逐點插值而是沿長度均分，色↔時間會與點密度耦合失真；退路是分段多 Polyline（與 admin 同策略）。apply 時先驗證。
- **admin-web 數百段 polyline** → DOM/效能；用 canvas renderer 緩解，單日資料量可接受。
- **色標雙份實作漂移**（Flutter vs CSS）→ spec 定唯一真值 + 兩端各寫測試對關鍵錨點取色。
- **偏離 dataviz「sequential 單一色相」預設** → 暖→冷是使用者明確的語意需求；以「雙極、非彩虹、validator 通過」收斂，於 design 記錄此刻意偏離。
- **起點事件座標缺 GPS**（打卡當下無定位權限）→ 該 clock-in 可能無座標；此時退回「無起點錨點」，若也無 pings 則顯示無資料。

## Migration Plan

1. 純前端 + 既有端點；無資料模型 / api 遷移。
2. 舊行為（單色線、起點=first ping）被新行為取代；無持久化狀態需轉換。
3. Rollback：前端改動可整批 revert；色標為新增 util。

## Open Questions

- 錨點間插值用 RGB 還是 HSL？（HSL 過渡較順但需避免繞回綠；傾向在紅-紫段用 RGB 線性即可，兩端一致為要。apply 時定。）
- legend 擺放位置（地圖左下/右上）與是否可收合 —— 交 UI 微調，非契約。
