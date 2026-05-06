## Context

「argus」是專案 day-1 的 codename。從 `add-tenant-and-auth` 開始所有 package 名、bundle id、storage 前綴、HTTP UA、文件都用這個名字。`grep -l argus` 在 90 個檔案有 hit。

決定改名後（詳見 proposal.md）需要在不破壞商業邏輯的前提下把所有識別字串掃過一遍。

## Goals / Non-Goals

**Goals:**
- 把「argus」字串完整替換為「bandao」/「班到」於所有 active 識別點
- 保留 archived changes 的歷史內容不動
- Server 端商業行為 zero behavioral change
- Client 端只有「視覺品牌」改變（app 名稱、storage prefix、UA）
- CI 全綠
- iOS / Android build 可以重新出 bundle 並安裝在 simulator

**Non-Goals:**
- 不改任何 spec 的功能定義（其他 capability 行為不動）
- 不改 DB schema、collection 名（存 server 端不變）
- 不重新設計 logo（使用者後續自己處理視覺資產）
- 不寫 migration script（既有測試裝置直接登出 / 重灌）
- 不動已 archived changes 內的 `argus` 字串（歷史紀錄要可追溯）
- 不重新命名 git commit history 中的 `argus`（不可能）

## Decisions

### D1：以「bandao」（pinyin）為英文識別、「班到」為中文顯示

**Why**：
- 所有程式碼識別字串、bundle id、storage key 等需要 ASCII，必須英文 / 拼音
- 顯示給人看的字串（UI、文案、Logo）用「班到」中文
- Pinyin「bandao」覆蓋了 namespace；中文「班到」覆蓋顯示層

具體規則：

| 場合 | 用什麼 |
|---|---|
| `Cargo.toml` `name` | `bandao-api` |
| `package.json` `name` | `bandao-admin-web` |
| `pubspec.yaml` `name` | `bandao_app` |
| iOS bundle id | `tw.ccmos.app.bandao` |
| iOS BGTask id | `tw.ccmos.app.bandao.queue-drain` |
| Android `applicationId` | `tw.ccmos.app.bandao` |
| SecureStorage prefix | `bandao.location_tracking.*` |
| HTTP User-Agent | `bandao-api/0.1.0 (https://github.com/mosluce/bandao)` |
| admin-web `<title>` | `班到 admin`（顯示層中文） |
| Login screen 標題 | `班到`（顯示層中文） |
| iOS `CFBundleDisplayName` | `班到`（顯示層中文） |
| Android app label | `班到` |
| README 標題 | 主標題用「班到」、subtitle 用 `(bandao)` |

### D2：不寫 storage migration

**Why**：
- 既有 secure storage 內容包括：login token、pending queue、consent flag、`last_clean_stop`
- 寫 migration code 把舊 prefix 內容搬到新 prefix 是純粹一次性的工作量
- Pre-launch 階段測試裝置數量少（< 5 台），等於登出重灌
- 寫 migration 反而增加 future maintenance 負擔

**Trade-off**：所有測試裝置上的 user 要重新 login + 重新給 location consent。可接受。

### D3：不改 archived changes 內的字串

**Why**：
- archived changes 是「在那個時間點寫的提案 / 設計 / 任務」歷史紀錄
- 改了等於偽造歷史，未來查 root cause 會困惑
- 範圍變大、merge conflict 風險增加
- 訊息上仍然清楚（讀者看到 `argus` 知道是 day-1 codename）

**例外**：active `openspec/specs/*/spec.md` 內如果有 `argus` 字面（多半是 example code），改成 `bandao` 因為這是 active spec，會影響後續 change reading。

### D4：根目錄路徑 + GitHub repo rename — 使用者決定

兩個都是 **可選但有副作用** 的動作：

#### 根目錄 `/Volumes/Backup/Workspace/ccmos/argus`
- Pros：跟新 brand 一致
- Cons：所有 IDE 設定、shell history、git remote URL 都會失效
- 決策時機：apply 開始前問使用者；如果 yes，先 stash → mv → restore

#### GitHub repo `github.com/mosluce/argus`
- Pros：新 contributor / link share 看到正確品牌
- Cons：所有外部連結失效（GitHub 會 redirect 一段時間，但不保證永久）；CI badge URL、本地 `git remote -v` 要更新
- 決策時機：archive 後問使用者，使用者自己在 GitHub web 設定 + 本地 `git remote set-url`

兩個都不在 tasks.md 強制範圍內，由使用者決定後手動跟進。

### D5：Test assertion 字串改名範圍

某些 integration test 含 hardcoded `argus` 字串（例：`"argus admin"` 在 login flow assertion）。這些**改成 `bandao`** 因為他們驗證 UI 文字。

但其他 test 內例如 `register_admin("admin@example.com", "Acme")` 中的 `Acme` 是測試用 Org 名，**不是品牌字串、不要改**。

判斷標準：grep `argus` 看每一筆是不是「這次 rebrand 的字串」 — 屬於識別字串才改。

### D6：Git commit history 不動、根目錄 .git 保留

- commit history 中所有 `chore(argus): ...` 等保留
- 新 rebrand commit message 開始用 bandao
- `.git/` 不動

### D7：archived changes 不動的代價

歷史 changes（如 `add-app-checkin`）內 `pubspec.yaml` 修改紀錄會留著舊名 `argus_app`。不影響 active code 也不影響 spec — 純歷史。可接受。

### D8：Capability `brand-identity` 為什麼存在

純粹為了讓 openspec 接受這個 change（schema 要求 ≥ 1 個 delta）。Capability 的 requirement 文字記錄「brand identity 是 bandao」這件事 — 未來其他 change 動到 bundle id / storage prefix / UA 時，要 reference 這個 capability。

兩個 requirement 已足夠：
1. 程式碼識別字串統一在「bandao」前綴下
2. 顯示給人看的中文識別字串為「班到」

## Risks / Trade-offs

- **既有測試裝置 storage 失效**：D2 已接受。
- **某個漏網 grep**：grep `argus` 全部跑、CI test 跑、admin-web build / iOS build / Android build 都跑一次 catch 漏網。risk 中等。
- **iOS code-signing 成本**：bundle id 改動會讓 development provisioning profile 重新產生。pre-launch 階段這是常規動作。risk 低。
- **Android applicationId 改動**：開發者 device 上的 app 視為新 app（不會更新而會並存）。pre-launch 可接受。
- **CI badge URL（.github/workflows）**：path filter 改 `argus` 路徑的 hard-coded 沒有，因為都是 `app/**` 等子目錄 path。只要 GitHub repo rename 才需動 badge URL。
- **External 連結失效**：本地不會接到外部連結，但 commit message / PR 中可能有 `github.com/mosluce/argus` 的 link — repo rename 後 GitHub redirect 短期 OK，長期不保證。

## Migration Plan

純 in-place 替換，沒有 schema 演化：

1. branch 開新 commit chain（不直接做 `mv argus bandao`，等 commit 完再決定根目錄改不改）
2. 程式碼識別 / 文件 / test assertion 全部 grep + replace
3. 跑 CI（cargo / pnpm / flutter）三條全綠
4. iOS / Android 重新 build 並 install 到 simulator 確認啟動
5. archive
6. 使用者決定根目錄改名 / GitHub repo rename / `git remote set-url`

**Rollback**：純 git revert。沒有資料層改動。
