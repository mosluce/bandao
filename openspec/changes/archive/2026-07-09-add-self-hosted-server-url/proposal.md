## Why

本 repo 是 public 的：任何人都能自己把 `api/`（+ Mongo）部署起來，跑一套自己的班到後端。但目前上架的官方 App（Play Production / iOS Unlisted）被硬綁在 compile-time 的 API base URL，**自建者無從把 App 指向自己的 server** —— 他們不會、也不該為此自行上架一顆 App。

其實「執行期覆寫 API base URL」的機制早已存在（`ApiBaseUrlResolver` + secure-storage override + 一個設定畫面），只是整條路徑被刻意標記為 **debug-only** 並在 release build 被 tree-shake 掉。本次把這條既有路徑**從開發後門升級為 release 正式功能**，讓自建者能在登入前指定自己的 server URL；官方 hosted 仍是預設，一般使用者零感知。

## What Changes

- **release build 開放 API base URL 覆寫**：移除 `DevOverrides` 在 `kReleaseMode` 的 early-return 短路；release 也可讀寫 per-device override，疊加在 compile-time default 之上（override 有值優先，否則用預設）。
- **驗證規則分岔（release 嚴格 / debug 寬鬆）**：release build 只接受 `https://` + 具備 host 的 URL；debug build 維持寬鬆（允許 `http://`、`localhost`、內網 IP，供開發）。https-only 同時讓 iOS ATS / Android cleartext 無需開任何傳輸例外。
- **設定入口去 gate、升級為正式畫面**：登入頁新增一個低調但**在所有 build 皆可見**的「伺服器設定」入口（取代目前僅 debug 的 logo 5-連點）；設定畫面本身移除 release 早退，成為自建者可用的正式頁。
- **明示當前連線對象**：登入頁顯示目前連到「官方預設」或「自訂 <host>」，降低誤連／被導向惡意 server 的風險。
- **切換 server 時清理狀態**：變更 base URL 等同換一台後端，既有 bearer token／session 對新 server 無意義；儲存新 URL 時清除已存的登入狀態，使用者於新 server 重新登入。
- **範圍僅限 App（Flutter）**：`api/` 與 admin-web 完全不動 —— App 原生請求無 CORS 問題，自建者只要把 `api/` 跑在有 TLS 的公網網域即可被指向。

## Capabilities

### Modified Capabilities
- `app-shell`: API base URL 解析納入 release override 與分 build 的驗證規則；移除「release 排除 override 路徑」的舊要求；登入頁新增伺服器設定入口與當前連線顯示；伺服器設定畫面成為正式（非 debug-only）頁面；切換 server 清除既有登入狀態。

## Impact

- **app/（Flutter）**：
  - `core/storage/dev_overrides.dart`：移除三個 `kReleaseMode` 短路；建議連同類別／storage key 由 `dev.*` 命名正名為中性名稱（如 `server_url_override`）。
  - `core/storage/api_base_url.dart`：`ApiBaseUrlResolver` 疊加邏輯不變；註解更新（不再是 debug-only）。
  - 新增 URL validator：release 強制 https + host，debug 寬鬆；供設定畫面與（若需要）背景同步共用。
  - `features/auth/presentation/dev_server_config_screen.dart`：移除 `_seed()` 的 `kReleaseMode` 早退；改用正式標題與文案；Crashlytics 自我測試按鈕維持 `kDebugMode`。
  - `features/auth/presentation/login_screen.dart`：`_onLogoTapped` 入口去 `kDebugMode` gate、改為可見連結；新增「目前連線：官方 / 自訂 <host>」顯示；儲存新 URL 後清除登入狀態。
  - `core/env/env.dart`：`compileTimeDefault()` 維持為官方預設基準，不變。
  - `features/checkin/data/background_sync.dart`：已直接讀 override，放開寫入後自動跟隨新 server；需確認換 server 時的 token/URL 一致性。
  - l10n：新增伺服器設定入口／當前連線／驗證錯誤等字串。
- **api/、admin-web**：無變更。
- **已知取捨**：僅支援 https 公網 server（依 explore 定調）；不支援內網 http／純 IP 自建情境，亦不做 server 書籤或多 server 切換（單一 override）。
