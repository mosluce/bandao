## Why

Admin 看打卡列表時 `region_name` 只顯示到區（`信義區` / `Cupertino`），對排查「這個工人今天在哪幾條街跑」用處不大。改細到街道層級（`信義區 · 忠孝東路五段` / `Cupertino · Stevens Creek Boulevard`）讓 admin 一眼看出活動範圍，但有意停在「路名」這條隱私安全線 — 不收集巷弄與門牌，避免單一事件就能還原精確位置。

順手把 ROADMAP 上停留已久的 reverse-geocoding cache 一起做，因為粒度變細後同地點重複打 Nominatim 機會更高，cache 的成本效益正好凸顯。

## What Changes

- `NominatimGeocoder` 提高 `zoom` 從 14 改為 17，並從 response 抓取 `road` 欄位
- `best_label` 重寫成 compose 邏輯：`"{district} · {road}"`，缺其中一個就降為單側 fallback，兩個都缺 fallback 到 `display_name` / `null`（行為跟現在一致）
- 新增 `CachedReverseGeocoder` decorator，包裝任意 `ReverseGeocoder`：LRU 容量 10000、TTL 1 小時、key 取 lat/lng 4 位小數（~11 m 格網）
- `AppState::new()` 改用 `CachedReverseGeocoder::new(NominatimGeocoder::new())`；測試用 `StaticReverseGeocoder` 不包 cache，現有測試不受影響
- `admin-web/pages/privacy.vue` 範例字串從「信義區」更新為含街道的新格式，並補一句「僅收集到街道層級，不含巷弄與門牌」
- 舊事件不回填 — 新事件用新格式、舊事件保留現有 `region_name`，admin-web 顯示邏輯不變

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `checkin-events`：reverse-geocoder 的 requirement 內容更動 — 改 zoom 與 compose 規則、加上 cache 行為。`region_name` 欄位本身不變（仍是 `String?`）。

## Impact

- **api/**：`services/reverse_geocoder.rs`（重寫 best_label、加 road 欄位、提 zoom）；新增 `services/reverse_geocoder/cache.rs`（或同 module 內的 `CachedReverseGeocoder`）；`state.rs`（生產組裝改成包 cache）。新增 unit tests for compose + cache。
- **admin-web/**：`pages/privacy.vue`（範例字串 + 一句新說明）。
- **app/**：不動。client 不顯示 `region_name`。
- **MongoDB schema**：不變。`region_name` 仍是單一 `String?`，只是內容變細。
- **舊資料**：不回填。混合精度可接受（archive policy 與 ROADMAP 已達共識）。
- **Nominatim usage policy**：zoom=17 仍是同一 endpoint、rate 一樣；cache 反而降低外部呼叫量。
- **隱私政策同步**：`admin-web/pages/privacy.vue` 必須跟 API 改動一起 ship，否則合規上不一致。
