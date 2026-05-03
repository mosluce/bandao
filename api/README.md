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
