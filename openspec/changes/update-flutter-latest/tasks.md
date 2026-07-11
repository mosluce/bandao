## 1. 版本鎖定契約（flutter-toolchain capability）

- [x] 1.1 在 `app/.tool-versions` 新增 `flutter 3.44.6-stable`
- [x] 1.2 本機 `cd app && asdf install` 確認能正確抓到 3.44.6-stable
- [x] 1.3 `.github/workflows/app.yml` 的 `flutter-version` 從 `3.29.3` 改成 `3.44.6`
- [x] 1.4 `app/pubspec.yaml` 的 `environment.flutter` 下限調整為 `>=3.44.0`（維持下限語意，不寫死上限）
- [x] 1.5 `README.md` 「Flutter `>= 3.24`」「CI 跑 3.29.3」的文字同步更新為新版本

## 2. 依賴 bump

- [x] 2.1 解除 `app/pubspec.yaml` 的 `workmanager: ">=0.6.0 <0.8.0"` 上限，改為允許最新相容版本（`>=0.9.0 <0.10.0`），並更新/移除已過時的版本理由註解
- [x] 2.2 `flutter pub outdated` 盤點直接依賴的目前版本 vs 可用最新版本（發現 firebase_core/crashlytics、riverpod、go_router、geolocator、flutter_map、app_settings、connectivity_plus、flutter_secure_storage 等有跨大版號的 latest，經與使用者確認後排除在本次範圍外）
- [x] 2.3 `flutter pub upgrade`（不加 `--major-versions`）把依賴 bump 到目前限制式內的最新相容版本，共更新 58 個依賴
- [x] 2.4 `flutter pub get` 產生新的 `pubspec.lock`，resolve 成功無衝突
- [x] 2.5 `dart run build_runner build`（drift codegen）確認新版 drift 下 codegen 正常（186 個輸出檔案；注意新版 build_runner 已忽略 `--delete-conflicting-outputs` 參數，行為改為內建預設）
- [x] 2.6 `flutter analyze` 全過，處理新版 SDK/依賴帶來的 lint/deprecation 警告（僅 1 個 workmanager `isInDebugMode` deprecation，見第 3 節）

## 3. workmanager breaking change 調整

- [x] 3.1 搜尋程式碼中所有 workmanager enum 用法（`NetworkType.connected`、`ExistingWorkPolicy.keep`）——這兩個值本來就是 camelCase，遷移後編譯通過不需改動
- [x] 3.2 搜尋 `inputData` 傳遞相關程式碼——本專案從未使用 `inputData` 參數，JSON→原生 Map 的傳遞方式變更不影響本專案
- [x] 3.3 確認 `android/app/build.gradle.kts` 的 compile/target SDK（透過 `flutter.compileSdkVersion`/`flutter.targetSdkVersion`）滿足 workmanager 新版要求的 SDK 35
- [x] 3.4 Java 版本從 `JavaVersion.VERSION_11` 調整為 `VERSION_17`（AGP 8.11 與 workmanager 新版都要求 Java 17）
- [x] 3.5（新增）`initBackgroundSync()` 移除已 no-op 的 `isInDebugMode: kDebugMode` 參數，順手移除變成無用的 `flutter/foundation.dart` import
- [x] 3.6（新增，iOS 原生層）`ios/Runner/AppDelegate.swift`：`import workmanager` → `import workmanager_apple`；`WorkmanagerPlugin.registerTask(withIdentifier:)` → `WorkmanagerPlugin.registerBGProcessingTask(withIdentifier:)`（原生模組改名，不改會導致 Swift 編譯失敗，archive 直接過不了）

## 4. Android 建置調整

- [x] 4.1 built-in Kotlin 遷移：目前專案用的是 AGP 8.11.1（未到 AGP 9+），Flutter 官方遷移指南僅在 AGP 9+ 才強制要求 built-in Kotlin，AGP 9+ 仍暫時支援 legacy KGP；本次維持 `id("kotlin-android")` 寫法，不做遷移。改為實際需要的調整：`android/settings.gradle.kts` 的 `com.android.application` 8.7.0→8.11.1、`org.jetbrains.kotlin.android` 1.8.22→2.2.20；`android/gradle/wrapper/gradle-wrapper.properties` 的 Gradle 8.10.2→8.14.5（皆為 `flutter build` 實測報出的最低需求）
- [x] 4.2 本機跑 `flutter build appbundle --release`，release build 成功（`app-release.aab`, 82.5MB）
- [x] 4.3 `ndkVersion` 從 `27.0.12077973` 調整為 `28.2.13676358`（`integration_test`/`jni` 透過 workmanager 0.9.x 遞移要求；NDK 版本向下相容，用最高需求值即可）

## 5. 背景同步實測（workmanager 核心功能驗證）

- [x] 5.1（部分驗證，使用者操作）：iOS 模擬器上用真實帳號登入、上班 → 放到背景 3 分鐘（模擬器 Features → Location → Freeway Drive 模擬移動）→ 回前景 → 下班，admin-web 軌跡地圖正確顯示背景期間的完整路徑。確認的是 `UIBackgroundModes: location`（持續背景定位）+ drift 本地佇列 + 批次上傳 + admin-web 渲染這條鏈路端到端正常
- [x] 5.2（使用者操作 + lldb 強制觸發）：關掉本地 api 模擬斷線 → app 前景點「下班」送出失敗、事件卡在本地 pending（UI 顯示「待送出」+ connection refused）→ app 丟到背景 → api 恢復 → `lldb -p <pid> -o 'expr -l objc -O -- (void)[[BGTaskScheduler sharedScheduler] _simulateLaunchForTaskWithIdentifier:@"tw.ccmos.app.bandao.queue-drain"]'` 強制觸發 BGProcessingTask（不必等 iOS 系統排程）→ `clock_out` 事件成功送達 server（`source: "app"`），`checkin_user_status` 正確轉為 `off_duty`。確認 workmanager 0.9.x 新 API（`registerBGProcessingTask` + `AppDelegate.swift` 的原生層遷移）在真實背景執行情境下運作正常

## 6. SnackBar 行為驗證（Flutter 3.38+ 自動消失行為變更）

- [x] 6.1 逐一檢查程式碼中所有 `SnackBar(` 建構呼叫——實際只有 6 處（先前盤點的「12 處」是 `showSnackBar(` 字串重複匹配造成的計算錯誤），分布在：
  - `lib/features/auth/presentation/server_config_screen.dart`（1 處）
  - `lib/features/auth/presentation/home_screen.dart`（2 處）
  - `lib/features/checkin/presentation/home_buttons.dart`（1 處）
  - `lib/features/checkin/presentation/location_consent_dialog.dart`（1 處）
  - `lib/features/checkin/presentation/history_screen.dart`（1 處）
- [x] 6.2 確認 6 處全部是純 `content:` 訊息，沒有任何一處帶 `action:` 參數——Flutter 3.38+「有 action 的 SnackBar 不再自動消失」這個行為變更對本專案零影響，不需要程式碼調整

## 7. iOS 打包驗證

- [x] 7.1 用新 Flutter 版本跑 `flutter build ipa --release`——過程中發現並修正兩個真實 breaking change：`ios/Podfile` + `project.pbxproj`（3 處）的 `IPHONEOS_DEPLOYMENT_TARGET` 13.0→14.0（workmanager_apple 0.9.x 的實際要求）；以及第 3 節記錄的 `AppDelegate.swift` 原生模組改名
- [x] 7.2 修正後 `flutter build ipa --release` 完整跑過 archive + export，成功產出 App Store IPA（36.1MB），沒有再現先前記錄的 Xcode 26 export 問題（已更新對應記憶，見 memory `project_ios_ipa_export_xcode26`）——原本規劃的 `xcodebuild -exportArchive` 手動 workaround 這次沒有用到的必要
- [x] 7.3 Flutter 內建的 App Settings Validation 確認 Version 0.4.0 / Build 10，與 `pubspec.yaml#version`（`0.4.0+10`）一致

## 8. 收尾

- [x] 8.1 `flutter test` 全過（181 個測試）
- [x] 8.2 PR #44（`update-flutter-latest` → `main`）三個 required check 全綠：`analyze + test`、`fmt + clippy + test`、`typecheck + test + build`（後兩者因 path-scoped CI 未動 api/admin-web 直接報 success）
- [x] 8.3 確認 `app/.tool-versions`、`.github/workflows/app.yml`、`README.md`、`app/pubspec.yaml` 四處版本號彼此一致（`flutter-toolchain` spec 的核心要求）
