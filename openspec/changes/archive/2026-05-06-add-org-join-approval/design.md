## Context

兩條 join 路徑現況：

```
/auth/register?mode=join                      /me/memberships
─────────────────────────                     ────────────────
unauthenticated visitor                       authenticated user
       │                                              │
       ▼                                              ▼
  validate_email + password                      resolve org_code
       │                                              │
       ▼                                              ▼
  resolve org_code                            enforce_join_cooldown
       │                                              │
       ▼                                              ▼
  enforce_join_cooldown                       memberships.create(member)
       │                                              │
       ▼                                              ▼
  create user                                  update session.current_org_id
       │
       ▼
  memberships.create(member)
       │
       ▼
  issue_session(current_org=org)
```

兩者最後都直接打 `dashboard_memberships.create`。改完之後變成中間插一個 `join_requests` 表：

```
join 路徑                                     admin 審核路徑
─────────                                     ──────────────
create user (if needed)                       GET /orgs/me/join-requests
   │                                                │
   ▼                                                ▼
enforce_join_cooldown                          POST .../approve  → membership.create
   │                                          POST .../reject   → status=rejected
   ▼
join_requests.create(pending)
   │
   ▼
issue_session(current_org=null)
```

關鍵 invariant：`join_request.status=pending` 對同一 `(org_id, user_id)` 只能存在一筆（partial unique index）。Approve = state transition + atomic membership insert。Reject / Cancel = 純 status 變更。

## Goals / Non-Goals

**Goals:**
- Admin 對任何「想加入 Org 的人」都有 approve / reject 控制權
- 申請者使用體驗順：申請後立刻有 session，能看到自己的 pending 狀態，能取消
- 既有 active members 完全不受影響
- 既有 cooldown / ALREADY_MEMBER 規則整合進新流程
- 沒有 silent breakage：spec 明確標出 register/join 行為改變

**Non-Goals:**
- 不做 email 通知（依賴 email provider，獨立 ROADMAP 條目）
- 不做申請過期 / TTL（admin 手動處理）
- 不做 reject cooldown（YAGNI；admin 看到同一 email spam 申請可手動 ignore）
- 不做 Org-level toggle 控制是否啟用審核（一律強制）
- 不重構 `dashboard_memberships` schema（仍是 `(user_id, org_id, role, joined_at)` 結構）
- 不影響 owner transfer / member removal / cooldown / slug 既有行為
- 不改 mobile app（AppUser 體系跟 dashboard_memberships 無關）
- 不對 AppUser register 流程做任何改動

## Decisions

### D1：分離 collection `join_requests`，不在 `dashboard_memberships` 加 status 欄位

**Why**：
- pending request 跟 active membership 行為差太大：pending 不能 list_by_user 認證、不參與 session.current_org、有 reject reason、可 cancel
- 既有所有 `memberships.list_by_user` / `memberships.find_by_user_org` 等 query 不用加 `status=active` 濾鏡
- approve 是 state transition + 跨 collection write，這比「同表 status flip」表達意圖更清楚
- 從 read model 看：「我屬於哪些 Org」只需查 `dashboard_memberships`；「我有哪些 pending 申請」只需查 `join_requests`。職責清楚不會互相干擾。

**Alternative**: 共用 `dashboard_memberships` 加 status 欄位。Pro: 一個 entity 一個表，conceptually 連續。Con: 所有既有 query 要加 filter，session / role lookup 要小心避免「 pending 也算數」的 bug。

### D2：Schema

```rust
struct JoinRequest {
    _id: ObjectId,
    user_id: ObjectId,
    org_id: ObjectId,
    status: JoinRequestStatus,    // pending | approved | rejected | cancelled
    application_message: Option<String>,  // ≤ 500 字
    rejection_reason: Option<String>,     // ≤ 500 字，admin reject 時填
    requested_at: DateTime,
    decided_at: Option<DateTime>,         // approve / reject / cancel 都填
    decided_by: Option<ObjectId>,         // approve / reject 寫 admin user_id；cancel 留 null（用 user_id == decided_by 沒太多意義）
}

enum JoinRequestStatus { Pending, Approved, Rejected, Cancelled }
```

Indexes:
- `(org_id, user_id, status)` partial unique on `status="pending"` — 防同人對同 Org 多筆 pending
- `(org_id, status)` for admin 列 pending（最常 query）
- `(user_id, status)` for `GET /me/join-requests`

Approved 紀錄不刪 — 留 audit trail。Rejected / Cancelled 也不刪。如果未來累積太多再說（partial unique 已經保護「pending 不重複」這個重點）。

### D3：Approve 是原子操作

```
admin POST /orgs/me/join-requests/:id/approve
  │
  ▼ (mongodb transaction OR session-level write)
1. read join_request, verify status=pending && org_id matches admin's org
2. cooldown re-check: if removed_memberships marker still active, fail with EMAIL_IN_COOLDOWN (rare race but possible)
3. ALREADY_MEMBER re-check: if dashboard_memberships row already exists for (user_id, org_id), fail
4. update join_request → { status: approved, decided_at, decided_by }
5. insert dashboard_memberships row { role: member, joined_at }
6. response: 204 No Content
```

**為什麼用 transaction**：approve 涉及兩個 collection write，要一致。如果用兩個獨立 update 而中間掉，可能出現「 request 變成 approved 但沒對應 membership」的不一致。Mongo replica set 預設支援 transactions，testcontainers MongoDB 7 也支援。

如果 transaction 不可用（單機 mongo），回退方案：先寫 membership（idempotent — duplicate index 自然防護），後 update request。中間掉 → request 還是 pending，admin 重點一次按 approve 補完。可以接受。

### D4：Cancel 與 Reject 的差別

- **Cancel**：申請者主動取消自己的 pending request。`DELETE /me/join-requests/:id`，要求 caller 是 owner 該 request。把 status 改 `cancelled`、寫 `decided_at`。
- **Reject**：admin 拒絕。`POST /orgs/me/join-requests/:id/reject`，body `{ rejection_reason? }`。把 status 改 `rejected`、寫 `decided_at`、`decided_by`、`rejection_reason`。

兩者都不刪 row，只翻 status。差別只在誰能做、要不要 reason。

### D5：Cooldown 規則整合

既有 `enforce_join_cooldown` 在「create membership」當下檢查 `removed_memberships` 表。改了之後：
- **First check**：`POST /me/join-requests` (and register mode=join 內部) 創建 request 之前 enforce cooldown — 早 fail，避免 admin 看到「明顯不可批准」的申請
- **Second check (defense in depth)**：admin approve 時再 enforce 一次 — 防止「申請當下沒 cooldown，等批准時 cooldown 還在生效」（雖然 cooldown 是固定 7 天、申請者 spam 申請也不會延長 cooldown 期，這個 race 在實務上機率極低，但 spec 上為清楚還是多檢一次）

### D6：register mode=join 行為改變

```
舊：visitor → register → user + membership(member) + session(current_org=org)
新：visitor → register → user + join_request(pending) + session(current_org=null)
```

response shape 仍然是 `AuthResponse`，差別在 `current_org=null`、`memberships=[]`。前端 register form 完成後檢查 `current_org` 為空、call `GET /me/join-requests` 拿 pending list 顯示「等待審核」訊息。

### D7：admin-web 路由 `/admin/join-requests` vs `/members?tab=requests`

兩個選擇：
- (a) **新獨立頁** `/admin/join-requests` — 簡單清楚，badge 直接 link 過去
- (b) 整合到 `/members` — tab 切換

我選 (a)。理由：審核操作 + member CRUD 是兩種場景（一個處理新人、一個管現有人），合併會有畫面複雜度。獨立頁能放更詳細的申請者 metadata + reason 文字框。

### D8：Badge 數字 caching

`/` admin 區塊或 nav bar 顯示「N 筆待審核」— N 是 `count(join_requests where org_id=current_org && status='pending')`。

實作選項：
- (a) 每次 `useAuth.refresh()` 順便 fetch count — 強耦合
- (b) 獨立 composable `usePendingJoinRequestsCount`，間隔 1 分鐘 polling
- (c) 跟 `/me` response 一起回（embed in AuthResponse）

我選 (b) 一致性最低、最易擴展。用 30 秒 polling 跟既有 `/checkin` 看板一致。

### D9：申請者 Zero-org state UX

申請者 register 完成後 `/no-org` 頁面（既有路由）顯示：
- 「您已申請加入 Acme，請等待管理員審核。」
- pending 申請列表（看 D8）
- 「取消申請」按鈕（call DELETE）
- 既有的「建立新 Org」與「加入其他 Org」按鈕仍在 — 申請被拒絕後可以重試 / 加別的 Org

如果同時有 active membership 跟 pending request（user 已是 Org A 的 member、申請加入 Org B），主視窗依舊在 Org A，pending list 用 `GET /me/join-requests` 取得在某個地方顯示（暫時放 home page footer 或單獨 `/me/join-requests` 頁面）。

### D10：register `mode=join` 失敗時的 user identity 怎麼辦

新流程：register mode=join 先建 user identity 然後建 join_request。如果 join_request 創建失敗（比如 cooldown）— user 已經建好但沒 membership 也沒 request。

選擇：
- (a) Rollback user creation（既有路徑遇到 membership 失敗就 cleanup user 的模式）
- (b) Keep user, return error — 反正 user 之後仍可登入嘗試 `/me/memberships`

我選 (a) — 跟既有 register-failure 的 cleanup 行為一致。對使用者而言，register 失敗就應該是「沒有 user 也沒有 request」，下次重試完整來過。

## Risks / Trade-offs

- **既有 invite link 的 admin 沒準備**：突然 invite link 不再「點下去就直接加入」，可能 admin 不知道有 pending 在 dashboard 等待。**Mitigation**：home badge 醒目（紅點 + 數字），admin 第一次進 dashboard 會看到。發布前可在 changelog / 內部通知里講清楚。
- **小 Org admin 不常登入**：申請者卡很久。**Mitigation**：Q4 已決定不過期；email 通知在後續 ROADMAP 條目處理；admin 教育是 product / ops 課題，spec 不解決。
- **同人多次申請被拒絕**：Q3 不做 cooldown，admin 可能被 spam。**Mitigation**：admin 可逕自 reject 不看；如果真的成為痛點再 retro 加 cooldown。第一版 YAGNI。
- **Approve 跨 collection write 的一致性**：D3 已述。Production 必須跑 mongo replica set 才有 transaction；單機 mongo 走回退路徑（先 membership 後 request status flip），duplicate index 防 double approve。
- **既有 cooldown spec 文字會跨 capability**：cooldown 仍是 dashboard-auth 的 requirement，但觸發點改了 — modify 既有「Cooldown blocks rejoin during register mode=join」requirement 的描述。
- **AuthResponse 不變對 D8 的影響**：badge count 走獨立 endpoint（D8 b），AuthResponse shape 不動，前端 zero-org page 多一次 fetch。可接受。

## Migration Plan

純前進式變更：
1. API 部署：新 endpoints + 改 register/join 行為
2. admin-web 部署：新頁面、badge、register UX 訊息更新
3. 沒有 schema migration（新 collection 自動隨第一筆 write 創建；既有 `dashboard_memberships` 不變）

**Rollback**：
- API：revert register/join handler 改動，刪除新 endpoints
- admin-web：revert UI
- DB：`join_requests` collection 仍在但孤立，無害
- 任何 in-flight pending request 在 rollback 後變成「孤兒」 — 不會再被處理。可在 rollback 前先把所有 pending 通通 approve 或 reject 清空，避免使用者疑惑。
