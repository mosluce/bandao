## Why

這是一個 monorepo（`api/`、`admin-web/`、`app/` 三元件，程式層零互相 import）。目標是**元件間 CI 獨立**，但目前每個 PR 都會把三個 workflow 全跑一遍——只動 `app/` 的 PR 也會空跑 8 分鐘的 Rust build 與 Nuxt build，純浪費。

天真的解法（在 `pull_request:` 加 `paths:` filter）**會害 PR 無法 merge**：main 的 branch protection 把三個 check（`fmt + clippy + test`、`typecheck + test + build`、`analyze + test`）都設為 **required**，一旦某 workflow 因 path filter 不觸發，它的 required check 永遠不回報，PR 就卡在「等一個不會來的 check」。

本 change 用 **git 內建 diff（不引第三方 action）** 做元件變動偵測：每個 workflow 在 PR 上照常觸發、其 required check 一律以同名回報成功，但**重步驟只在該元件真的有變動時執行**。順帶檢討 branch protection 的 `strict`（require up-to-date）設定——對單人維護 + 解耦元件，它帶來的 merge 摩擦大於其防護價值。

## What Changes

- 三個 workflow（`api.yml` / `admin-web.yml` / `app.yml`）改為 **conditional-skip** 模式：
  - `pull_request:` 觸發**不加** `paths:` filter（保證 required check 回報）。
  - job 內第一步用 `git diff <base.sha>...HEAD`（僅 git，無第三方 action）判斷該元件路徑是否變動。
  - **每個重步驟**（toolchain / cache / build / lint / test）掛 `if:` gate；沒變動就整串跳過，但 **job 仍以同名 check 結論 success**。
  - `push:` 分支維持既有 `paths:` filter（本來就正常）。
- **branch protection**：關閉 `strict`（require branches to be up to date），但**保留**三個 per-component check 為 required。
- 文件：`DEPLOY.md` 補一段說明 CI path-scoping 行為與 merge policy。

## Capabilities

### New Capabilities
- `ci-pipeline`: monorepo 的 per-component CI 契約——每元件一個穩定命名的 required check；PR 上一律回報，重步驟只在該元件變動時執行（git-only 偵測）；push 走 path filter；merge 不要求 up-to-date（strict off），但仍要求各 check 通過。

## Impact

- **`.github/workflows/*.yml`（3 個）**：新增 `fetch-depth: 0` 的 checkout、一個 `Detect changes` 步驟、把既有重步驟加上 `if: steps.changed.outputs.run == 'true'`。**check 名稱不變**，branch protection 的 required contexts 零改動。
- **branch protection（repo 設定，非程式）**：`gh api repos/mosluce/bandao/branches/main/protection/required_status_checks --method PATCH -f strict=false`（保留 contexts）。列為 change 的 ops 步驟。
- **DEPLOY.md**：新增 CI/merge policy 小節。
- **不改**任何 `api/`、`admin-web/`、`app/` 的產品程式碼。
- **已知取捨**：strict off 後，同元件的兩個並行 PR 若有語意衝突（各自綠、合起來壞）可能漏到 main；由 merge 後的 push CI 事後幾分鐘內轉紅作為 backstop（見 design）。
