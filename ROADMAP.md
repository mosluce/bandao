# ROADMAP

收集「想到了，但還沒要動手」的點子。這裡的東西**不會自動進入開發**，要動工前必須先走 OpenSpec / opsx 流程（`/opsx:explore` 或 `/opsx:propose`）轉成正式 change。

## 怎麼用

- 隨手記：有想法就加一條，不需要完整描述，但要寫到「未來的自己看得懂」。
- 不要在這裡寫實作細節或 task 拆解 — 那是 `openspec/changes/` 的事。
- 點子被認領動工時，從這裡刪掉、開對應的 opsx change，change 名稱可在備註裡 cross-link 回來。
- 點子被否決或不再相關，直接刪掉，不必留墓碑。

## 條目格式

```
- **[類別]** 一句話描述。可選：動機 / 觸發條件 / 風險。
```

類別建議：`api` / `admin-web` / `app` / `infra` / `db` / `cross`。

## Ideas

### 下一批 changes（已規劃）

- **[cross]** Marketing landing site at `bandao.ccmos.tw`：產品落地頁，給未下載 app 的潛在使用者看；同時把 `/privacy` 從 admin-web 搬出去，讓 admin-web 純做產品；store metadata 的 marketing URL 有正經去處。靜態站即可（單 HTML / Astro / Nuxt SSG），新 Zeabur service + DNS CNAME。可平行於 `app-release-prep`，不阻塞 app 送審；上線後回頭把 store 的 marketing/privacy URL 改指向。

### Side ideas（尚未排程）

- **[api]** Auto-checkout：偵測使用者忘記下班（例如超過班次上限或長時間無心跳）後自動補一筆下班事件。MVP 走 admin 強制收班，這是後續強化。
- **[cross]** OpenAPI codegen：admin-web / app 的 API 型別目前手寫鏡像 Rust DTO。等 schema 穩了改成從 OpenAPI / utoipa 生成，避免漂移。
- **[cross]** Per-Org email 唯一性：MVP 用全域唯一 email 換取登入流程簡單。若未來要支援同一人在多 Org 各持帳號，需要在登入引入 Org selector，連帶調整 `dashboard_users` 索引與 `/auth/login` 介面。
- **[admin-web]** ESLint：MVP 暫時不裝 lint。確定 Nuxt 版本穩定後加回 `@nuxt/eslint` 模組與 `pnpm lint` 腳本。
- **[admin-web]** 升 Nuxt v4（≥ 4.4.4）：對齊現代 ecosystem、吃 v4 dev server / Vite 加速。動的時候：搬源碼到 `app/`、`@nuxtjs/tailwindcss` 視情況升 7.x、重新 `pnpm install` smoke 一輪。觸發：admin-web 要動結構時順手。背景 agent 已排程 2026-05-17 檢查 nuxt/nuxt#34957 是否修復，可解 `nuxt: "3.21.2"` 的 exact pin。
- **[cross]** 邀請連結加入需 admin 審核：`/register?code=...` 改成「申請加入 → admin 審批 → 成為 member」兩段流程。新增 pending membership 狀態、admin 端審核 UI / API、可選的拒絕理由。動機：對抗 invite link 被外流時的濫用，跟 vanity slug 這種「公開 URL」搭配特別合理。
- **[cross]** 註冊需驗證信箱：register 後寄驗證信，verify endpoint 點過才開啟完整功能。需要 token store、未驗證帳號是否能 join 的策略。寄信 provider 抽象已經在 `add-forgot-password` 做掉（`EmailSender` trait + `ResendEmailSender`），這個功能可以直接複用，不用重新設計。動機：防 typo / 防偽造 email 註冊、復原密碼前置條件。
- **[cross]** 邀請成員用 email 邀請信：取代現在純靠分享 org code/slug 加入申請的方式，admin 直接輸入 email 主動寄邀請信。寄信 provider 抽象已經在 `add-forgot-password` 做掉，可以直接複用；還沒決定的是 token 的儲存結構要不要跟 reset token 共用一張表（`add-forgot-password` 的 design.md 刻意不現在就共用，理由見該文件 Non-Goals）、以及 email 邀請的加入是否要跳過現有的 join-request 審核步驟。
- **[cross]** `delete-org`：owner 可解散整個 Org，cascade 刪除該 Org 的 memberships / sessions / slug 預留 / cooldown markers / AppUser / 打卡事件與狀態；user identity 不刪（它在多對多模型下可能還是其他 Org 的成員）。動機：當 owner 想離開但組織也不再需要時的終極脫身路徑（與 owner transfer 互補）。需考慮：是否走 soft delete + 寬限期、確認流程、跨 collection 一致性。
- **[cross]** 即時看板 push：admin-web `/checkin` 目前是 30 秒輪詢。可考慮 SSE / WebSocket 真即時更新。觸發：admin 抱怨延遲或人多時 polling 太重。
- **[cross]** 多裝置 session 管理 UI：AppUser 可能在多裝置登入；目前沒地方看「我有哪些在線 session」也沒地方一鍵下線他裝置。`app_sessions` 已支援多筆，缺 UI / endpoint。
- **[infra]** `/readyz` deep health：目前 `/healthz` 只報 process 起來、不打 Mongo。等有監控之後加 `/readyz`，會 ping Mongo + 確認 tailscale 上線；給 SLO / 告警系統用，不影響 deploy 流。
- **[infra]** Staging 環境：MVP 只開 prod，靠 `git revert` 回滾。哪天人多 / risky 變動多，再開 `staging` 分支 + 第二個 Zeabur project。
- **[infra]** Backup 升級：daily/ 升級成 daily/ + weekly/ + monthly/ 三層保留，需要 S3 replication 規則或 host 上的 cron 把週末 / 月初的 dump 複製到 weekly/ / monthly/ prefix；目前先只留 30 天 daily。
- **[infra]** 監控與告警：Loki / Grafana / Sentry 任一接 api + Mongo host 的 log 與錯誤；restore drill 失敗自動 page 操作者。動的時候要考慮 secrets 管理跟成本。
- **[infra]** Queue / scheduler / worker 基礎設施：目前 production 是單一長跑的 Rust binary（Zeabur），完全沒有背景工作/排程/重試佇列的機制——`startup.rs` 唯一的「背景工作」是開機時跑一次的 drift-repair，不是常駐輪詢。`add-forgot-password` 的寄信失敗目前選擇不重試（fail-soft + log），就是因為這個基礎設施還不存在；`auto-checkout`（偵測忘記下班自動補下班事件）也需要類似的定時觸發能力。值得評估的方向：Mongo-backed outbox collection + in-process 常駐 tokio task 輪詢（最小改動，不需要新服務）vs. 獨立的 worker service（更乾淨但要多開一個 Zeabur service、多一份部署複雜度）。動機：一旦有兩個以上功能都想要「背景重試」或「定時觸發」，各自兜一份土炮方案的成本會超過先做一次共用基礎設施。
- **[cross]** 登入失敗鎖定：`/auth/login`、`/app/auth/login` 目前都沒有 rate limit，知道 org code 的人可以無限次數猜 AppUser／dashboard 密碼。連續失敗 N 次（例如 3 次）鎖定一段時間（例如 1 小時），admin 可手動解鎖。這個機制做出來之後，才算真正補上「輪替組織代碼」功能被拿掉後留下的風險缺口——`remove-org-code-rotation` 已經套用並移除該功能，是接受這個殘留風險、等這個機制上線來緩解的前提。
- **[cross]** Dashboard user 登入支援 OTP 兩階段驗證：帳密登入成功後多要求輸入一組動態驗證碼（email OTP，或 TOTP authenticator app）才核發 session，跟現有的密碼本身是兩件事。寄信 provider 抽象已經在 `add-forgot-password` 做掉，若走 email OTP 可以直接複用，不用重新設計。需要決定：預設開啟還是使用者自行選擇啟用、OTP 過期時間、失敗次數限制（可能跟上面的「登入失敗鎖定」共用同一套節流機制）、幾台裝置的 remember-this-device 要不要做。動機：目前帳密外洩（例如密碼重複使用被撞庫）就能直接登入，沒有第二層防線。
