# argus-api

Rust + axum + MongoDB。multi-tenant 簽到系統的後端，dashboard 與 app 共用。

## 開發前置

- Rust toolchain（鎖定在 `rust-toolchain.toml`，目前 1.93.1）
- Docker（跑本地 MongoDB 與整合測試的 testcontainers）
- 從 repo root 起 MongoDB：`docker compose up -d mongodb`

## 跑起來

```bash
cd api
cargo run
```

預設監聽 `127.0.0.1:8080`，連 `mongodb://argus:argus@localhost:27017/argus`。本地若 8080 被佔（macOS 常見），改 port：

```bash
ARGUS_LISTEN_ADDR=127.0.0.1:9090 \
ARGUS_ALLOWED_ORIGIN=http://localhost:3000 \
cargo run
```

## 測試

整合測試會用 testcontainers 自己起 MongoDB container，不需要 `docker compose up` 預先啟好（但 Docker daemon 必須在跑）。

```bash
cargo test            # unit + integration
cargo test -- --nocapture   # 看 tracing 輸出
```

第一次跑會比較久（拉 mongo image + 編 deps）。後續快取生效後本機約 1 分鐘內可跑完全部測試。

**macOS 注意**：`cargo test` 一次跑全部 integration binary 時，loopback ephemeral port 可能因 TIME_WAIT 累積而觸發 `AddrNotAvailable`，個別 test 會偽失敗。確認 logic 是否真的壞掉，建議用 [`cargo-nextest`](https://nexte.st/) 或這個 shell loop 跨 binary 序列化：

```bash
for t in $(ls tests/*.rs | grep -v common | sed 's|tests/||;s|\.rs||'); do
  cargo test --test "$t" -- --test-threads=1
  sleep 4
done
```

## 環境變數

| 變數 | 預設 | 說明 |
| --- | --- | --- |
| `ARGUS_MONGO_URI` | `mongodb://argus:argus@localhost:27017/argus?authSource=admin` | MongoDB 連線字串 |
| `ARGUS_MONGO_DB` | `argus` | 資料庫名稱 |
| `ARGUS_LISTEN_ADDR` | `127.0.0.1:8080` | API listen address |
| `ARGUS_SESSION_TTL_SECONDS` | `1209600`（14 天）| Dashboard session 存活時間 |
| `ARGUS_COOKIE_DOMAIN` | _(不設)_ | Cookie domain；跨子網域時設定 |
| `ARGUS_COOKIE_SECURE` | `false` | Production 必須設 `true`（要求 HTTPS）|
| `ARGUS_ALLOWED_ORIGIN` | _(不設)_ | CORS `Access-Control-Allow-Origin`；用 cookie auth 時必填具體 origin（不能用 `*`） |
| `ARGUS_LOG` | `info,argus_api=debug` | `tracing_subscriber` EnvFilter 格式 |

`.env` 也會被 `dotenvy` 載入。

## 架構摘要

- **入口**：`src/main.rs`（config / tracing / Mongo 連線 / axum router / graceful shutdown）
- **HTTP**：`src/handlers/`（auth / me / orgs / users）
- **領域層**：`src/db/`（Repository pattern 包 mongodb collection）、`src/domain/`（純資料結構）
- **錯誤**：`src/error.rs` 的 `ApiError` enum 走 `IntoResponse`，回傳 `{ error: { code, message } }`

更詳細的設計請看 `openspec/changes/archive/2026-05-01-add-tenant-and-auth/design.md`、`2026-05-01-add-org-vanity-slug/design.md`、`2026-05-01-add-member-removal-and-owner/design.md`，以及多對多 + owner transfer 的 `add-multi-org-membership/design.md`。

## Org 自訂代碼（vanity slug）

每個 Org 有一個 random `code`（10 字元、`[2-9A-HJ-NP-Z]`）負責安全屏障，再外加可選的 `slug`（小寫、`^[a-z0-9]{2,24}$`）負責人類介面。Join input 會依字符集分流：slug-shaped 走 `slug_reservations` 表，code-shaped 走 `orgs.code`。

兩個 admin-only endpoint：

- `POST /orgs/me/slug` body `{ "slug": "acme" }` → 200 `{ "slug": "acme" }`
- `DELETE /orgs/me/slug` → 204

回應錯誤碼：

| Code | HTTP | 說明 |
| --- | --- | --- |
| `INVALID_SLUG_FORMAT` | 400 | 不符 `^[a-z0-9]{2,24}$` |
| `SLUG_RESERVED` | 400 | 命中保留字（API 路徑根、系統識別字、`argus`，列表在 `auth::slug::RESERVED_SLUGS`） |
| `SLUG_TAKEN` | 409 | 已被其他 Org active 持有，或仍在 30 天 grace 期間 |
| `SLUG_CHANGE_TOO_SOON` | 429 | 距離上次變更未滿 30 天；body 含 `retry_after`（ISO-8601）|

Slug 換掉時舊 slug 進 30 天 grace（`slug_reservations.expires_at` + Mongo TTL 自動清），這 30 天內舊 slug 仍能 join 原 Org，且其他 Org 不能搶走。第一次 SET 不受 30 天限制；之後每 30 天一次（DELETE 也計入）。

## Identity vs membership（多對多）

`dashboard_users` 是純 identity（email + password_hash），可同時持有 0..N 個 `dashboard_memberships(user_id, org_id, role)`。Role 住在 membership 上、不在 user 上。Session 帶可變的 `current_org_id: Option<ObjectId>`，每 request middleware 重查 `(user_id, current_org_id)` 拿 role。`current_org_id` 為 `null` 是合法的「zero-Org 狀態」，org-scoped endpoint 會回 `NO_ACTIVE_ORG`。

Logged-in 使用者操作多 Org 的 endpoints：

- `POST /me/orgs` body `{ "org_name": "..." }` 建立新 Org，呼叫者成為 owner，session current_org 換到新 Org
- `POST /me/memberships` body `{ "org_code": "..." }` 加入既有 Org（接受 org_code / active slug / grace slug），cooldown 規則與 register mode=join 相同
- `POST /me/current-org` body `{ "org_id": "..." }` 切換目前 session 的 active Org（必須是自己的 membership）
- `GET /me` 回 `{ user, memberships: [{ org, role }, ...], current_org, role }`

`POST /auth/register` 嚴格只給新 identity 用：email 已存在直接 `EMAIL_TAKEN`，請用 login + `/me/orgs` 或 `/me/memberships` 替代。

## 成員退出 / 移除 / 擁有權轉移（owner / cooldown）

每個 Org 有一個 owner（`Org.owner_id`），由建立 Org 的人擔任，可透過 `POST /orgs/me/owner` 轉移給另一位 admin。Owner 永遠是 admin、不能被降級、不能被踢、不能自離；要離開必先轉移擁有權。

Endpoints：

- `DELETE /dashboard-users/:id` admin 移除目標在 `current_org` 的 membership（不可移除自己；自離請走 `/me/leave`；目標的其他 Org membership 與 user identity 不受影響）
- `POST /me/leave` 認證使用者離開 `current_org`（owner 不可呼叫；server 會 force-kick 該 (user, org) 的所有 session，但其他 org 的 session 留著）
- `GET /dashboard-users/cooldowns` admin 列出當前 Org 的冷卻中 email
- `DELETE /dashboard-users/cooldowns/:email` admin 提前釋放冷卻
- `POST /orgs/me/owner` body `{ "new_owner_user_id", "current_password" }` owner 轉移擁有權給同 Org 的另一位 admin（密碼重認證；轉移後原 owner 變成普通 admin）

被移除 / 自離後寫入 `removed_memberships` marker（`org_id` + lowercase email），cooldown 7 天。任何 membership 建立路徑（register mode=join + `POST /me/memberships`）都會檢查 marker；命中 → `EMAIL_IN_COOLDOWN`。Marker 由 Mongo TTL（`cooldown_until`）自動 GC。

| Code | HTTP | 說明 |
| --- | --- | --- |
| `OWNER_PROTECTED` | 403 | 操作目標是 Org owner（不可移除 / 不可自離 / 不可降級） |
| `EMAIL_IN_COOLDOWN` | 409 | 此 email 對該 Org 在冷卻期內，不能 rejoin |
| `NO_ACTIVE_ORG` | 403 | 目前 session 沒有 `current_org_id`，需先建立 / 加入 / 切換到一個 Org |
| `NOT_A_MEMBER` | 404 | `/me/current-org` 切換到自己沒有 membership 的 Org |
| `ALREADY_MEMBER` | 409 | `/me/memberships` 加入自己已是成員的 Org |
| `INVALID_PASSWORD` | 401 | owner transfer 的 `current_password` 驗證失敗 |
| `INVALID_TARGET` | 400 | owner transfer 的 `new_owner_user_id` 不是同 Org 的 admin |
| `SAME_OWNER` | 400 | owner transfer 的目標就是呼叫者 |

## AppUser（手機 app 端使用者）

跟 dashboard 完全分開的軸：`app_users` 是 1:1 with Org（同一個人不會跨 Org 當 AppUser），由 admin 建立，無自助註冊。Auth 走 `Authorization: Bearer <token>` header（而不是 cookie），token 存在 `app_sessions` collection（與 dashboard session 同樣是 server-side opaque random + sliding refresh）。下一個 ROADMAP item `add-app-shell` 會 bootstrap Flutter app 消化這個 surface。

### Mobile-facing endpoints `/app/*`

| Endpoint | 說明 |
| --- | --- |
| `POST /app/auth/login` | body `{ org_code, username, password }`；`org_code` 接受 random code / active slug / grace-period slug（同 register mode=join 的 resolver） |
| `POST /app/auth/logout` | 刪除目前 token；其他 device 的 session 不受影響 |
| `GET /app/me` | 回 `{ user, org, needs_password_change }` |
| `POST /app/me/password` | body `{ current_password, new_password }`；改完密碼後 `needs_password_change` 清除，token 仍有效 |

login 失敗（org_code 不存在、username 不存在、wrong password、status=disabled）一律回 `INVALID_CREDENTIALS`，不洩漏失敗原因。

`needs_password_change=true` 時除了 `GET /app/me`、`POST /app/me/password`、`POST /app/auth/logout` 之外的 `/app/*` endpoint 都回 `423 LOCKED` + `NEEDS_PASSWORD_CHANGE`，強制 app 端先帶使用者改密碼。

### Admin-facing endpoints `/app-users/*`（dashboard cookie + admin role）

| Endpoint | 說明 |
| --- | --- |
| `GET /app-users` | 列出 `current_org` 內的 AppUser |
| `POST /app-users` | body `{ username, display_name }`；server 產 12 字一次性初始密碼（字符集同 `org_code`），response 含 `initial_password` 一次顯示 |
| `PATCH /app-users/:id` | body `{ display_name?, status? }`；`status=disabled` 會同步刪該 AppUser 全部 sessions |
| `POST /app-users/:id/password-reset` | 重新產一次性密碼、強制 `needs_password_change=true`、刪所有 sessions |

| Code | HTTP | 說明 |
| --- | --- | --- |
| `USERNAME_TAKEN` | 409 | 同 Org 內 username 已存在（case-insensitive） |
| `INVALID_USERNAME_FORMAT` | 400 | username 不符 `^[a-zA-Z0-9_.-]{2,32}$` |
| `NEEDS_PASSWORD_CHANGE` | 423 | AppUser 尚未變更初始密碼，先改才能呼叫其他 `/app/*` |

## 打卡 / Checkin

四個事件 + 三狀態的狀態機。AppUser 透過 `/app/checkin/*` 提交事件，admin 透過 `/checkin/*` 看即時看板與強制收班。Flutter app（`app/`）已經消化這個 surface — 詳見 `add-app-checkin` 的 device-local queue + workmanager 背景同步。

### 狀態機

```
status: off_duty | on_site | in_transit

  off_duty   ─clock_in─────▶ on_site         上班（在某現場開始）
  on_site    ─transfer_out─▶ in_transit      離開現場、在路上
  in_transit ─transfer_in──▶ on_site         抵達下一現場
  on_site    ─clock_out────▶ off_duty        在現場下班
  in_transit ─clock_out────▶ off_duty        忘了 transfer_in 直接下班
```

`transfer_in` 表示「到了下一個現場」，**不是**「回到原本的 primary」。多現場 shift（A → B → C）是合法且預期的工作流。

### Mobile-facing endpoints `/app/checkin/*`（Bearer auth）

| Endpoint | 說明 |
| --- | --- |
| `POST /app/checkin/events` | body `{ event_type, lat, lng, accuracy?, manual_label?, occurred_at_client }`；返 `{ event, status }` |
| `GET /app/checkin/status` | 自己當前的 status + last_event |
| `GET /app/checkin/events` | 自己的事件歷史，cursor 分頁（`?before=<RFC3339>&limit=N`） |

### Admin-facing endpoints（dashboard cookie + admin）

| Endpoint | 說明 |
| --- | --- |
| `GET /checkin/users` | 即時看板：`current_org` 內所有 AppUser 的當前狀態 + has_skew_warning |
| `GET /checkin/users/:id/events` | 單個 AppUser 的事件歷史，cursor 分頁 |
| `POST /checkin/users/:id/force-checkout` | 強制收班，body `{ reason?: String (≤240) }`；事件 source=admin_force、location 沿用最後一筆、manual_label 標註「管理員強制收班」 |
| `PATCH /orgs/me/settings` | body `{ transfer_enabled?, timezone? }`；`transfer_enabled` 受 state-lock；`timezone` 隨時可改 |

### 雙時間戳與離線同步

每筆事件都有兩個時間戳：

- `occurred_at_client`：app 端裝置記錄的時間，由 request body 帶來。**顯示與排序皆以此為準**。任意 skew 都接受（包括未來 / 過去數天）。
- `occurred_at_server`：server 收到當下。僅供 audit 與 admin-web `has_skew_warning` 判定（`|client - server| > 1h` 時為 true）。

每個 AppUser 的事件嚴格按 `occurred_at_client` 升序：新事件 `client` 時間若 ≤ 該 AppUser 上一筆，回 `409 OUT_OF_ORDER`。

**App 端 queue 契約（`add-app-checkin` 已實作）**：
- 事件先寫進 device-local 持久 queue（drift / SQLite，table `pending_events`），重啟後仍在。
- 嚴格序列化送出：每筆等到 `2xx` 才送下一筆（單一 in-flight，沒有平行）。
- 失敗則同一筆以同 `occurred_at_client` 重試（不要重新 timestamp），`5xx` / network 走 1/2/4/8/16/30s exp backoff、4xx state-machine errors 標 `failed` 不重試。
- Login-handover queue wipe：每次成功登入比對每列的 `app_user_id`，不同 user 的 row 直接刪，避免共用裝置的事件被誤投到下一個帳號。

只要 app 遵守這個契約，正常運作下不會觸發 `OUT_OF_ORDER`。

### Reverse geocoding

每筆事件成功收下後，server 同步呼叫 `ReverseGeocoder::lookup(lat, lng)` 補上 `region_name`。預設實作 `NominatimGeocoder` 串接 [Nominatim](https://nominatim.openstreetmap.org/)：

- User-Agent: `argus-api/<version>`，符合 [Nominatim Usage Policy](https://operations.osmfoundation.org/policies/nominatim/) 要求
- 2 秒 timeout
- `zoom=17, addressdetails=1` — 可拿到 `road` 欄位，組合成 `"{district} · {road}"`（例：`"信義區 · 忠孝東路五段"`、`"Cupertino · Stevens Creek Boulevard"`）。缺其中一個就降為單側；兩個都缺 fallback 到 `display_name` 或 `null`。隱私邊界停在路名，不收巷弄與門牌。
- 任何失敗（timeout / non-2xx / parse error）→ `region_name = null`，事件照常記錄（fail-soft）
- accept-language 預設 `"zh-TW,en"`

**Cache 層**：`AppState` 把 `NominatimGeocoder` 包進 `CachedReverseGeocoder` decorator — process-local LRU、capacity 10000、TTL 1 小時、key = lat/lng 取 4 位小數（≈ 11m 格網）。同棟建築 / 同停車場的多筆事件命中同 key 直接回快取，Nominatim 呼叫量大幅降低。Cache miss 走原 lookup → 寫快取；TTL 過後視為 miss。負結果（`None`）也會快取避免重打失敗的 endpoint。Restart 後快取清空（無持久化）。Tests 直接注入 `StaticReverseGeocoder` 跳過 cache，行為可預期。

**換 provider**：`ReverseGeocoder` 是 trait，要切 Mapbox / Google / 自架時新增一個 impl 注入 `AppState` 即可，handler 不變。Nominatim 的 free-tier 適合 dev / pilot，production 通常要換家。

### Org timezone

`Org.timezone` 是 IANA 名稱（例：`Asia/Taipei`、`UTC`），新 Org 預設 `Asia/Taipei`。**純顯示用**：DB 一律存絕對 UTC 時間，server 不依此做 date-range 計算或保留期決定。admin-web 與未來 Flutter app 用此值 render 時間。

驗證：用內建 IANA primary-name 列表（[`api/src/services/timezone.rs`](src/services/timezone.rs)）。日後若想換 `chrono-tz` crate，只要改 `validate_timezone` 一處。

### Error codes（這個 change 新增）

| Code | HTTP | 說明 |
| --- | --- | --- |
| `INVALID_TRANSITION` | 422 | 狀態機不允許此 transition；body 含 `from` / `attempted` |
| `TRANSFER_DISABLED` | 403 | Org 關閉 transfer，不接受 transfer_out / transfer_in |
| `OUT_OF_ORDER` | 409 | 新事件的 `occurred_at_client` ≤ 該 AppUser 上一筆 |
| `STATE_LOCKED` | 409 | 有人非 off_duty 時無法調整 `transfer_enabled` 或 `location_tracking_enabled`；body 含 `on_duty_count` |
| `NOT_ON_DUTY` | 409 | 強制收班的目標目前已下班 |
| `INVALID_TIMEZONE` | 400 | timezone 不在 IANA primary-name 列表中 |

## 位置軌跡 / Location tracking

選擇性開啟的「上班期間定時記錄位置」功能。Worker 端每 60 秒採樣一次 GPS（且距離上次記錄超過 100 公尺才存），分批 push 到 server，server 留 90 天自動清掉，admin 可看軌跡或匯出 xlsx。客戶端的實作在 `add-location-tracking-app` change（依此 server 端 surface）。

### Toggle

`Org.settings.checkin.location_tracking_enabled: bool`，預設 `false`、隨 Org 建立時不存就視為 `false`。Admin 透過 `PATCH /orgs/me/settings` 帶 `{"location_tracking_enabled": true|false}` 切換。

State-lock：跟 `transfer_enabled` 共用同一個 lock — 只要任一 toggle 出現在 PATCH body，就會檢查 `current_org` 內是否有 AppUser 非 off_duty，有就回 `409 STATE_LOCKED`。理由是「mid-shift 改設定都會造成資料不一致」，沒必要分兩個 lock 規則。

### Mobile-facing endpoint `/app/checkin/locations`（Bearer auth）

| Endpoint | 說明 |
| --- | --- |
| `POST /app/checkin/locations` | body `{ pings: [{ lat, lng, accuracy?, occurred_at_client }, ...] }`；1..=100 筆；caller 從 token 推；toggle off 整批 `403 LOCATION_TRACKING_DISABLED`；個別 ping 失敗走 partial accept |

Response shape (201)：

```json
{
  "accepted_count": 3,
  "rejected": [
    { "index": 1, "code": "INVALID_PING_TIMESTAMP", "message": "..." }
  ]
}
```

即使 `accepted_count == 0` 仍回 201 — partial accept 的 channel 是 `rejected[]`，不是 4xx。Client 看 indices 對應自己 batch 的 row 處理。

Per-ping validation：`lat ∈ [-90, 90]`、`lng ∈ [-180, 180]`、`accuracy ≥ 0`（若有），`occurred_at_client` 必須是 RFC3339、不可為未來、不可超過 30 天前。

### Admin-facing endpoints（dashboard cookie + admin role）

| Endpoint | 說明 |
| --- | --- |
| `GET /checkin/users/:id/locations?before=&limit=&from=&to=` | 該 AppUser 的 ping 歷史，cursor 分頁 newest-first（default 200、max 1000）。可選 `from` / `to`（RFC3339）做區間過濾，沿用 export 的 90 天區間規則（單側允許；違反 `400 INVALID_RANGE`） |
| `GET /checkin/users/:id/locations/export?from=&to=` | xlsx 下載；`from` / `to` 必填且 `to - from ≤ 90 天`、`from ≥ now - 90 天`、`to ≥ from`；違反一律 `400 INVALID_RANGE` |

xlsx 結構：1 個 sheet `locations`、header row（`occurred_at_client (UTC)`、`occurred_at_server (UTC)`、`lat`、`lng`、`accuracy_meters`）+ 凍結首列、依 `occurred_at_client` 升序。

### 保留期 / TTL

`location_pings` 集合的 TTL index 建在 `occurred_at_server` 上，`expireAfterSeconds = 90 * 24 * 3600`。為什麼不用 `occurred_at_client`：client 時鐘亂跳會讓 client-time TTL 提早或延後刪資料；server-time 穩定但「retention reference」跟「logical time」分離。Mongo TTL monitor 約每 60 秒掃一次，所以實際保留是「90 天 ± 一分鐘」。

### 不做 reverse geocoding

Pings 不寫 `region_name`。原因：100 worker × 60s 取樣 × 8 hr 在 100m filter 後仍 ~150K pings/天，per-ping Nominatim 會打爆 free-tier。Admin map 直接畫 lat/lng；要文字化由 admin-web 自己 lazy-geocode（之後可加 cache，看 `Reverse geocoding cache` ROADMAP 條目）。

### Error codes（這個 change 新增）

| Code | HTTP | 說明 |
| --- | --- | --- |
| `LOCATION_TRACKING_DISABLED` | 403 | Org 沒開 toggle、AppUser 上傳 ping 整批被擋 |
| `INVALID_BATCH` | 400 | ping 為空或超過 100 筆 |
| `INVALID_RANGE` | 400 | export 的 `from` / `to` 缺欄位、>90 天、太老、`to < from` |
| `INVALID_PING_TIMESTAMP` | (per-ping) | 單筆 ping 不是 RFC3339 / 在未來 / 超過 30 天前；只出現在 `rejected[]`，不是 ApiError |
| `INVALID_PING_COORDINATES` | (per-ping) | lat / lng 範圍外或 accuracy 負值；只出現在 `rejected[]` |
