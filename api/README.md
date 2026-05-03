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

更詳細的設計請看 `openspec/changes/add-tenant-and-auth/design.md` 與 `openspec/changes/add-org-vanity-slug/design.md`。

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

## 成員退出 / 移除（owner / cooldown）

每個 Org 有一個永久 owner（建立 Org 的 user，`Org.owner_id`）。Owner 永遠是 admin，不能被降級、不能被踢、不能自離；唯一脫身路徑是 ROADMAP 上的 `transfer-org-ownership` 或 `delete-org`（皆未實作）。

Endpoints：

- `DELETE /dashboard-users/:id` admin 移除其他成員（不可移除自己；自離請走 `/me/leave`）
- `POST /me/leave` 認證使用者自離（owner 不可呼叫）
- `GET /dashboard-users/cooldowns` admin 列出當前 Org 的冷卻中 email
- `DELETE /dashboard-users/cooldowns/:email` admin 提前釋放冷卻

被移除 / 自離後寫入 `removed_memberships` marker（`org_id` + lowercase email），cooldown 7 天。`register mode=join` 時若命中未過期 marker → `EMAIL_IN_COOLDOWN`。Marker 由 Mongo TTL（`cooldown_until`）自動 GC。

| Code | HTTP | 說明 |
| --- | --- | --- |
| `OWNER_PROTECTED` | 403 | 操作目標是 Org owner（不可移除 / 不可自離 / 不可降級） |
| `EMAIL_IN_COOLDOWN` | 409 | 此 email 對該 Org 在冷卻期內，不能 rejoin |
