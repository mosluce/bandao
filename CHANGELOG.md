# CHANGELOG

班到 (Bandao) 跨服務 release notes。本檔案統一紀錄 `app/`、`admin-web/`、`api/` 三個元件的對外可見變動，依元件分節，依時間倒序。

格式參考 [Keep a Changelog](https://keepachangelog.com/zh-TW/)。版號約定：

- `app/` 跟著 `app/pubspec.yaml#version`（`<name>+<build>`）。
- `admin-web/` 跟著 `admin-web/package.json#version`。
- `api/` 跟著 `api/Cargo.toml#package.version`。

每次 cut release 時，把要寫進 store / GitHub Release 的 release notes 從這裡 paste 到對應欄位。

## App

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
