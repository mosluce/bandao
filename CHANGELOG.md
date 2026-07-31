# CHANGELOG

班到 (Bandao) 跨服務 release notes。本檔案統一紀錄 `app/`、`admin-web/`、`api/` 三個元件的對外可見變動，依元件分節，依時間倒序。

格式參考 [Keep a Changelog](https://keepachangelog.com/zh-TW/)。版號約定：

- `app/` 跟著 `app/pubspec.yaml#version`（`<name>+<build>`）。
- `admin-web/` 跟著 `admin-web/package.json#version`。
- `api/` 跟著 `api/Cargo.toml#package.version`。

每次 cut release 時，把要寫進 store / GitHub Release 的 release notes 從這裡 paste 到對應欄位。

## App

### [0.4.3+6] - 2026-07-31

#### Changed
- 打卡時新增「地點」欄位，上班／下班／轉出／轉入都要填寫，內容會一起記錄在打卡紀錄裡。
  填過的地點會列在欄位下方，之後點一下就能帶入，不用重打。地點沒填時打卡按鈕不會啟用。
- 打卡紀錄的位置改為顯示「地點, 地址」。只有其中一項時就顯示該項，兩項都沒有時維持顯示座標。
- 剛填過的地點會立刻出現在建議清單裡，不需要重開 App，離線時也一樣。

#### Fixed
- 修復同一支手機換帳號登入後，地點建議會顯示**前一位使用者**填過的地點。建議清單現在依帳號分開儲存，
  登出後也不會殘留。

### [0.4.3+3] - 2026-07-31

#### Fixed
- 修復「我的軌跡」在一整天都待在同一處時，地圖只顯示空白背景（路線和標記仍在，但看不到街道）。
  地圖會自動縮放到剛好框住當天的所有位置，範圍太小時就會縮過頭。

### [0.4.3+2] - 2026-07-31

#### Fixed
- 修復使用外部帳號驗證的組織**完全無法登入**：輸入正確帳密後畫面一直轉圈、沒有錯誤訊息也無法繼續。
  這類帳號沒有內部使用者名稱，App 解析登入回應時因此失敗。首頁的識別碼欄位現在會顯示外部系統
  的帳號（原本顯示使用者名稱的位置）。
- 修復登入若因非預期原因失敗時，畫面會停在無限轉圈、看不到任何錯誤也無法重試。現在會回到登入
  畫面並顯示錯誤訊息。
- 修復剛按下上班、還沒產生任何定位點時，開啟「我的軌跡」會導致畫面異常。

### [0.4.3+1] - 2026-07-31

#### Changed
- 「我的軌跡」的路線現在會經過上班、下班、轉出、轉入的打卡點。先前路線只由定位
  回報點連成，而定位追蹤是在伺服器確認上班後才啟動、按下下班的當下就停止，所以
  打卡標記永遠落在路線之外——路線的頭尾各短少一段。
- 「走動距離」因此涵蓋了上班點到第一個定位點、最後一個定位點到下班點這兩段；
  「在班時長」改為上班到下班的實際跨距，不再是首尾定位點的間距。
- 「位置點」現在計算實際畫出來的點數（含打卡點），同一天的數字會比先前多 2–4。
- 修正：歷史中含有從舊系統匯入的打卡紀錄時，事件列表會解析失敗導致頁面無法載入。

### [0.4.2+12] - 2026-07-12

#### Fixed
- Android：修復 Keystore 加密金鑰失效（OS 升級、還原到不同裝置、螢幕鎖密碼被清除等
  情境）時，讀取本機加密設定會丟未攔截的 `PlatformException`（`BadPaddingException`），
  導致 app 卡死在啟動流程無法開啟。現在遇到解密失敗會清掉損壞的項目、視為未設定，
  app 可以正常啟動。

### [0.4.1+11] - 2026-07-11

#### Added
- 「我的工作日記」軌跡地圖新增依時段上色 — 路徑從清晨到深夜以暖到冷的漸層呈現，
  一眼看出當天路線的時間分布；地圖起點錨定在上班打卡時間，即使當下還沒有第一筆
  定位 ping 也會先顯示起點。

#### Maintenance
- Flutter 升級到 3.44.6-stable，`workmanager` 從 `<0.8.0` bump 到
  `>=0.9.0 <0.10.0`（連帶處理 Android AGP/Gradle/Kotlin/NDK/JDK、iOS
  deployment target 13.0→14.0 的原生層 breaking change）。純 toolchain 維護，
  使用者無感。詳見 OpenSpec change `update-flutter-latest`。

### [0.4.0+10] - 2026-07-10

#### Added
- 支援自建 API server：登入頁新增「伺服器設定」入口（所有 build 皆可見），可把 app
  指向自己部署的 `api/` 後端。登入頁顯示目前連線對象（官方預設 / 自訂 host），切換
  server 會清除舊 session 要求重新登入。
- release build 只接受 `https://` + 具 host 的伺服器網址（免除 iOS ATS 例外）；debug
  build 維持寬鬆（`http`/`localhost`/內網 IP）供本機開發。

#### Why
- 本 repo 為 public，讓想自建後端的使用者能用官方上架 app 指向自己的 server，無需自行
  上架。詳見 OpenSpec change `add-self-hosted-server-url`。

### [0.3.1+8] - 2026-05-21

#### Added
- 新增「我的工作日記」(`/trajectory`) — 在 app 內回顧自己今天的工作路線、走動距離與在班時長。
  支援今天 + 過去 7 天任一日切換，map 走 CARTO Positron tile，距離透過 `latlong2` 取
  geodesic 累加。
- 首頁加上「我的今天」摘要卡 — 顯示當日距離與在班時長，點擊即跳轉到 `/trajectory`；
  上班中或當日有 ping 時顯示，否則隱藏。
- 底部導覽列改為三分頁的 `StatefulShellRoute.indexedStack`：首頁、歷史、我的軌跡，
  每個分頁保留自己的 state，切換時不重 build。
- API：`GET /app/checkin/me/locations` — AppUser bearer auth、token-derived identity，
  ordering 與 range 規則與既有 admin `/checkin/users/:id/locations` 完全一致。
  此 endpoint 不受 Org `location_tracking_enabled` toggle 拘束（toggle 僅 gate POST）。

#### Changed
- `NSLocationWhenInUseUsageDescription` 與打卡前的同意對話框文案改為「我的工作日記」
  優先 — 先說明使用者本人能在 app 內回顧，再提到組織管理員可查閱。
- App Store / Play Store 描述 + promotional text reframe，「我的工作日記」放第一條
  特色 bullet。
- 移除首頁底部的「事件歷史」TextButton — 已由底部導覽列取代。

#### Why
- iOS App Review submission `2f88a54d-2b9a-4069-b5fa-88e2ed770187` (0.3.0+7) 被 2.5.4
  退件，理由是 `UIBackgroundModes: location` 只服務 employer-side tracking 不符合 Apple
  政策。新增的「我的工作日記」讓 AppUser 自己成為背景位置資料的主要受益者。完整 review
  reply 留存於 `app/store_metadata/ios/app_review_replies/2.5.4-2026-05-15.md`。

### [0.3.0+7] - 2026-05-09

#### Fixed
- iOS：背景中移動觸發背景同步、且裝置處於鎖屏狀態時，會因 Keychain
  讀不到 bearer token 導致 `POST /app/checkin/locations` 沒帶
  `Authorization` header，server 回 401，processor 走 `_onAuthExpired`
  把人靜默登出，使用者拿出手機後跳到 `/login`。修法：
  - `SecureStorage` 在啟動時讀一次 token 進記憶體，hot path 讀寫不再每次
    打 Keychain；同時把 iOS Keychain accessibility 改成 `first_unlock`，
    補 cold-launch-while-locked 邊界。
  - 行為對使用者透明，session 一旦建立即可橫跨整個鎖屏背景時段。

### [0.3.0+4] - 2026-05-08

首次 TestFlight 可用版本。`+3` 的 cut 因為 build 指令缺
`--dart-define=API_BASE_URL=...`，內建 URL 跑回 `Env.compileTimeDefault`
的 `localhost:9090`，TestFlight 使用者完全連不到後端 — 已被
`+4` 重 cut 取代。`scripts/release_ios.sh` 起會自動帶 dart-define。

#### Added
- 上班、下班、轉場（轉出 / 轉入）三種打卡事件
- 工作期間軌跡記錄 — admin 端 toggle 控制，使用者第一次上班前需同意
- 多組織支援，一個帳號可加入多個 Org，隨時切換
- 多裝置 session — iOS / iPad / Android 同時登入
- 離線打卡 queue — 沒網路時暫存到 drift SQLite，恢復連線後自動同步
- 事件歷史頁，含 pull-to-refresh

#### Native / 隱私
- iOS 位置權限走 When-In-Use（不要求 Always）；上班期間 OS 顯示藍色提示
- Android 透過 Foreground Service + sticky notification 實作上班期間追蹤；不申請 ACCESS_BACKGROUND_LOCATION
- Firebase Crashlytics 接 client crash report，**不**關連使用者身份
- iOS + iPad 同 binary（`TARGETED_DEVICE_FAMILY = 1,2`）

## admin-web

（首次發 release notes 從此版起。歷史變動請見 git log。）

## api

（首次發 release notes 從此版起。歷史變動請見 git log。）
