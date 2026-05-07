# CHANGELOG

班到 (Bandao) 跨服務 release notes。本檔案統一紀錄 `app/`、`admin-web/`、`api/` 三個元件的對外可見變動，依元件分節，依時間倒序。

格式參考 [Keep a Changelog](https://keepachangelog.com/zh-TW/)。版號約定：

- `app/` 跟著 `app/pubspec.yaml#version`（`<name>+<build>`）。
- `admin-web/` 跟著 `admin-web/package.json#version`。
- `api/` 跟著 `api/Cargo.toml#package.version`。

每次 cut release 時，把要寫進 store / GitHub Release 的 release notes 從這裡 paste 到對應欄位。

## App

### [0.3.0+3] - 2026-05-07

首次公開上架到 App Store + Google Play。

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
