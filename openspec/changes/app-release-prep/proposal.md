## Why

Bandao 的 Flutter app 目前只是本機 dev artifact — 跑 `flutter run --dart-define=API_BASE_URL=https://bandao-api.ccmos.tw` 才會打到 prod。要讓「任何有打卡需求的公司」能下載並用 admin-web 註冊 Org 加入打卡，必須把 app 變成兩家 store 上經過審查的公開產品。這個 change 是 phase 1：把所有 in-repo 的 release 準備（簽名、版號、權限、Crashlytics、metadata、文件）一次到位，讓操作員能本機手動 cut 第一輪 release。CI 自動化是後續另一個 change，先不混進來。

## What Changes

- **Android signing**：`android/app/build.gradle.kts` 的 release `signingConfig` 從 debug → 從 `android/key.properties` 讀正式 keystore。`.gitignore` 加上 `android/key.properties` + `*.jks`。`google-services.json` 落到 `android/app/`。
- **iOS 版號清理**：`app/ios/Runner.xcodeproj/project.pbxproj` 中 6 處硬寫的 `MARKETING_VERSION = 1.0` / `CURRENT_PROJECT_VERSION = 1` 改用 `$(FLUTTER_BUILD_NAME)` / `$(FLUTTER_BUILD_NUMBER)`。`GoogleService-Info.plist` 落到 `ios/Runner/`。確認 `TARGETED_DEVICE_FAMILY = "1,2"`（iPhone + iPad）保留。
- **Permissions / 文案**：iOS `Info.plist` 已是 When-In-Use，文案補上「上班期間 app 持續上傳位置、背景時 iOS 顯示藍色提示、按下班即停」。Android `Manifest` 確認有 `FOREGROUND_SERVICE_LOCATION`，**不**加 `ACCESS_BACKGROUND_LOCATION`（透過 foreground service 走 sticky notification 路線）。`admin-web /privacy` 內容檢視是否涵蓋 app 端蒐集項。
- **Crashlytics 整合**：`pubspec.yaml` 加 `firebase_core` + `firebase_crashlytics`；iOS Pods + dSYM upload Run Script Phase；Android Google Services + Crashlytics gradle plugins；`main.dart` hook `FlutterError.onError` + `PlatformDispatcher.instance.onError`。**不**呼叫 `setUserId`，crash 不關連使用者身份。
- **Store metadata 結構**：在 `app/store_metadata/{ios,android}/` 建立目錄樹，含 description / promotional_text / keywords / support_url / privacy_url / marketing_url / release_notes / screenshots（iPhone 6.7 + 6.5 + iPad 12.9 / Android phone + tablet）/ feature_graphic.png。Support URL = `mailto:support@ccmos.tw`，Privacy URL = `https://bandao-admin.ccmos.tw/privacy`，Marketing URL 暫留空。Store 顯示名「班到」主名 +「Bandao」副名。
- **CHANGELOG**：repo 根目錄新增 `CHANGELOG.md`（Keep a Changelog 風格），seed 一條 `0.3.0+3`。
- **DEPLOY.md**：新增「App cut release runbook」段落，含 pre-reqs / Android 步驟 / iOS 步驟 / store-side 審查重點 / rollback。
- **ROADMAP**：移除 `[app] Android live smoke for location tracking`（被本 change 的 §7 涵蓋）。**保留** `[cross] Marketing landing site at bandao.ccmos.tw`（獨立 change 處理）。

## Capabilities

### New Capabilities

- `mobile-release`: 規範 Bandao Flutter app 公開上架到 App Store + Google Play 的所有必要條件 — 簽名、版號 source of truth、權限模型（When-In-Use + Foreground Service）、Crashlytics 接線、隱私 / 聯絡管道、binary 對應的支援機型。

### Modified Capabilities

（無 — 沒有任何既有 capability 的 requirement 在此 change 中被改動。`app-checkin` / `app-shell` / `location-tracking` 等行為保持不變，這個 change 只是讓既有產品可以被打包上架。）

## Impact

- **Affected code**:
  - `app/pubspec.yaml`（新增 firebase 套件）
  - `app/android/app/build.gradle.kts`（signing config 重構，Crashlytics gradle plugin）
  - `app/android/build.gradle.kts`（root-level Google Services plugin）
  - `app/android/app/src/main/AndroidManifest.xml`（FOREGROUND_SERVICE_LOCATION 確認）
  - `app/ios/Runner.xcodeproj/project.pbxproj`（6 處版號 placeholder + 確認 device family）
  - `app/ios/Podfile`（Firebase pods）
  - `app/ios/Runner/Info.plist`（usage description 文案）
  - `app/lib/main.dart`（Crashlytics error hooks）
  - 新檔：`app/android/app/google-services.json`、`app/ios/Runner/GoogleService-Info.plist`
  - 新檔：`app/store_metadata/**`
  - 新檔：根目錄 `CHANGELOG.md`
  - 修改：`.gitignore`（android/key.properties、*.jks、Firebase debug keys）
  - 修改：`DEPLOY.md`、`README.md`、`ROADMAP.md`
- **APIs / contracts**: 不影響後端 API 或 admin-web。app 仍使用既有 `Authorization: Bearer <token>` 流程打 `https://bandao-api.ccmos.tw`。
- **Dependencies**:
  - 新增 dart deps：`firebase_core`、`firebase_crashlytics`
  - 新增 native deps：iOS Firebase pods、Android Google Services / Crashlytics gradle plugins
  - 不影響既有 deps 的版本
- **Systems**:
  - 新外部依賴：Firebase 專案（操作員建立並提供 GoogleService-Info.plist / google-services.json）
  - 新外部依賴：Apple Developer + App Store Connect 的 app record（操作員建立）
  - 新外部依賴：Google Play Console 的 app + Play App Signing enrollment（操作員建立）
  - 新外部依賴：Android upload keystore（操作員產生並存於可信賴的 password manager — 1Password / Bitwarden Premium / self-hosted Vaultwarden 之類，需支援檔案 attachment）
  - 新外部依賴：`support@ccmos.tw` mail alias（操作員在 ccmos.tw mail provider 設定）
- **Out of scope（明示延後）**:
  - CI / fastlane release 自動化 → 後續獨立 change，前置條件是這個 change 上架至少一輪
  - `bandao.ccmos.tw` marketing site → ROADMAP 獨立條目
  - 自動 version bump 工具 → 跟 CI change 一起做
  - Sentry / 其他可觀測性 → 已有 Crashlytics 涵蓋 client crash
  - Multi-env flavor builds → prod-only
  - App-side minimum-version check / forced update → 暫不做
