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

### 下一批 changes（已規劃，等 `add-tenant-and-auth` 落地後排程）

- **[cross]** `add-app-user-mgmt`：admin dashboard CRUD AppUser（無自助註冊）、AppUser 登入流程（多 session 切換、token 機制與 dashboard 分離）、AppUser 在 app 端首登流程。依賴：`add-tenant-and-auth` 提供 Org context。
- **[cross]** `add-checkin-events`：上下班 / 轉出 / 轉入四種打卡事件、Org-level toggles（轉出/轉入預設開、可關）、state-locked toggle（當日有人在班時不可調整）、admin 強制收班。每次打卡記錄座標 + reverse geocoded 區域名稱 + 可選手動輸入地點。依賴：`add-app-user-mgmt`。
- **[cross]** `add-location-tracking`：上班期間定時回傳定位形成軌跡（預設關、Org toggle 開啟），同樣受 state-locked 規則。儲存策略（取樣頻率 / 壓縮 / 保留期）需 explore。依賴：`add-checkin-events`。

### Side ideas（尚未排程）

- **[api]** Auto-checkout：偵測使用者忘記下班（例如超過班次上限或長時間無心跳）後自動補一筆下班事件。MVP 走 admin 強制收班，這是後續強化。
- **[cross]** OpenAPI codegen：admin-web / app 的 API 型別目前手寫鏡像 Rust DTO。等 schema 穩了改成從 OpenAPI / utoipa 生成，避免漂移。
- **[cross]** Per-Org email 唯一性：MVP 用全域唯一 email 換取登入流程簡單。若未來要支援同一人在多 Org 各持帳號，需要在登入引入 Org selector，連帶調整 `dashboard_users` 索引與 `/auth/login` 介面。
- **[infra]** Reverse geocoding provider 抽象：第一版可能直接接一家（Google / OSM Nominatim / Mapbox），上線後抽 trait 方便切換 / fallback / 在地化。
- **[admin-web]** 軌跡視覺化：軌跡上線後 dashboard 需要地圖頁顯示某 AppUser 某日的點線。預期使用 Leaflet 或 MapLibre。
- **[admin-web]** ESLint：MVP 暫時不裝 lint。確定 Nuxt 版本穩定後加回 `@nuxt/eslint` 模組與 `pnpm lint` 腳本。
- **[admin-web]** 升 Nuxt v4（≥ 4.4.4）：對齊現代 ecosystem、吃 v4 dev server / Vite 加速。動的時候：搬源碼到 `app/`、`@nuxtjs/tailwindcss` 視情況升 7.x、重新 `pnpm install` smoke 一輪。觸發：admin-web 要動結構（例如 `add-app-user-mgmt` 開工）時順手。
- **[cross]** 邀請連結加入需 admin 審核：`/register?code=...` 改成「申請加入 → admin 審批 → 成為 member」兩段流程。新增 pending membership 狀態、admin 端審核 UI / API、可選的拒絕理由。動機：對抗 invite link 被外流時的濫用，跟 vanity slug 這種「公開 URL」搭配特別合理。
- **[cross]** 註冊需驗證信箱：register 後寄驗證信，verify endpoint 點過才開啟完整功能。需要 token store、寄信 provider（SES / Resend / SMTP）抽象、未驗證帳號是否能 join 的策略。動機：防 typo / 防偽造 email 註冊、復原密碼前置條件。
- **[cross]** `delete-org`：owner 可解散整個 Org，cascade 刪除該 Org 的 memberships / sessions / slug 預留 / cooldown markers / 將來的 AppUser 與打卡資料；user identity 不刪（它在多對多模型下可能還是其他 Org 的成員）。動機：當 owner 想離開但組織也不再需要時的終極脫身路徑（與 owner transfer 互補）。需考慮：是否走 soft delete + 寬限期、確認流程、跨 collection 一致性。
