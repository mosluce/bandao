# argus

多租戶簽到系統。包含後端 API、管理後台、終端使用者 App，以及共用的 MongoDB 資料層。

> 系統設計與規範請看 [`AGENTS.md`](./AGENTS.md)；尚未排程的點子記在 [`ROADMAP.md`](./ROADMAP.md)；變更流程使用 [OpenSpec / opsx](./openspec/)。

## 模組

| 路徑 | 技術 | 說明 |
| --- | --- | --- |
| [`api/`](./api/) | Rust + axum + MongoDB | 唯一與資料庫直接通訊的服務層，提供 dashboard 與 app 共用的 HTTP API |
| [`admin-web/`](./admin-web/) | Nuxt 3 + TypeScript | 給 Org admin / member 使用的管理後台 |
| [`app/`](./app/) | Flutter（尚未落地） | 終端使用者打卡 App，預計 iOS + Android |

## 開發前置

- **Rust** — 由 `api/rust-toolchain.toml` 鎖定；建議用 [rustup](https://rustup.rs/) 或 asdf。
- **Node 20+** — `admin-web/` 開發用；建議用 nvm / asdf / volta。
- **Docker** — 跑本地 MongoDB 與 `api/` 的整合測試。
- **MongoDB 7.x** — 透過 `docker-compose.yml` 起本地實例。

## 快速開始

開三個 terminal，依序：

```bash
# Terminal 1 — MongoDB（首次會自動 pull image）
docker compose up -d mongodb

# Terminal 2 — API（http://localhost:8080，預設）
cd api
cargo run

# Terminal 3 — admin-web（http://localhost:3000）
cd admin-web
cp .env.example .env       # 首次
pnpm install               # 首次
pnpm dev
```

如果本機 8080 被佔，改用其他 port 並讓 admin-web 對應：

```bash
# api
ARGUS_LISTEN_ADDR=127.0.0.1:9090 ARGUS_ALLOWED_ORIGIN=http://localhost:3000 cargo run

# admin-web .env
NUXT_PUBLIC_API_BASE_URL=http://localhost:9090
```

預設 Mongo 連線字串：`mongodb://argus:argus@localhost:27017/argus?authSource=admin`

更詳細的環境變數與測試流程：
- [`api/README.md`](./api/README.md)
- [`admin-web/README.md`](./admin-web/README.md)

## 變更工作流程

argus 採 **spec-driven** 開發，所有非瑣碎的變更都先走 OpenSpec / opsx：

1. 點子先進 [`ROADMAP.md`](./ROADMAP.md)（不立刻動工）
2. 要做時 `/opsx:propose` 產出 proposal + design + specs + tasks
3. `/opsx:apply` 依 task 逐項實作
4. 完成後 `/opsx:archive` 封存到 `openspec/changes/archive/`

詳見 [`AGENTS.md`](./AGENTS.md) 的「工作流程」章節。
