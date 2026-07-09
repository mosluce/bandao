## Context

App 目前的 API base URL 解析分兩層（`app-shell` spec 現況）：

- `Env.compileTimeDefault()`：`--dart-define=API_BASE_URL` 有值優先，否則 Android → `http://10.0.2.2:9090`、其餘 → `http://localhost:9090`。
- `ApiBaseUrlResolver`：在上者之上疊一層 per-device override（存 secure storage `dev.api_base_url_override`）。但 `DevOverrides.read/write/clear` 在 `kReleaseMode` 直接 early-return（`kReleaseMode` 是 const，整段被 tree-shake），且設定畫面入口（logo 5-連點）與 `_seed()` 也都 gate 在 debug。**結論：release build 無法覆寫，官方上架 App 硬綁 compile-time default。**

repo 為 public，自建者會部署自己的 `api/` 後端，但用的是官方上架的那顆 binary。因此需求是把既有 override 路徑**在 release 開放**，並補上生產級 UX 與護欄。explore 已定調：只支援 **https 公網網域**、UX 走**獨立進階入口**、**release 嚴格 / debug 寬鬆**的驗證規則。

## Goals / Non-Goals

**Goals:**
- release build 能由使用者在登入前指定自訂 API base URL，疊加在 compile-time default 之上；官方 hosted 維持預設、一般使用者零感知。
- release 只接受 https + 具 host 的 URL；debug 維持寬鬆以利本機開發。
- 明示當前連線對象，降低誤連／惡意導向風險。
- 換 server 時清理與舊 server 綁定的登入狀態。
- 僅動 App；`api/`、admin-web 不變。

**Non-Goals:**
- 不支援內網 / 純 IP / 明碼 http 的自建情境（https 公網 only）。
- 不做 server 書籤、多 server 快速切換或自動探索（單一 override）。
- 不動 privacy URL 的 override（那條服務 admin-web，維持 debug-only）。
- 不改 `api/` 或 admin-web；不處理網路可達性（公網 https 自負）。
- 不改登入 API 契約（仍 `org_code + username + password`）。

## Decisions

### D1. 復用既有疊加層，只拆掉 release 短路
維持 `ApiBaseUrlResolver` 的「override 有值優先，否則 compile-time default」語意不變；本次僅移除 `DevOverrides` 三個方法的 `kReleaseMode` early-return，讓 release 也讀寫 secure-storage override。
- **為何**：機制已驗證可用（`apiClientProvider` 已在 override 變更時 invalidate 重建），改動面最小、風險最低。
- **替代**：把 server URL 做成登入表單第 4 欄——會汙染 99% 官方用戶的主流程，且與 explore 定調的「獨立進階入口」相悖。

### D2. 驗證規則分 build 岔開（release 嚴格 / debug 寬鬆）
新增一個共用 validator：
- **release**：`Uri.tryParse` 成功、`scheme == 'https'`、`hasAuthority`（有 host）。拒絕 http、無 scheme、純 path。
- **debug**：維持現況寬鬆（有 scheme + authority 即可），允許 `http://`、`localhost`、內網 IP。
- **為何**：release 面向真實使用者與公網，https 是機密與傳輸完整性的底線，同時免去 iOS ATS / Android cleartext 例外；debug 需要連本機 `http://localhost:9090`，不能一起收緊。
- **張力**：兩套規則會讓「在 debug 能存、release 存不進」的 URL 存在差異；validator 以 `kReleaseMode` 分支，並在錯誤文案明示「需 https」。

### D3. 命名正名（去 dev 化）
`DevOverrides`、storage key `dev.api_base_url_override`、`dev_server_config_screen.dart`、`AppRoutes.devServerConfig` 皆以 `dev` 命名，語意已不符（不再是 debug-only）。建議正名為中性名稱（如 `ServerUrlOverride` / `server.api_base_url` / `server_config_screen` / `serverConfig`）。
- **為何**：功能升級為正式，命名應反映其為使用者可見的伺服器設定，而非開發後門，避免後續維護者誤判其 gating。
- **替代**：保留 `dev.*` 命名只改行為——省 churn 但留下誤導性名稱；傾向一次正名，屬機械式改名、風險低。storage key 變更無痛（舊 key 僅 debug 曾用，不具生產資料）。

### D4. 入口：登入頁可見的「伺服器設定」連結
移除 `_onLogoTapped` 的 `kDebugMode` gate 與 logo 5-連點祕技，改為登入頁上一個低調但所有 build 皆可見的「伺服器設定」文字連結，點入既有設定畫面。
- **為何**：自建是正式功能，需可被自建者發現；5-連點祕技對正式功能是反模式。低調呈現（次要文字連結）避免干擾官方用戶。
- **替代**：首次啟動詢問「官方/自建」——對 99% 官方用戶是多餘摩擦，否決。

### D5. 明示當前連線對象
登入頁顯示目前 effective base URL 的來源：等於 compile-time default 顯示「官方預設」，否則顯示「自訂 <host>」（只顯示 host，不顯示完整 URL 以免雜訊）。設定畫面沿用現有「目前連線」欄位。
- **為何**：一旦 App 可被指向任意 server，使用者輸入的是自己 org 帳密；明示連線對象讓誤連或被社交工程導向惡意 server 時有機會察覺。
- **安全前提**：URL 由使用者自行輸入、限 https；非典型釣魚，但惡意 server 仍可取得當下輸入帳密並回傳假資料——以「明示 + https + 易恢復官方預設」為緩解，不做憑證釘選（first version）。

### D6. 切換 server 清除登入狀態
儲存一個與當前 effective URL 不同的 base URL 時，清除 `auth.bearer_token`（及相依的登入態），使用者需在新 server 重新登入；`auth.last_org_code` 可保留（純便利，不含機密）。
- **為何**：bearer token 由特定 server 簽發，指向新 server 後無意義；殘留舊 token 會造成 401 迴圈或狀態混淆。背景同步（`background_sync.dart`）直接讀 override URL，清 token 可避免它拿舊憑證打新 server。
- **替代**：保留 token 靠後端 401 自然登出——體驗差且易誤解為「自建 server 壞了」，否決。

### D7. https-only 免除傳輸層例外
因 release 僅允許 https，iOS `NSAppTransportSecurity` 與 Android `usesCleartextTraffic` 皆無需為自建情境開例外，維持現有安全預設。
- **為何**：把「只支援公網 https」的產品決定，轉成一條免維護的安全性質。

## Risks / Trade-offs

- **惡意 server 收割帳密／回傳假資料** → URL 由使用者自輸 + 限 https + 明示當前連線 + 易恢復官方預設；不做憑證釘選（列為後續）。
- **自建者只有內網 / http** → 本版不支援（explore 定調 https 公網 only）；文件明示。內網情境留待後續（可能需自簽憑證信任或 debug build）。
- **release/debug 驗證分岔造成困惑** → validator 錯誤文案明示「release 需 https」；design 記錄兩套規則來由。
- **命名正名的 churn** → 機械式改名，涵蓋 storage key／class／route／畫面檔名；storage key 變更無生產資料風險。
- **切換 server 未清乾淨的殘留狀態** → 集中在儲存流程清 token；測試涵蓋「換 URL → 舊 token 清除 → 需重登」。

## Migration Plan

1. 純 App 端行為變更，無資料模型 / 後端遷移。
2. 官方上架 App 更新後：預設行為完全不變（override 為空 → compile-time default → 官方 server）。
3. 自建者流程：更新 App → 登入頁「伺服器設定」→ 填 `https://自己的網域` → 儲存（清舊登入態）→ 於自建 server 登入。
4. Rollback：使用者於設定畫面「恢復官方預設」即清除 override，立即回官方 server；程式層若需整體回退，因預設路徑未變，風險低。
5. storage key 由 `dev.api_base_url_override` 改名後，舊 debug override 不再被讀取（無生產影響）。

## Open Questions

- 正名的確切命名（`ServerUrlOverride` / storage key / route 名）待定；傾向一次到位。
- 登入頁「當前連線」與設定入口的確切文案與擺放位置（次要連結 vs 齒輪 icon）待 UX 拍板。
- 是否於設定畫面加一顆「測試連線」按鈕（打自建 server 的健康檢查端點）以利自建者除錯？傾向後續再議，非本版必要。
- 憑證釘選 / 進一步的 server 信任機制是否需要——傾向 first version 不做，列為後續。
