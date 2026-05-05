## Context

`api/src/services/reverse_geocoder.rs` 目前是 `NominatimGeocoder` 直接掛在 `AppState.geocoder` 上，每筆 `POST /app/checkin/events` 同步打一次 Nominatim：

```
App → app_checkin handler → state.geocoder.lookup(lat, lng) → Nominatim
                                       │
                                       └─ best_label() picks finest of:
                                          suburb > city > town > village >
                                          county > state > country
```

`zoom=14` 的 Nominatim response 大約只到 suburb 級別。`address.road` 欄位幾乎拿不到。`best_label()` 也沒考慮 road 欄位。結果就是 admin-web 看到的 region_name 永遠是區。

ROADMAP 一直放著兩條相關項目：
- 「reverse geocoding cache」：LRU、5 位小數 key、TTL 1 小時 — 還沒做
- （街道層級沒明確列入 — 直接從使用者請求進來）

兩者 coupling 很自然：粒度變細不會改變 Nominatim 呼叫頻率（單筆 event = 單筆 lookup），但**同一棟建築的多筆事件**重複打 Nominatim 拿同樣 road 資訊就純粹浪費。Cache 在街道層級啟動的 cost-benefit 比區級別明顯。

## Goals / Non-Goals

**Goals:**
- Admin 在打卡列表能看到「區 · 街道」格式的 region_name（新事件起）
- Nominatim 失敗仍 fail-soft：`region_name = null`，事件照常存
- 透過 cache 降低重複地點的 Nominatim 呼叫量
- 沒有 schema migration 與資料回填
- 隱私政策（`admin-web/pages/privacy.vue`）與 API 行為一起更新

**Non-Goals:**
- 不收集巷弄、門牌號碼（隱私安全線）
- 不替舊事件回填 region_name（舊資料維持區級別）
- 不改 `EventLocation` 的 schema（不加 `street`、`district`、`city` 等結構化欄位）
- 不替 `pending_location_pings` 加 reverse geocoding（pings 量大、保持原狀）
- 不做地圖式 / 結構化展開 UI — admin-web 只是把更細的 string 顯示出來
- 不引入新 geocoding provider（仍只有 Nominatim）

## Decisions

### D1：Compose 格式 `{district} · {road}`，中間以 `·` 分隔

**Why**：使用者選定「組合式」而非取代式。保留區的 context 才能在不同城市同名街道的情境下保持唯一性（例如台北信義區忠孝東路 vs 高雄苓雅區忠孝東路）。`·` 分隔字元中性、跨 locale 可讀（`Cupertino · Stevens Creek Blvd`）。

**Alternatives considered**:
- `{road}` 取代式：失去區 context、同名街道易混淆
- 多個欄位結構化：要動 schema、admin-web 改 rendering，工程大
- 直接用 Nominatim `display_name`：太長且跨國家格式差很大

**Fallback chain**：
- 兩者都有 → `"信義區 · 忠孝東路五段"`
- 只有 road → `"忠孝東路五段"`
- 只有 district → `"信義區"`（與現狀同）
- 都沒有 → fallback 到 `display_name` → 仍 None → null（與現狀同）

### D2：`zoom=17`

**Why**：Nominatim 的 zoom 對應 OSM zoom levels。zoom 14 ≈ suburb 級、zoom 16 ≈ small road、zoom 17 ≈ road 已經穩定回得出。zoom 18 開始可能拿到門牌（House number），跨過隱私安全線，所以停在 17。

**Alternatives**: zoom 16（部分小路拿不到 road）、zoom 18（拿太細）。

### D3：Cache 採 decorator pattern

```rust
let geocoder: SharedReverseGeocoder = Arc::new(
    CachedReverseGeocoder::new(NominatimGeocoder::new())
);
```

**Why**：
- 不改 `ReverseGeocoder` trait
- 測試可以用 `StaticReverseGeocoder` 直接，不被迫經過 cache
- 未來換 provider（OpenCage、Google）只要實作 trait，cache 邏輯不動
- 「cache 是 decorator」這個語意比「cache 內建在 NominatimGeocoder」清楚

**Alternatives**: 在 NominatimGeocoder 內建 cache（耦合）、`CachedReverseGeocoderProvider` factory（過度設計）。

### D4：Cache key 取 lat/lng 4 位小數

**Why**：4 位小數 ≈ 11 公尺格網。同一棟建築、同個停車場、同個工地大門 hit 同一格 → cache hit。

ROADMAP 原本寫 5 位小數（≈ 1 m），對街道層級反而精細到 cache hit 機會偏低。4 位小數對「街道」這個粒度剛好（街道寬度通常 5-30 m）。

```rust
fn key(lat: f64, lng: f64) -> (i64, i64) {
    ((lat * 10_000.0).round() as i64, (lng * 10_000.0).round() as i64)
}
```

**Alternatives**: 5 位小數（cache hit 太低）、3 位小數 ~111 m（過度合併、不同街道擠進同 key）、geohash（複雜度過頭）。

### D5：Cache 容量 10000、TTL 1 小時

**Why**：
- 10000 entries × 估每筆 ~200 bytes（key 16B + label 字串 + metadata） ≈ 2 MB，記憶體成本可忽略
- 1 小時 TTL 對 Nominatim 資料穩定度足夠（街道資料變動極少），又能在 hot reload / 部署後自然失效
- 整 process 共享一份 cache（`Arc::new`）— 多個 axum worker 共用

**Alternatives**: 容量無上限（OOM 風險）、TTL 24 小時（資料更新風險）、無 TTL（同 OOM）。

### D6：實作用 `lru` crate

`lru` crate 是 Rust 社群標準，提供 `LruCache<K, V>` 帶 capacity 上限。包一層 `RwLock` 給 cross-thread 安全。TTL 自己疊一層 — 在 value 裡記 `expires_at: Instant`，讀取時檢查、過期視為 miss。

**Alternatives**: `moka`（更強，但對這量級殺雞用牛刀）、`cached` crate（macro-based，不適合 trait object）、自己寫 LRU（重新發明輪子）。

### D7：admin-web 顯示邏輯不變

`pages/checkin/[appUserId].vue` 與 `pages/checkin/index.vue` 已經是 `manual_label || region_name || coords` 鏈。region_name 從「信義區」變成「信義區 · 忠孝東路五段」就只是字數變多 — 表格欄位寬度可能要看一下，但不需要 rendering 層改邏輯。

### D8：隱私政策更新落在 `pages/privacy.vue` 而非 spec

`org-privacy-policy` 的 spec 沒有規定具體範例字串（policy 本身是 template），所以 spec 不動；只更新 admin-web 的 page rendering（範例字串 + 一句「僅到街道層級」說明）。

## Risks / Trade-offs

- **Nominatim zoom=17 對偏遠地區可能仍沒 road** → fallback chain（D1）會降到 suburb/city，等於現狀，行為連續。
- **Locale 差異**：`address` 欄位在不同國家的取名不同（日本沒有 road，會用 `quarter`、`block`、`number`） → fallback chain 涵蓋常見替代，cover 主要使用情境（台灣 + 海外少量）。極端情況退到 `display_name`。
- **`·` 分隔符在某些字型 / 介面渲染可能不一致** → 試過中文 + 拉丁字混排視覺 OK；如果出問題改成 ` - ` 是一行 fix。
- **Cache 在 process 重啟後清空**（沒持久化） → 對冷啟動的影響：第一波打卡會重打 Nominatim，預計 5-10 分鐘 warm-up 後 hit rate 上升。可接受，反正不是 critical path（fail-soft）。
- **Cache 跨 worker 不一致**（每 axum worker 一個 process？） → 實際上 axum 單 process 多 thread；`Arc<RwLock<LruCache>>` 在同一 process 內 share。多 process 部署（如 fly.io 多機）才會 per-instance — 影響可忽略。
- **隱私 narrative 改變**：之前承諾「僅到區」，現在到「街道」 — 屬於範圍擴大。privacy.vue 必須說清楚並一起 ship。Spec 上 `org-privacy-policy` 沒寫死範例所以 spec 不變、只是 page 改字。
- **`add-event-street-region` archive 後**，舊事件 region_name 跟新事件混在同一 list — admin 看到「8/15 到 8/20 的 events 都是區，8/21 之後是區+街」可能困惑。可以在 admin-web FAQ 或 UI 加一行說明，但**這次先不做**，等使用者反映再處理。

## Migration Plan

純前進式變更：
1. API 改完部署 → 新事件開始用新 compose
2. admin-web privacy.vue 同步部署 → 隱私政策對外一致
3. 舊事件不動

**Rollback**：把 best_label 改回原版 + zoom 改回 14 + 拔掉 cache decorator 即可。已寫入的「街道」格式 region_name 留在 DB 裡無害（仍是 `String?`），未來再前進。
