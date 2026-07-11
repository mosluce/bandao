# CHANGELOG

班到 (Bandao) 跨服務 release notes。本檔案統一紀錄 `app/`、`admin-web/`、`api/` 三個元件的對外可見變動，依元件分節，依時間倒序。

格式參考 [Keep a Changelog](https://keepachangelog.com/zh-TW/)。版號約定：

- `app/` 跟著 `app/pubspec.yaml#version`（`<name>+<build>`）。
- `admin-web/` 跟著 `admin-web/package.json#version`。
- `api/` 跟著 `api/Cargo.toml#package.version`。

每次 cut release 時，把要寫進 store / GitHub Release 的 release notes 從這裡 paste 到對應欄位。

## App

### [0.4.1+11] - 2026-07-11

#### Maintenance
- Flutter 升級到 3.44.6-stable，`workmanager` 從 `<0.8.0` bump 到
  `>=0.9.0 <0.10.0`（連帶處理 Android AGP/Gradle/Kotlin/NDK/JDK、iOS
  deployment target 13.0→14.0 的原生層 breaking change）。純 toolchain
  維護，無使用者可見的功能變化。詳見 OpenSpec change `update-flutter-latest`。

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
