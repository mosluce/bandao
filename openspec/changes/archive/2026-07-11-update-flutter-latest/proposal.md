## Why

`app/` 的 Flutter 版本目前散落在四個彼此不同步的地方：本機開發者的全域 asdf `~/.tool-versions`（因人而異，這次踩到的是 3.38.7-stable）、CI 的 `.github/workflows/app.yml`（釘死 3.29.3）、`pubspec.yaml` 的 `environment.flutter`（只有下限 `>=3.24.0`，沒有上限也沒有實際鎖定）、以及 `README.md` 的文字說明。`app/.tool-versions` 完全沒鎖 flutter（只鎖了 `ruby` 給 fastlane 用），代表本機開發版本其實不受 repo 控管，換一台機器 clone 下來版本可能完全不同。這個落差已經直接卡住一個依賴：`workmanager` 被上限鎖在 `<0.8.0`，理由寫在註解裡是「因為 CI/`.tool-versions` 還停在 3.29.3」——但 `.tool-versions` 裡其實從未真的鎖過 flutter，這條理由本身已經跟現況脫節。

現在要把 Flutter 升到目前可用的最新穩定版（3.44.6-stable），順手把版本鎖定這件事本身也做成一個可維護的契約，並解開被卡住的 `workmanager` 上限、一併 bump 其他直接依賴到相容範圍內的最新版本。

## What Changes

- 新增 `app/.tool-versions` 對 flutter 的版本鎖定（3.44.6-stable），本機與 CI 走同一個版本來源。
- `.github/workflows/app.yml` 的 `flutter-version` 從 `3.29.3` 升到 `3.44.6`。
- `pubspec.yaml`：
  - `environment.flutter` 下限同步調整。
  - `workmanager` 上限 `<0.8.0` 解除，允許 bump 到 0.8.0+/0.9.x。**BREAKING**（對這個專案自己的背景同步整合行為而言）：workmanager 0.8.0 起 enum 從 `snake_case` 改 `camelCase`、`inputData` 傳遞方式從 JSON 序列化改成原生 Map、改用聯邦式外掛架構，且要求 Android compile/target SDK 35、Java 17、iOS deployment target 13+。
  - 其餘直接依賴（`flutter_riverpod`、`go_router`、`dio`、`firebase_core`、`firebase_crashlytics`、`geolocator`、`flutter_map`、`drift`、`sqlite3_flutter_libs` 等）bump 到目前版本限制式下的最新相容版本。
- `README.md` 「Flutter `>= 3.24`」「CI 跑 3.29.3」的敘述同步更新。
- Android `build.gradle.kts` 視需要遷移到 Flutter 3.44 的 built-in Kotlin 支援；重新核對手動釘死的 `ndkVersion = "27.0.12077973"` 在新版 Flutter 預設 `abiFilters` 行為下是否仍足夠。
- 逐一驗證 SnackBar 相關程式碼（7 個檔案、12 處用法）在 Flutter 3.38+ 「有 action 的 SnackBar 不再自動消失」行為變更下是否仍符合預期。
- iOS `xcodebuild -exportArchive` 流程在新 Flutter engine 版本下重新走一次，確認 Xcode 26 export 沒有新地雷（呼應先前已知的 ipa export 問題）。

## Capabilities

### New Capabilities
- `flutter-toolchain`: 定義 `app/` 的 Flutter SDK 版本治理契約——本機（透過 `app/.tool-versions`）與 CI（`.github/workflows/app.yml`）必須鎖定同一個 flutter 版本，避免版本各自漂移；`pubspec.yaml` 的 `environment.flutter` 下限與該鎖定版本一致。

### Modified Capabilities
（無——`ci-pipeline`、`mobile-release` 既有的 spec 需求本身不隨這次版本 bump 改變，只是實作層面升級。）

## Impact

- **`app/.tool-versions`**：新增 flutter 版本鎖定。
- **`.github/workflows/app.yml`**：`flutter-version` 升級。
- **`app/pubspec.yaml`** / **`app/pubspec.lock`**：`environment.flutter`、`workmanager` 上限、其餘依賴版本全數調整；`pub get` 後 lockfile 會大幅變動。
- **`app/android/app/build.gradle.kts`**（與可能的 `settings.gradle.kts`）：Kotlin 套用方式、`ndkVersion` 視驗證結果調整。
- **`app/lib/features/**`**（7 個檔案，12 處 `SnackBar(` 用法）：僅在驗證發現行為不符預期時才需要改動，屬於驗證後的條件性修改。
- **`app/lib/**` 背景同步相關程式碼**：若 workmanager enum/inputData API 有變動，需要對應調整呼叫端。
- **iOS 打包流程**（不在版控內的操作步驟）：需要重新走一次 `xcodebuild -exportArchive` 驗證。
- **`README.md`**：Flutter 版本敘述文字更新。
- **不改**任何後端（`api/`）、`admin-web/` 的程式碼或 CI。
