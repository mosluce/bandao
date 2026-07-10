## Context

三個 workflow（`api.yml`、`admin-web.yml`、`app.yml`）目前：`push:` 有 `paths:` filter（正常），`pull_request:` **沒有** filter → 每個 PR 三個都全跑。main 的 branch protection：三個 check 皆 required，且 `strict: true`（require up-to-date）。

目標是讓 PR 只實際跑「有變動的元件」，同時**不破壞 required checks**。核心限制：GitHub 上「因 path filter 未觸發的 workflow」其 required check 會永遠 pending → 擋 merge。因此不能單純在 `pull_request:` 加 `paths:`。

## Goals / Non-Goals

**Goals:**
- 只動到有變動元件的 PR 才跑其重步驟；其餘元件的 required check 秒回報成功。
- branch protection 的 required contexts / check 名稱**零改動**。
- 僅用 git 內建能力偵測變動，不引第三方 action。
- 檢討並（傾向）關閉 `strict`，降低單人維護的 merge 摩擦。

**Non-Goals:**
- 不改任何元件的產品程式碼、測試內容或 build 指令本身。
- 不引入 merge queue（未來多人協作再議）。
- 不合併三個 workflow 成一個（維持一元件一 workflow 一 check）。

## Decisions

### D1. Gate 步驟，不 gate job（branch-protection 安全性）
重步驟各自掛 `if: steps.changed.outputs.run == 'true'`，**job 本身一律跑到結束、結論 success**。不使用 job-level `if:` 跳過整個 job。
- **為何**：required check 的 job 若被 job-level `if:` skip，GitHub 對「skipped」結論的處理不一致（可能停在 pending、或當中性），有機會重演「等一個不會來的 check」死結。讓同名 check 永遠回報 success 是唯一穩妥解。
- **成本**：即使跳過，job 仍會起 runner + checkout（約數十秒）。相對 8 分鐘 build 可忽略。

### D2. git-only 變動偵測
`actions/checkout` 用 `fetch-depth: 0`；偵測步驟：
```
if [ "${{ github.event_name }}" != "pull_request" ]; then
  echo "run=true" >> "$GITHUB_OUTPUT"        # push 已被 paths filter
elif git diff --name-only "${{ github.event.pull_request.base.sha }}...HEAD" \
     | grep -qE '^(api/|\.github/workflows/api\.yml)'; then
  echo "run=true" >> "$GITHUB_OUTPUT"
else
  echo "run=false" >> "$GITHUB_OUTPUT"
fi
```
- **`fetch-depth: 0`**：預設 shallow 沒有 base commit，無法算 diff。全歷史 checkout 對 `app/`（歷史較大）多幾秒，可接受。
- **`base.sha...HEAD`（三點）**：merge-base 起的差異＝「這個 PR 改了什麼」，不含 base 分支自己的新 commit。
- **cwd 注意**：`app.yml` 設了 `defaults.run.working-directory: app`；偵測步驟需在 repo root 跑（`git diff --name-only` 預設輸出 repo-root-relative 路徑，但為保險該步驟以 step-level `working-directory:` 覆寫回 repo root，或用 `git -C "$GITHUB_WORKSPACE"`）。
- **為何 git-only**：使用者要求不引第三方 action（供應鏈 / 版本維護面）；git diff 足夠。

### D3. push 維持 paths filter
`push:` 分支的 `paths:` 不動；只有 `pull_request:` 走 D2 偵測。
- **為何**：push（合入 main）用 path filter 不會死結——沒有「required 但不觸發」的問題，因為 push 後的 check 不是 PR gate。維持現狀最小改動。

### D4. 關閉 strict（require up-to-date），保留 required checks
`required_status_checks.strict = false`；`contexts` 三個 check 保留。
- **為何**：strict 唯一實質防護是「語意衝突」（兩 PR 各自綠、合起來壞）。本 repo 三元件程式零互 import，跨元件語意衝突不存在；風險僅限「同元件並行兩 PR」，單人維護少見。strict 帶來的「每次 merge 逼所有 open PR update+重跑 CI」摩擦（見 #33）對單人是純負擔。
- **Backstop**：strict off 若真漏語意衝突，merge 後 main 的 push CI 仍會跑並轉紅，事後幾分鐘發現。
- **可逆**：未來多人協作 / 同元件常並行時，重開 strict 或改用 GitHub merge queue。

## Risks / Trade-offs

- **偵測步驟 cwd 出錯** → grep 對不到路徑會誤判「無變動」而跳過真正該跑的 build。以 repo-root 執行 + 明確 glob 緩解；驗證時用真實 PR 確認。
- **`fetch-depth: 0` 拖慢 checkout** → app 歷史較大；仍遠低於被省下的 build 時間。
- **strict off 的語意衝突窗口** → 由 post-merge push CI 作 backstop；單人 + 解耦元件下風險低。
- **base.sha 語意** → 用 PR event 的 `base.sha`，非 `github.sha`（後者在 PR 是 merge commit）；三點 diff 確保只看 PR 自身變動。

## Migration Plan

1. 逐一改三個 workflow（互不相依，可分別驗證）。
2. 開一個「只動 app/ 的 no-op PR」驗證：`analyze + test` 全跑、`fmt + clippy + test` 與 `typecheck + test + build` 秒回報 success；PR required checks 全綠、可 merge。
3. 反向驗證：只動 `api/` 的 PR，api 全跑、另兩者 skip。
4. PATCH branch protection 關 strict（保留 contexts）。
5. DEPLOY.md 補 CI/merge policy。
6. Rollback：workflow 改動可單獨 revert；strict 可隨時 PATCH 回 true。

### D5. 偵測結果輸出 `::notice::`
偵測步驟在判定「該元件無變動、跳過重步驟」時，輸出一行 `::notice::`（例如 `no api/ changes — skipping heavy steps`），讓 PR 的 Checks 頁與 job summary 一眼看出這個 PR 實際驗了哪些元件。
- **為何**：conditional-skip 後，「秒綠」的 check 不再等於「真的跑過」；一行 notice 讓 reviewer 不必翻 log 就知道哪些被跳過，避免誤以為都驗了。

### D6. 不抽 reusable workflow；三份各自寫，未來 reuse 走 composite action
三個 workflow 共用的只有 ~10 行 scaffold（`checkout fetch-depth: 0` + detect step + `::notice::`）；差異部分（各自的 toolchain `uses:` + cache + build/lint/test）佔絕大多數且互不相同。本版**三份各自寫**，不做抽象。
- **為何不用 reusable workflow（`workflow_call`）**：(1) 它會把回報的 check context 變成 `caller_job / reusable_job` 複合名 → 打破 branch protection required contexts（`fmt + clippy + test` 等固定字串），與本 change「check 名稱不變、branch protection 零改動」直接衝突；(2) 重步驟是 `uses:` 的 toolchain，塞不進字串參數，硬做只能在單檔內 `if: inputs.component == ...` 三路分支，把三元件重新耦回一個檔。投報比為負。
- **未來 reuse 的正解**：若加第四元件或 detect 邏輯長大到值得統一，抽成 **composite action**（`.github/actions/detect-changes/`），在現有三個 job 內以 `uses: ./.github/actions/...` 呼叫——能 DRY 又**不改 job/check 名稱**。

## Open Questions

- （無）本版決策已收斂。
