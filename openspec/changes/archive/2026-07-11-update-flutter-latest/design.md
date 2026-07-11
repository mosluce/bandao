## Context

`app/` 的 Flutter 版本目前分散在四個不同步的地方（見 proposal.md），本機開發版本（3.38.7-stable，來自使用者全域 asdf 設定）與 CI 釘死版本（3.29.3）已經有近十個小版本的落差。`workmanager`（背景同步佇列，`app/pubspec.yaml` 核心依賴）被明確上限鎖在 `<0.8.0`，理由是「CI/`.tool-versions` 還停在 3.29.3」——但這個理由本身已經跟現況（`.tool-versions` 從未真的鎖過 flutter）脫節，屬於技術債。

目標版本：3.44.6-stable（asdf 已知最新穩定版；3.46.x 系列仍是 pre-beta，不採用）。

## Goals / Non-Goals

**Goals:**
- 本機（`app/.tool-versions`）與 CI（`.github/workflows/app.yml`）鎖定同一個 Flutter 版本，消除版本漂移。
- 解除 `workmanager` 的人為上限，bump 到相容的最新版本，並驗證背景同步行為未被破壞。
- 其餘直接依賴 bump 到 `pubspec.yaml` 版本限制式下的最新相容版本。
- 針對已知會受影響的行為（SnackBar 自動消失、Android Kotlin/NDK 設定、iOS ipa export）逐一手動驗證，而非只靠 CI 的 `flutter analyze` / `flutter test` 過關。

**Non-Goals:**
- 不追新到 pre-release/beta channel（3.46.x）——只用 stable channel 的最新版。
- 不順便重構已存在的 riverpod/go_router/drift 架構模式，僅做版本 bump。
- 不處理 `api/`、`admin-web/` 的任何版本或依賴。
- 不在這次變更內把 `flutter-toolchain` 的版本鎖定機制自動化成 CI 檢查（例如比對 `.tool-versions` 與 workflow 是否一致的 lint）——先建立契約與人工遵循，自動化留待未來需要時再開。

## Decisions

**D1. 用 `app/.tool-versions` 而非只改 CI workflow 來鎖版本。**
理由：這次問題的根源就是「本機版本不受 repo 控管」。單獨改 CI 只解決 CI 端，本機開發者（含未來的協作者）clone 下來還是會抓到自己全域 asdf 設定的版本，問題會再發生。`app/.tool-versions` 是 asdf 的標準機制，`asdf install` 會自動抓專案內鎖定版本，且 repo 裡已經有 `app/.tool-versions` 存在（目前只鎖 ruby），是延續既有慣例而非新增機制。
替代方案考慮過：只在 README 用文字說明「請用 3.44.6」——文字說明不會被工具強制執行，容易再度漂移，否決。

**D2. CI workflow 的 `flutter-version` 手動同步，而不是從 `.tool-versions` 動態讀取。**
理由：`subosito/flutter-action` 目前的用法是直接寫死版本字串；改成從 `.tool-versions` 動態解析版本號需要額外的 shell 步驟，增加 workflow 複雜度，對一個單人維護、變更頻率低（一年可能一兩次）的版本號來說，維護動態解析的成本大於手動同步兩處字串的成本。`tasks.md` 會把「兩處必須一致」列成明確的驗證步驟，降低漏改風險。
替代方案考慮過：`.tool-versions` 動態讀取進 CI——增加的 shell 邏輯量不成比例，否決。

**D3. `workmanager` 上限解除後，直接 bump 到當下最新穩定版本（而非只開一點點上限，比如 <0.9.0）。**
理由：0.8.0 本身就是 breaking release（enum 命名、inputData 傳遞方式都變了），既然背景同步程式碼一定要因應這次調整，不如一次調整到當下最新版本，避免下次升級又要重新讀一次 changelog、重新測一次。
風險對應見下方 Risks。

**D4. Android `build.gradle.kts` 的 Kotlin 套用方式與 `ndkVersion` 先「照 Flutter 官方遷移指南調整、實測通過即可」，不預先假設具體改法。**
理由：Flutter 3.44 的 built-in Kotlin 遷移細節（是否移除手動 `id("kotlin-android")`、`kotlin_version` 宣告位置怎麼變）需要在實際跑 `flutter create` migration 提示或官方遷移文件時才能確認精確步驟，設計階段先框定「這是一個必須做的驗證/調整項目」，實際改法留給 `tasks.md` 執行時對照官方指南操作。

## Risks / Trade-offs

- **[Risk] `workmanager` API 變更（enum snake_case→camelCase、inputData JSON→原生 Map）導致背景同步佇列在升級後靜默失敗，且這類問題不容易被 `flutter test` 的 unit test 抓到（背景執行、平台 channel 相關）。**
  → Mitigation: `tasks.md` 把「Android 實機/模擬器上手動觸發一次背景同步、確認 log 顯示任務執行成功」列成獨立、不可跳過的驗證任務；iOS 背景執行機制不同（BGTaskScheduler），若專案的 workmanager 用法有 iOS 分支也要對應驗證。

- **[Risk] Android built-in Kotlin 遷移改動 `build.gradle.kts` 的套用方式，若遷移不完整可能導致 release build 失敗但 debug build 正常（因為簽章/優化路徑不同），CI 的 `flutter analyze`/`flutter test` 不會跑 release build 所以抓不到。**
  → Mitigation: 驗證任務裡明確加入本機跑一次 `flutter build appbundle --release`（呼應 `mobile-release` capability 既有的驗證方式），不只依賴 CI。

- **[Risk] `ndkVersion` 手動釘死在 `27.0.12077973`，若 Flutter 3.35+ 預設 `abiFilters` 行為與這個 NDK 版本產生非預期組合（例如缺某個 ABI 的 prebuilt library），只有在 release build 或特定裝置架構上才會炸開，開發機上 debug 可能完全正常。**
  → Mitigation: 同上，用 release build 驗證；若發現不相容，`tasks.md` 允許同步調整 `ndkVersion` 到 Flutter 3.44 建議值。

- **[Risk] SnackBar 自動消失行為變更（3.38+）影響 7 個檔案 12 處用法中「預期使用者看到訊息後手動點掉」的场景（例如需要使用者明確確認的錯誤訊息），若行為改變後訊息消失時機不對，容易被誤判為「小事、之後修」而漏掉。**
  → Mitigation: 這次不套用「排之後修」的預設，明確把 12 處用法列成 `tasks.md` 裡的逐一檢查清單，而不是籠統的一條「檢查 UI」。

- **[Risk] iOS export 流程（Xcode 26）先前已知有坑，這次連 Flutter engine 版本都換了，风险面比單純 Xcode 版本問題更大，且沒有自動化 CI 覆蓋 iOS 打包（`app.yml` 只跑 `analyze`/`test`，不含 build ipa）。**
  → Mitigation: `tasks.md` 把「用 `xcodebuild -exportArchive` 走一次完整 iOS 打包流程」列成獨立驗證任務，且明確標註「不要用 `flutter build ipa` 直接 export」（沿用先前踩坑的教訓）。

- **[Trade-off] 其餘依賴（riverpod/go_router/dio/firebase_core/firebase_crashlytics/geolocator/flutter_map/drift/sqlite3_flutter_libs）一次性 bump 到最新相容版本，會讓這次變更的 diff 範圍變大、難以在出問題時快速定位是哪個依賴造成的。**
  → 接受此權衡：這些套件目前查無已知破壞性變更，且 `flutter test`/`flutter analyze` 加上手動驗證清單已經是這次變更的把關手段；若之後真的要定位問題，`pubspec.lock` 的 diff 本身就是可回溯的清單。

## Migration Plan

1. 建立 `app/.tool-versions` 的 flutter 鎖定，本機 `asdf install` 驗證可抓到正確版本。
2. 更新 CI workflow 版本號，確認 PR 上的 `analyze + test` check 用新版本跑過。
3. 解除 `workmanager` 上限、bump 全部直接依賴，`flutter pub get` 產生新 `pubspec.lock`。
4. 逐項跑 Risks 段落列出的手動驗證（背景同步、Android release build、SnackBar、iOS export）。
5. 更新 `README.md` 文字。
6. 全部驗證通過後合併；若中途發現任何一項無法在合理時間內修好（例如 workmanager 新版有更嚴重的相容性問題），可以把「其餘依賴 bump」與「workmanager bump」拆成兩個獨立 PR 分開合併——`tasks.md` 的任務顆粒度會保留這個拆分彈性。

Rollback：純 git revert 即可，沒有資料遷移或不可逆的外部狀態變更；`app/.tool-versions` 與 CI 版本號同步 revert。

## Open Questions

- Flutter 3.44 的 built-in Kotlin 遷移確切步驟，需要在實作階段對照官方遷移文件/`flutter create` 的遷移提示才能定案，目前設計階段無法給出精確 diff。
- `ndkVersion` 是否需要調整、調整到哪個值，取決於實測 release build 的結果，無法在設計階段預先決定。
