## Why

目前 `POST /auth/register?mode=join` 與 `POST /me/memberships` 兩條 join 路徑都直接寫入 `dashboard_memberships` 把申請者一刀升為 `role=member`，admin 沒有介入點。invite link（`/register?code=...`）一旦 leak（slack 訊息轉發、員工離職前外洩、social engineering），任何人都能立刻看到 Org 內部資料 — AppUser 列表、打卡狀態、軌跡、xlsx 匯出。配合 vanity slug 這種可猜的「公開 URL」風險特別大。

改成「申請 → admin 審批 → 成為 member」兩段流程把這個攻擊面封掉：申請者建好 user identity 但拿到 zero-org session，admin 看到 dashboard badge 後決定 approve / reject。reject 可附理由。

## What Changes

### API（modified `dashboard-auth` capability）

- `POST /auth/register?mode=join` 行為改變：成功時建立 `dashboard_user` identity + `join_requests` row（status=pending），**不**建 `dashboard_memberships`。session 仍發但 `current_org_id=null`（zero-org state），`/me` 顯示沒有 active org。
- `POST /me/memberships` 行為改變：建立 `join_requests` row（status=pending），不建 membership。response 不切換 `current_org_id`。
- 既有 `enforce_join_cooldown`（`removed_memberships` 7-day cooldown）的觸發點從「create membership」搬到「create join_request」。被 kick / 自離後想再加入仍要等 cooldown 過。
- 既有 `ALREADY_MEMBER` 仍適用：申請時若已是 active member 直接拒絕。
- 新 error code `JOIN_REQUEST_PENDING`：申請者對同 Org 已有 pending 申請時不能重複申請。

### API（new `org-join-requests` capability）

- `POST /me/join-requests` — authenticated user 提申請（`POST /me/memberships` 內部呼叫此實作）。Body: `{ org_code, application_message? }`（message ≤ 500 字）。
- `DELETE /me/join-requests/:id` — 申請者主動取消自己的 pending 申請。
- `GET /me/join-requests` — 申請者列自己的所有申請（pending / approved / rejected / cancelled）。
- `GET /orgs/me/join-requests?status=pending` — admin 列當前 Org 的申請。
- `POST /orgs/me/join-requests/:id/approve` — admin 批准。原子性：把 pending request 變成 approved + insert membership row（role=member）。
- `POST /orgs/me/join-requests/:id/reject` — admin 拒絕。Body: `{ rejection_reason? }`（≤ 500 字）。
- 不做 reject cooldown（YAGNI），不做自動過期。

### Schema

- 新 collection `join_requests`：`{ _id, user_id, org_id, status: 'pending'|'approved'|'rejected'|'cancelled', application_message?, rejection_reason?, requested_at, decided_at?, decided_by? }`
- Unique index on `(org_id, user_id, status)` 限定在 `status=pending` 的部分索引（mongo partial index），確保同人對同 Org 同時只能有一筆 pending。

### admin-web

- 新頁面 `/admin/join-requests`（admin-only）：列當前 Org 的 pending 申請、approve / reject 按鈕、reject 可帶 reason
- `/`（home）admin 區塊加 badge：「N 筆待審核申請」
- Register 表單在送出後若是 `mode=join`，提示「已收到申請，等待 X Org 管理員審核」
- Zero-org state（`/no-org`）若 user 有 pending requests，顯示申請清單 + 取消按鈕；rejected 申請顯示拒絕理由

### Tests

- API integration: register/me-memberships 路徑的新行為、approve flow、reject flow、cancel flow、duplicate pending 拒絕、cooldown 在 request 創建時觸發、cross-org guard、role guard
- admin-web vitest: join-requests 頁面 approve/reject 按鈕、badge 數字反映 pending count、申請者 zero-org 視圖渲染 pending list

## Capabilities

### New Capabilities

- `org-join-requests`：Org 加入申請與審核流程的完整領域 — 申請者 / 取消 / 列舉、admin 審核 / 批准 / 拒絕、與 membership 的 atomic transition、與 cooldown / ALREADY_MEMBER 的互動。

### Modified Capabilities

- `dashboard-auth`：`POST /auth/register?mode=join` 與 `POST /me/memberships` 兩條既有 join 路徑改成創建 `join_request` 而非直接 membership；`removed_memberships` cooldown 觸發點改在 request 創建層。

## Impact

- **`api/`**：新 `JoinRequest` domain + `join_requests` repo；改 `register` / `join_membership` handler；新 7 個 endpoint；現有 `enforce_join_cooldown` 搬位置；新 error code `JOIN_REQUEST_PENDING`。
- **`admin-web/`**：新頁面 + composable + register/zero-org page UX 改、home badge。
- **`app/`**：完全不動（AppUser 跟 dashboard membership 無關）。
- **MongoDB schema**：加 `join_requests` collection + partial unique index。`dashboard_memberships` schema 不變。
- **既有 active members**：完全不受影響。新行為只觸及「下一次有人想加入的人」。
- **既有 invite link**：URL `/register?code=...` 不變，但點下去送出後變成「等待審核」。對 admin 是友善的（看到 badge），對申請者要習慣多一步。
- **依賴關係**：無外部依賴（不需要 email provider）；future email 通知可在 `add-email-provider` 上線後增量加。
