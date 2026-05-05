# AGENTS.md

此檔為 argus 專案中所有 AI coding agent（Claude Code、GitHub Copilot、OpenCode、Codex、Cursor 等）的共通指引。各家工具的私有設定（slash command、skill、prompt）放在 `.claude/`、`.github/`、`.opencode/`，本檔只放跨工具一致的規範。

## 專案概觀

argus 是一個全端服務，由三個應用 + 一個資料層組成：

| 模組 | 技術 | 角色 |
| --- | --- | --- |
| `api/` | Rust | 後端 API 服務，唯一與資料庫直接通訊的層 |
| `admin-web/` | Nuxt (Vue 3 + TypeScript) | 內部管理後台，僅給營運/管理者使用 |
| `app/` | Flutter | 終端使用者 App（iOS / Android） |
| `mongodb` | MongoDB | 共用資料庫，由 `api/` 管理 schema 與 migration |

> 上表的目錄結構為預期的 monorepo 佈局；若實作時調整，請同步更新本檔。

## 核心原則

1. **Spec-driven**：所有非瑣碎的功能、行為變更都要先走 OpenSpec / opsx 流程（`openspec/`），先 propose 再 apply，最後 archive。不要跳過 spec 直接實作新行為。
2. **API 是唯一資料源**：`admin-web/` 與 `app/` 都透過 `api/` 存取資料；前端不得直接連 MongoDB，也不得在前端塞商業邏輯應由後端決定的判斷（權限、計價、狀態流轉等）。
3. **型別由後端輸出**：API 的 request/response 型別以 Rust 為 source of truth，Nuxt 端的 TypeScript 型別與 Flutter 端的 Dart model 應由 OpenAPI schema 產生，避免手動同步。
4. **小步快跑**：一個 PR 對應一個 OpenSpec change；不在同一 PR 內混合 refactor 與功能。

## 工作流程

1. **點子收集** — 想到但還沒要做的事，記到 `ROADMAP.md`，**不要**直接開 opsx change。
2. **propose** — `/opsx:propose` 描述要做什麼、為什麼，產出 proposal、design、specs、tasks。
3. **explore**（選用） — `/opsx:explore` 在 propose 前釐清模糊需求。
4. **apply** — `/opsx:apply` 依 tasks 逐步實作，每完成一項就更新 task 狀態。
5. **archive** — `/opsx:archive` 完成後封存到 `openspec/changes/archive/`，並把對應 spec 寫入 `openspec/specs/`。
6. **commit**（archive 後自動執行）— archive 完成後立即把 working tree commit 掉（`chore(openspec): archive <change-name> and sync specs`，如果同步了 spec 才加 `and sync specs`）。**不要**累積多個 archive 才一起 commit，會混進不同 change 的 code，diff 變得難審。掃過 staged paths 確認沒有 secrets（`.env`、credential、private key）再 commit。

每次開始新的工作前，先查 `openspec/changes/` 是否已有進行中的 change；有就接手，沒有才新建。當使用者丟出新想法時，先判斷是「現在要做」還是「先記下來」— 若是後者，加進 `ROADMAP.md` 而不是立刻產 change。

## 各模組規範

### `api/`（Rust）

- 邊界：HTTP handler 只處理協定（路由、驗證、序列化），業務邏輯放在 service / domain 層。
- 錯誤：用具名 enum 錯誤型別，不要 `anyhow` 串到 handler。對外輸出統一錯誤格式。
- MongoDB 存取集中在 repository 層，不要在 handler 直接拿 collection。
- 寫測試優先：每個對外 endpoint 至少一個整合測試（打真 MongoDB，不 mock）。

### `admin-web/`（Nuxt）

- 嚴格 TypeScript（`strict: true`），不放縱 `any`。
- 從 API 產出的型別只進不出，不要在前端重新定義 server 已定義過的結構。
- 元件以 Composition API + `<script setup lang="ts">` 為預設。
- 狀態管理優先用 server-side fetching（`useFetch` / `useAsyncData`），需要跨頁共享時才引入 store。

### `app/`（Flutter）

- Dart 走 null safety；公開 API 一律標型別，不仰賴推斷。
- 架構分層：UI（widget）→ state（**Riverpod 2**）→ data（repository）→ network（**dio**，未來接 OpenAPI 生成的 client；目前手寫 mirror）。
- 路由：**go_router**（declarative + redirect）。
- 不在 widget 內直接呼叫 HTTP；一律透過 repository。
- 平台差異（iOS / Android）以條件式分支收斂在 platform 層，不污染 widget。
- 詳細結構、跑起來方式、dev menu 用法見 [`app/README.md`](./app/README.md)。

### MongoDB

- Collection / 索引 / migration 變更都要在 OpenSpec change 裡留紀錄。
- 不要在前端或 App 直接連線；不要把 ObjectId 直接吐到對外 API（用 string id）。

## 通用規範

- **commit / PR 風格**：Conventional Commits（`feat:`、`fix:`、`refactor:`…）。一個 PR 對應一個 OpenSpec change。
- **語言**：對話與 spec 文件可用中文；程式碼識別字、commit message、code comment 一律英文。
- **註解策略**：預設不寫註解；只在「為什麼這樣做」非顯而易見時才加，不解釋「在做什麼」。
- **不破壞性改動**：刪除/重構公用介面前先確認影響面；跨模組變更要在 OpenSpec change 內列出受影響的模組。

## Agent 行為守則

- 動手前先讀 `openspec/` 對應的 spec 與正在進行的 change，確認自己在做的事有 spec 背書。
- 模糊或衝突的需求，先開 `opsx:explore` 釐清，不要憑感覺實作。
- 大規模搜尋與探索優先用 agent / subagent，避免主對話塞滿無關內容。
- UI / 前端改動完成前要實際跑起來驗證；型別檢查通過不等於功能正確。
- 不要繞過 hook、不要 `--no-verify`、不要 `git push --force` 到主幹。
