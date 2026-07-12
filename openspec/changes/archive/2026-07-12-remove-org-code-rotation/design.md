## Context

`org.code` 的產生（`auth::org_code::generate()`）跟「換發新代碼」（rotate）是兩件事，目前綁在同一個模組裡但邏輯獨立：
- 產生：Org 建立時呼叫一次，之後不再變動（除非 rotate）。
- Rotate：`POST /orgs/me/code/rotate`，admin-only，換一組新的隨機值、舊值立即失效。

這次只拿掉 rotate 這個動作，`org_code` 模組本身（`generate()` / `is_well_formed()`）繼續被 Org 建立流程與 join/App 登入的 code 解析邏輯使用，不受影響。

## Goals / Non-Goals

**Goals:**
- 移除 rotate 這個 API 端點與對應 UI，不留殘骸（沒有 deprecated 但保留的路由、沒有殘留的前端 state）。
- `org.code` 的產生與消費路徑完全不動。

**Non-Goals:**
- 不處理「登入失敗鎖定」這個長期緩解機制的實作——已記在 ROADMAP.md，是獨立的未來 change。
- 不動 vanity slug 的 rotate/grace-period 機制——那是完全獨立的系統，這次不碰。

## Decisions

### D1. `org_code` 模組保留，只刪 rotate 這條使用路徑
不刪 `auth::org_code::generate()` / `is_well_formed()`，只刪 `handlers/orgs.rs::rotate_code` 這個呼叫它的 handler。
- **為何**：`generate()` 在 Org 建立時仍是必要的；`is_well_formed()` 是 join 流程判斷輸入是 code-shaped 還是 slug-shaped 的關鍵字符集檢查，這次沒有理由跟著動。

### D2. 套用順序：先於 `add-admin-web-sidemenu`
這個 change 動到的 `pages/index.vue`「管理員工具」區塊，跟 sidemenu change 要整段重寫的區塊完全重疊。
- **為何要先套用這個**：sidemenu 那次會把整個區塊搬進新的 layout component，如果這個 change 後套用，rotate 相關的 markup 可能已經被搬走或改名，這裡列出的 task 會直接對不上程式碼。反過來（先移除 rotate、再讓 sidemenu 從一個已經沒有 rotate 的乾淨起點開始重寫）不會有這個問題。

## Risks / Trade-offs

- **[Risk] 拿掉 rotate 之後，code 外流沒有任何補救手段** → 這是刻意接受的取捨，詳細推理見 proposal.md 的「Why」——由未來的登入失敗鎖定機制承接這個防線，不是靠 rotate。
