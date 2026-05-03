## Context

`org-tenancy`（落地於 `add-tenant-and-auth`）目前讓每個 Org 持有一個 random 10-char `code`，同時扮演「join 授權」與「枚舉防禦」兩個角色。code 的 alphabet（`23456789ABCDEFGHJKLMNPQRSTUVWXYZ`）刻意排除外觀混淆字元，搜尋空間 32¹⁰ ≈ 10¹⁵ 足夠抵抗暴力嘗試。

但 admin 對 random code 的痛點集中在「人類介面」：
- 口頭告訴新 member「H、F、Q、W、M、S、7、V、Z、B」很容易出錯
- 印在 onboarding 卡片 / 會議室 QR / 行銷物料上時不夠體面
- 行銷需求希望 invite link 帶語義（`?code=acme` vs `?code=HFQWMS7VZB`）

直接把 code 換成自選短字串會犧牲枚舉防禦。本設計把兩個角色拆開：保留 random `code` 不動（security backbone），新增可選 `slug`（人類介面）。

利益相關者：
- **Org admin**：需要設 slug、清 slug、看到當前 grace 狀態
- **Org member / 邀請來源**：用 invite link 加入時 input 同時吃 code / 現役 slug / grace slug
- **未來 AppUser onboarding**：同 endpoint 受惠，不需另外處理

## Goals / Non-Goals

**Goals:**
- Org 可選持有人類友善的 slug，admin 隨時設 / 改 / 清
- slug 失效時有 grace period，避免 invite link 一夕全壞
- slug 在 grace 期間鎖在原 Org（防 squatting / 命名借屍還魂）
- 既有 random code 行為一字不改（包括 `POST /orgs/me/code/rotate`）
- 現有 Org（沒有 slug）行為完全不變，零 migration cost
- API 與 admin-web 對應 UI 同步交付

**Non-Goals:**
- 不做付費 tier（記在 ROADMAP，本期任何 admin 都可設）
- 不做爭議仲裁機制：搶到 `acme` 之後合法 ACME 公司來了 → 走人工客服 / 帳號層級處理
- 不做品牌 reserved list（不擋 `google` / `apple`）
- 不做 slug 顯示樣式國際化（lowercase `[a-z0-9]` 只接 ASCII；中文 / Unicode slug 不支援）
- 不做 path-style URL（`/o/acme`）— invite 仍然走 query string `?code=acme`
- 不做 slug 全文搜尋 / 模糊匹配，輸入必須精確

## Decisions

### Decision 1: 並存模型（code + slug）vs 替代模型

選 **並存**。

理由：
- code 已經是部署中的安全屏障，替換的話需要重新設計枚舉防禦
- admin 心智模型清楚：「code 永遠在、slug 是錦上添花」
- 既有 invite link / 整合測試 / UI 全可保留

替代方案（slug 取代 code）的代價：
- alphabet 放寬 + 變長度後枚舉空間掉到 36⁴–36⁶，必須加 rate limit
- 既有 spec 的「globally unique 10-char」invariant 整段重寫
- migration：既有 Orgs 怎麼處理（強制設 slug？）

### Decision 2: slug 格式 `^[a-z0-9]{2,24}$`，server 端強制 lowercase

理由：
- lowercase 統一避免 `acme` vs `ACME` 衝突的判定
- 不開放 `-` / `_`：避免 `acme-corp` vs `acme_corp` vs `acmecorp` 的視覺混淆
- min 2：放寬到極致也好；2 字元 namespace 1296 個 slot 不算稀缺
- max 24：對齊 GitHub org name；URL 可讀且不爆 OG / metadata
- 純 ASCII：i18n / IDN homograph attack 免疫

UX：admin-web 輸入框可在 client 端自動 lowercase 補齊體驗，但 server 仍是 source of truth。

### Decision 3: 單一 `slug_reservations` collection 同時涵蓋 active + grace

選 **單表 reservation 模型**：active 與 grace 共用同一個 collection，靠欄位區分。

```
slug_reservations {
  _id: ObjectId
  slug: String                    // lowercase, unique 索引在這一欄
  org_id: ObjectId                // 持有者（active 或 grace 都記著）
  expires_at: Option<DateTime>    // None = active 持有；Some = grace、TTL 自動清
  created_at: DateTime
}
```

索引：
- `{ slug: 1 }` unique
- `{ expires_at: 1 }` TTL（`expireAfterSeconds: 0`，Mongo 對 None / null 不會清）
- `{ org_id: 1 }`（admin 介面看自家 reservation 用）

`Org.slug` 欄位仍存（admin-web 顯示與 / register response），但 source of truth 是 reservation collection。

理由：
- 不需要切兩個 collection 後協調跨表 uniqueness
- 唯一性靠 Mongo 原生 unique index 強制執行，無 race
- TTL 條件式生效（expires_at = None 不清，等於永久 active）
- lookup 一次查 reservation 即可
- 改名/清除只是 update reservation row（不再 insert/delete 兩處）

### Decision 4: 用「unique index 的原子性」取代 Mongo transaction

slug 必須跨 active + grace 全域唯一。我們用 reservation collection 的 unique index 作為仲裁者，**不需要 Mongo transactions**：

```
SET org A 的 slug 為 "newslug"：

  1. 試 insert slug_reservations
       { slug: "newslug", org_id: A, expires_at: None, created_at: now }
     ↳ duplicate key → SLUG_TAKEN（無論衝突來自 active 或 live grace）
     ↳ 成功 → 進步驟 2

  2. 把 A 的舊 active reservation（如果有）改成 grace：
       update_one { slug: <old>, org_id: A, expires_at: None }
                  { $set: { expires_at: now + 30d } }
     ↳ 失敗極少見（資料一致性破壞），記 error 但不 abort

  3. update orgs：{ $set: { slug: "newslug", slug_changed_at: now } }

  4. 若步驟 2 或 3 失敗，盡力 rollback 步驟 1（delete reservation by id）
     並把 error 往上回。

CLEAR org A 的 slug：

  1. update slug_reservations
       { slug: <current>, org_id: A, expires_at: None }
       { $set: { expires_at: now + 30d } }

  2. update orgs：{ $set: { slug: null, slug_changed_at: now } }
```

LOOKUP（join 用）：

```
1. find_one slug_reservations { slug: input }
2. 命中 → 看 expires_at：
     - None        → active，回 org_id
     - Some > now  → grace，回 org_id
     - Some <= now → 視為過期（雖然 TTL 通常 60 秒內就清掉），回 None
3. 沒命中 → INVALID_ORG_CODE
```

理由：
- testcontainers-modules `Mongo::default()` 是單節點，無法跑 multi-document transaction。要改成 replica set 需要自定 image + `--replSet rs0` + `rs.initiate()`，gating 整個 change 不值得
- Unique index 給的是 **document-level atomicity**：要嘛 insert 成功（reservation 屬於我），要嘛失敗（slug 已被持有），不會 race
- 步驟 2/3 失敗的恢復路徑簡單：刪掉剛 insert 的 reservation，slug 立即釋出
- 不犧牲任何 spec 行為（spec 只描述外部行為，不要求 transaction）

替代方案考慮過：
- Mongo transaction（需 replica set，testcontainers 麻煩、CI 路徑複雜化）
- Embedded array on Org（uniqueness 跨 doc 沒辦法用單一 index 強制）
- Lock collection / advisory locks（過設計）

### Decision 5: Rate limit 用 `slug_changed_at` 欄位 + 30 天視窗

```
SET slug 邏輯：
  if org.slug == None and 沒有 slug_changed_at:    # first SET
      pass
  elif now - slug_changed_at < 30 days:
      reject SLUG_CHANGE_TOO_SOON { retry_after: slug_changed_at + 30d }
  else:
      proceed
```

DELETE 同樣 consume rate limit（避免「set → delete → 改別人 → 再 set」繞過）。

理由：
- 用 last-changed timestamp 比另開「pending change」狀態簡單
- 30 天與 grace 對齊，讓 Org 永遠最多持有 2 個 slug 的 invariant
- first SET 免限制：剛建好的 Org 應該能無痛挑 slug

替代方案（沒採）：
- token bucket：對 slug 場景過設計
- 直接看 history count：要 join history collection 才能算，比 timestamp 慢

### Decision 6: Lookup 路由用 input format 分流

```
join input → format
  matches ^[a-z0-9]{2,24}$        → slug 路徑
                                     1. orgs.find_one({slug: input})
                                     2. history.find_one({slug, expires_at>now})
  matches ^[2-9A-HJ-NP-Z]{10}$    → code 路徑
                                     orgs.find_one({code: input})
  其他                              → INVALID_ORG_CODE
```

兩個格式在字符集和大小寫上完全不重疊（slug = lowercase + digits、code = uppercase + 限定 digits），不會誤判。

替代方案：先試 code、再試 slug、再試 history → 多兩次 db 查詢，無收益。

### Decision 7: Reserved word 寫成常數陣列，不開 admin 介面管理

```rust
// api/src/auth/slug.rs
pub const RESERVED_SLUGS: &[&str] = &[
    // API path 第一層
    "auth", "me", "orgs", "users", "dashboard-users",
    // 系統保留字
    "admin", "api", "app", "www", "dashboard", "login",
    "register", "logout", "support", "help", "status",
    "billing", "settings", "new", "create", "join",
    "root", "signup", "signin", "oauth", "callback",
    // 專案
    "argus",
];
```

理由：
- MVP 不需要動態管理介面（沒有「super admin」概念）
- code 中的常數可以納入測試覆蓋
- 未來要動只是加一行 + 改 spec

如果某個合法 Org 真的需要這些字 → 走客服 / 程式碼層 patch（罕見）。

### Decision 8: Invite URL 維持 `?code=`，不另開 `?slug=`

理由：
- admin-web 既有 copy / paste 邏輯都圍繞 `code` query
- 後端反正由 input format 分流，欄位名怎麼叫無妨
- 名稱叫 `code` 從使用者觀點是「邀請碼」，含意涵蓋 random + slug，可接受

UI 顯示時自動選最人性化的：

```
有 slug：  https://app/.../register?code=acme
沒 slug：  https://app/.../register?code=HFQWMS7VZB
```

### Decision 9: Grace TTL 用 mongo TTL index 自動清

```
db.org_slug_history.createIndex(
  { expires_at: 1 },
  { expireAfterSeconds: 0 }
)
```

理由：
- mongo TTL monitor 每 60 秒掃一次，足夠
- 不需要應用層 cron
- 與 dashboard_sessions 的 TTL 設計一致

注意：TTL 不是即時，30 天到期後可能還躺幾分鐘才被刪。lookup 時必須額外比 `expires_at > now` 才能精確判定，不能只仰賴 TTL 已清。

## Risks / Trade-offs

- **[Slug enumeration]** → mitigation：slug-set endpoint 對「taken」回應跟「reserved」、「format invalid」、「rate limited」回應一律 4xx，不洩漏 Org 存在；register endpoint 對 invalid slug 與 invalid code 都回 `INVALID_ORG_CODE`，不分流；rate limit 用 IP / session 防爬（既有 rate limit 不在本 change 範圍，但 register endpoint 將來補）。
- **[Squatting via 30 天輪換]** → mitigation：rate limit 30 天對齊 grace 期，DELETE 也計入。Org 同時最多持有 2 個 slug。仍可注冊多個 Org 並各自佔不同 slug，但這已超出本 change 範圍（多 Org 之間的命名空間政策另議）。
- **[Grace 期間人為衝突]** → mitigation：明文走人類處理（spec 寫死「Non-Goals」），UI 對 set-slug 失敗顯示「目前由其他 Org 在 grace 中持有，X 月 X 日後可用」（不洩漏對方資訊）。
- **[Mongo transaction 複雜度]** → mitigation：slug set 操作頻率低（每 Org 每 30 天最多 1 次），效能影響可忽略。replica set 在 docker-compose / 整合測試環境都已具備（mongodb 7 默認 replica set on testcontainers? 待驗證 — 若不具備 fallback 用 best-effort + retry，列為 open question）。
- **[既有 invite link 行為改變]** → mitigation：既有 link 是 `?code=<10-char>`，由 format 分流落到 code 路徑，行為一致。零 migration。
- **[admin 不會用 / 設錯]** → mitigation：UI 提供 inline validation hint（格式 / reserved / taken / cool-down 倒數），錯誤訊息中文化。
- **[reserved list 將來新增 API path]** → mitigation：每次新增 path 同步檢查 reserved list；可加一條測試「現有 path 第一層皆屬 RESERVED_SLUGS」，CI 失敗反向提醒。

## Migration Plan

無使用者資料 migration：既有 Orgs 沒有 `slug` 欄位 = serde Option 為 None，行為與 pre-change 一致。

部署順序：
1. 部署 API 新版本（含 ensure_indexes 加 slug sparse unique + history TTL）
2. 部署 admin-web 新版本（UI 才會出現 slug 區塊）
3. 監測：register endpoint 錯誤率、set-slug 用量

回滾策略：
- API 回滾：slug 欄位保留在 doc 裡無害（serde 容忍未知欄位 ↔ 前提：deserialize 設定不嚴格；本專案使用 mongodb crate 容忍）。回滾後新建 Org 自然不寫 slug。
- 已建立的 slug 與 history 在 DB 留著，下次重新部署可繼續用。
- TTL index 不會因為 API 回滾而消失。

## Open Questions

1. **整合測試裡 testcontainers 起的 mongo 是否預設 replica set？** Mongo 7 single-node 預設不是 replica set，transaction 不能跑。本 change 需要 replica set；若 testcontainers 預設不是，要在 `tests/common.rs` 加 `--replSet` 起初始化。先列為 open，apply 階段確認。

2. **TTL index 與既有 `dashboard_sessions.expires_at` TTL 是否要統一 helper？** 不影響 spec，但 `ensure_indexes` 函式可能可以重構出一個小 helper。先不動，apply 時看程式碼結構決定。

3. **DELETE 之後 admin 多久能再 SET？** 邏輯上：DELETE 把舊 slug 進 grace 並更新 `slug_changed_at`，下一次 SET 受同一個 30 天 cool-down 約束（決議：DELETE 計入 rate limit）。但 admin 可能誤觸 DELETE 又馬上想設回，UX 上要不要給「30 天內可一次 undo」？決議：本期不做，UI 顯示倒數即可，繞過 → 走人工。
