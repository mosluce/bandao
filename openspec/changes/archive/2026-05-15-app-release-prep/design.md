## Context

Bandao 的 Flutter app 已經有完整功能（登入、打卡、轉場、軌跡記錄、本機 queue、背景同步、事件歷史），打到 prod api `https://bandao-api.ccmos.tw` 時走的是 build-time `--dart-define=API_BASE_URL=...`。但要從「dev 機器跑得起來」變成「兩家 store 上有審查通過的公開產品」，還缺一條完整的 release prep 路徑：

- **簽名**：Android release 還用 debug keys（`build.gradle.kts` 已有 `// TODO: Add your own signing config` 註解），Play Store 不收。
- **版號 source of truth**：`pubspec.yaml = 0.3.0+3`，但 iOS `project.pbxproj` 有 6 處硬寫 `MARKETING_VERSION = 1.0` 跟 `CURRENT_PROJECT_VERSION = 1`，雖然不影響實際 archive 出來的版本（Info.plist 用 `$(FLUTTER_BUILD_*)` 取代），但 Xcode UI 顯示混亂。
- **權限文案**：iOS Info.plist 已正確使用 `NSLocationWhenInUseUsageDescription` 並開了 `UIBackgroundModes = ["processing", "location"]`，但文案要強化成 reviewer 一看就懂的版本。Android 的 sticky notification 要靠 `FOREGROUND_SERVICE_LOCATION` permission 而不是 `ACCESS_BACKGROUND_LOCATION`。
- **Crash visibility**：上架後使用者裝置 crash 完全黑箱，沒有任何 crash report 機制。
- **Store metadata**：description / screenshots / 隱私聲明等內容都還沒寫。
- **Privacy / Support 對外管道**：admin-web `/privacy` 已存在；個人 email `mosluce@no8.io` 不應該出現在 store metadata 上（repo 是 public，metadata 上架後也對所有使用者公開）。
- **文件**：`DEPLOY.md` 涵蓋了 api / admin-web / Mongo / Tailscale / S3，但沒有 app cut release 的 runbook。

操作員（`mosluce`）已開好 Apple Developer + Google Play Console + Firebase 帳號，但都還沒註冊 Bundle ID。Bundle ID `tw.ccmos.app.bandao` 已寫死在兩邊 native code（`Info.plist` 跟 `build.gradle.kts`）。

## Goals / Non-Goals

**Goals:**
- 把 app 從 local dev artifact 變成可以**手動**從操作員機器 cut 出 release，再 upload 到 App Store + Play Store 過審的產品。
- 所有 in-repo 的 release 準備（簽名 wiring、版號清理、權限文案、Crashlytics 接線、metadata 結構、文件）一次到位。
- Spec 層次定義「Bandao 公開上架的最小必要條件」，未來 audit 跟 phase 2 CI change 都對得上。
- iPad 支援保留（`TARGETED_DEVICE_FAMILY = "1,2"`），但只做基本 layout smoke 不做 iPad 特化體驗。
- Public repo 友善：個人 email、keystore、Firebase 私鑰都不入庫；可入庫的 Firebase config plist / json 是設計上允許公開的（Google 自己的安全模型靠 Cloud quota / API restrictions，不靠 client config 保密）。

**Non-Goals:**
- CI release pipeline（fastlane、tag-driven build / upload）— 留給後續 change，前置條件是這個 change 至少手動 cut 過一輪、跑通兩家審查。
- Marketing landing site（`bandao.ccmos.tw`）— ROADMAP 獨立條目，不阻塞 app 送審。
- Auto version bump / release-notes 自動生成 — phase 2 一起做。
- Multi-env flavor builds（dev / staging / prod 分開 build）— 目前只有 prod，要切 staging 是另一個 change。
- App-side minimum-version check / forced update — 等真的有 breaking API 變動再說。
- Sentry / Datadog / 其他可觀測性方案 — Crashlytics 涵蓋 client crash 已足夠 phase 1。
- iPad 特化 layout — 只確認既有 layout 在 iPad 不會壞。

## Decisions

### D1：Play App Signing（不走傳統 self-managed keystore）

**選擇**：上傳 first .aab 時讓 Google Play Console 接管 signing，操作員只持有 upload keystore。

**理由**：
- Upload keystore 丟了：申請 reset，Play 重新給；`applicationId tw.ccmos.app.bandao` 永遠安全。
- 傳統做法：keystore 丟了 = applicationId 永久廢掉，只能換 ID 重新上架，現有使用者要重新下載。
- Google Play 現在預設推 Play App Signing，老作法已不被鼓勵。
- 對 phase 2 CI 友善：CI 拿 upload keystore 即可，不必接觸 final app signing key。

**替代**：Self-managed keystore — 已過時，否決。

### D2：When-In-Use + Foreground Service（不走 Always / ACCESS_BACKGROUND_LOCATION）

**選擇**：
- iOS：`NSLocationWhenInUseUsageDescription` + `UIBackgroundModes = ["location"]`，靠 active CLLocationManager session 在背景持續取得位置（iOS 顯示藍色 status bar）。
- Android：`FOREGROUND_SERVICE` + `FOREGROUND_SERVICE_LOCATION`（Android 14+ 必要）+ 既有的「工作期間定位追蹤中」sticky notification。**不**宣告 `ACCESS_BACKGROUND_LOCATION`。

**理由**：
- Bandao 是手動打卡（使用者按上班 → 啟動 location session → 按下班 → 結束），**不需要** geofencing / 終止後喚醒 / 開機自動恢復這些 Always / Background Location 才能做的事。
- App Store 審查對 Always 敏感度極高；Play Store 對 Background Location 也要求填 form + 提交 demo 影片。走輕量路線可顯著降低送審阻力。
- 使用者授權對話框：When-In-Use 一次過，Always 要兩次（先 In-Use 再升級），拒絕率高。
- iOS 藍 bar 跟 Android sticky notification 對使用者更透明，符合 Bandao「上班才追蹤」的產品語意。
- 將來若需要 geofence 自動打卡（Always 才能做），再升級權限；升級成本比一開始就 Always 低（使用者已信任 app）。

**替代**：
- Always + ACCESS_BACKGROUND_LOCATION — 多花的審查阻力換不到 phase 1 用得到的 feature，否決。
- 純 In-Use（無 Background Modes）— app 切背景就停止追蹤，無法支援 Bandao 的「上班期間連續追蹤」核心場景，否決。

### D3：Firebase Crashlytics（不接 Sentry / 不做 client observability stack）

**選擇**：用 Firebase Crashlytics 收 client crash + non-fatal error，**不**呼叫 `setUserId`，crash 不關連 Bandao 使用者身份。

**理由**：
- Phase 1 主要痛點是「上架後 crash 黑箱」，Crashlytics 是 Firebase / Google 一條龍最低門檻方案。
- iOS dSYM upload + Android Mapping File upload 兩家工具都成熟（Crashlytics SDK + gradle plugin 自動處理）。
- 公開 repo 友善：Firebase config 檔（`GoogleService-Info.plist` / `google-services.json`）Google 設計上允許 client-side 公開；安全模型靠 server-side rules / API quota / certificate pinning（後者非必要），不靠 client config 保密。
- 不關連 user identity：crash report 對 debug 夠用（device model / OS version / stack trace），同時不增加隱私敏感度，App Privacy / Data Safety 可宣告為 not-linked。
- ROADMAP 已有「[infra] 監控與告警 Sentry / Loki / Grafana」條目處理 server-side observability，跟 client crash 切開乾淨。

**替代**：
- Sentry（client + server 同一 stack）— 設定成本較高，phase 1 過早優化，否決。
- 不接 crash report — 上架後 debug 全靠使用者回報，無法接受。

### D4：iOS 6-spot 版號 placeholder（一次清理 RunnerTests / Debug / Release / Profile 全部）

**選擇**：把 `project.pbxproj` 中 6 處 `MARKETING_VERSION = 1.0` / `CURRENT_PROJECT_VERSION = 1` 全改成 `$(FLUTTER_BUILD_NAME)` / `$(FLUTTER_BUILD_NUMBER)` placeholder。

**理由**：
- 已知 archive binary 的版本由 `Info.plist` 的 `$(FLUTTER_BUILD_*)` 決定（Apple 認 Info.plist），所以這 6 處硬寫值不影響 ship。
- 但每次 GUI 編輯 Xcode → 寫死回 1.0；下次有人手動改版本 → Xcode 又寫死 → 越來越多 mismatch。
- 一次全改 placeholder 後，未來打開 Xcode UI 看到的版本永遠跟 pubspec 對得上，省下「Xcode UI 跟實際 ship 版本不同」的混淆。

**替代**：只改其中影響 archive 的（事實上沒有任何一處影響） — 多此一舉，否決。

### D5：iPad 保留支援，但只做基本 smoke

**選擇**：保留 `TARGETED_DEVICE_FAMILY = "1,2"`，metadata 提供 iPad 12.9" screenshots（≥2 張）；不做 iPad 特化 layout。

**理由**：
- 移除 iPad 後想加回需要送審 — 「先保留」比「先移除再加回」省。
- Bandao 主場景是 iPhone（隨身打卡），但合理副場景：辦公室共用 kiosk、admin 用 iPad 看 dashboard。
- Flutter 預設 layout 在 iPad 通常不會壞，最壞是「太空」不是「破版」。
- 多花的成本：metadata 多一組 screenshots（Android 一組 tablet screenshots），manual smoke 多 5 分鐘看 iPad simulator。

**替代**：移成 iPhone-only — 將來想做 kiosk 就要再送審加 iPad，且 store metadata 要重做，否決。

### D6：Support URL = `mailto:support@ccmos.tw` alias（不放個人 email、不寫 build-time inject）

**選擇**：在 ccmos.tw mail provider 設 alias `support@ccmos.tw → mosluce@no8.io`。`store_metadata/.../support_url.txt` 直接 commit `mailto:support@ccmos.tw`。

**理由**：
- Store 上架後，support URL 對所有使用者公開顯示，無論 repo 公私 — 個人 email 寫進 store 就是公開了。
- Alias 是可逆決定：將來改維護者，改 forward 即可，不用動 metadata。
- 域名 `ccmos.tw` 跟產品 `bandao-admin.ccmos.tw` / `bandao-api.ccmos.tw` 一致，品牌統一。
- Build-time inject 對 metadata-only 內容（不進 binary）是 over-engineering：build pipeline 多一層複雜度，並沒有保護到任何不能公開的東西。

**替代**：
- admin-web 加 `/support` 頁面 — 多一頁前端工，但等 marketing-site change 一起做更合理，這個 change 用 mailto 起手即可。
- Build-time inject — overkill，否決。

### D7：Privacy URL 暫指 admin-web，未來無痛切到 marketing site

**選擇**：`store_metadata/.../privacy_url.txt = https://bandao-admin.ccmos.tw/privacy`（admin-web 既有 `/privacy.vue`）。

**理由**：
- 現狀 `/privacy` 已存在且涵蓋產品端蒐集項；不必為了上架而 block 在 marketing-site change 上。
- 將來 marketing-site 上線後，把 `/privacy` 搬到 `bandao.ccmos.tw/privacy`（admin-web 改 redirect），store metadata 改一行 URL → upload metadata（不需 binary 重 build），review fast-track 不用走完整審查。

**替代**：等 marketing-site 上線再開始 app 送審 — 兩個 change 串成 dependency 鏈，多花至少 2 週，否決。

### D8：Marketing URL 暫留空（兩家 store 都接受）

**選擇**：`marketing_url.txt` 空檔案；App Store Connect 的 Marketing URL 欄位不填；Play Console 的 Website 欄位暫填 admin-web（次佳）或留空。

**理由**：
- App Store Marketing URL 非必填；Play Store Website 非必填（但建議填）。
- 留空比暫指 admin-web 好：admin-web 是 product app，使用者點過去看到登入頁會困惑。
- Marketing-site 上線後再補。

**替代**：暫指 admin-web — 使用者體驗較差，否決。

### D9：Crashlytics force-crash test 透過 debug-only flag 隱藏

**選擇**：在 `main.dart` 或 settings 頁加一個只在 debug build 出現的「測試 crash」按鈕，release build 不存在。

**理由**：
- 必須能驗 Crashlytics 整合確實 work（force crash → Firebase console 收得到 → 對得上 dSYM）。
- Release binary 出現「測試 crash」按鈕會被 Apple reviewer 質疑（為什麼 production app 有這種功能）。
- 用 `kDebugMode` 或 `--dart-define=ENABLE_CRASH_TEST=true` 都可，建議 `kDebugMode`（不用記額外 flag）。

**替代**：用 `--dart-define` flag — 操作員必須記得開、容易漏，否決。

### D10：CHANGELOG.md 在 repo 根（不在 app/）

**選擇**：`/CHANGELOG.md` 在 repo 根目錄，紀錄 app（之後也可以紀錄 admin-web / api）的 release。

**理由**：
- Phase 1 只 release app，但未來 admin-web / api 也會走類似 cadence。
- 一個根 CHANGELOG 比 `app/CHANGELOG.md` + `admin-web/CHANGELOG.md` 好維護，使用者只要看一個檔。
- 內容用 sections (`### App` / `### admin-web` / `### api`) 分組。

**替代**：`app/CHANGELOG.md` — 將來 admin-web 要 release notes 又得做一次，否決。

## Risks / Trade-offs

- **Risk**：第一次 App Store 審查被 reject（location justification / privacy 文案 / metadata）→ **Mitigation**：§7 smoke 完整跑兩家，TestFlight + Play Internal Testing 各裝起來看實機行為；reviewer 通常 reject 1–2 次很正常，留意他們指出的點精準回覆。
- **Risk**：Android upload keystore 弄丟（操作員機器掛掉、password manager 沒同步）→ **Mitigation**：§0 操作員 task 強制要求把 .jks + passwords 都進 password manager 同一筆 item（任何支援 binary attachment 的 manager 皆可：1Password、Bitwarden Premium、self-hosted Vaultwarden 等），並在第二台裝置驗證可還原；DEPLOY.md runbook 包含「重新申請 upload key」的 Play Console 流程（Play App Signing 允許）。
- **Risk**：Firebase config 落入 public repo 被濫用 → **Mitigation**：Firebase 安全模型不依賴 client config 保密；Crashlytics 是只寫 endpoint，不會洩漏資料；如果發現異常用 quota，Firebase Console 可重發 config + 限制 API。Acceptable risk。
- **Risk**：iOS Background Modes "location" 在 reviewer 眼中是 Always 的 proxy → **Mitigation**：Info.plist 文案強化講清楚「按下班即停」；in-app rationale dialog 在第一次 prompt 前先顯示說明；如果被 reject，submit notes 引用 Strava / Google Fit 的同類 pattern。
- **Risk**：iPad smoke 發現既有 layout 嚴重破版 → **Mitigation**：§7 smoke 排在 §6 console 上 metadata 之前，發現問題可選擇暫時 drop iPad（改 `TARGETED_DEVICE_FAMILY = "1"`）後再送審。文件中標註此 fallback 路徑。
- **Trade-off**：Marketing URL 暫留空 → **Trade-off**：產品頁少一個外連，但避免暫指 admin-web 造成的混淆；marketing site 上線後補上。
- **Trade-off**：CHANGELOG 在 repo 根 → **Trade-off**：app 端 contributors 多了一個要更新的檔案（在 app/ 裡找不到），但換來統一性。第一條 entry 寫清楚這個約定。

## Migration Plan

這個 change 沒有 runtime migration（沒改 schema、沒改 API、沒改使用者資料）。Deployment 路徑：

1. **PR merge 到 `main`** — 帶簽名 wiring、版號清理、Crashlytics、metadata 結構、文件變動。
2. **Operator 跑完 §0 pre-flight**（部分可在 PR 之前完成、不阻塞 PR merge）— 產 keystore、開 Firebase 專案、設 mail alias、開 store consoles。
3. **Operator 本機 smoke**（§7） — 用 §1–§4 的成果做 release build → TestFlight + Play Internal Testing → 實機驗 location flow + Crashlytics 收 crash。
4. **Operator 上 metadata 並 submit for review**（§6 + §8） — 兩家平行送。
5. **過審後**：app 在 App Store + Play Store 上線。

**Rollback** 對應策略（不需 migration、純 binary 不通過時）：
- 審查 reject：依 reviewer 指示修；通常是 metadata / 文案、極少需要動 code。
- 上架後發現 crash：用 Crashlytics 看，hot-fix 走 patch release（pubspec bump → 再 cut → 再上）。
- 完全不能上：移除 store 上的 binary，maintain admin-web 為主要使用者入口（沒有 app 也能用 admin-web 端 manual checkin，雖然體驗差）。

## Open Questions

1. **App Store / Play Console 的 In-App Purchase 申報**：Bandao 是免費 app 沒有 IAP；要在兩家 console 對應 toggle 設定為 free / no IAP。確認操作員 §6 task 涵蓋此項即可。
2. **iPad screenshots 的內容**：iPad 12.9" screenshot 至少 2 張，要展示哪些畫面？建議：登入畫面、主畫面、checkin dashboard。實作 §5 時操作員自行決定。
3. **Crashlytics non-fatal error 範圍**：除了 uncaught error，要不要把 dio API 4xx/5xx error 也 record 成 non-fatal？暫定：先只接 uncaught；§4 實作時再評估。
