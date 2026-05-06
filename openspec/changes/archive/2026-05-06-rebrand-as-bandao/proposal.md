## Why

「argus」是希臘神話的百眼巨人 — 永不闔眼的監視者。在「員工打卡 + 位置追蹤」這個產品上，這個 codename 對工人的氣味直接是「被監視」。但我們刻意 ship 出來的產品方向相反：consent dialog 才開始追蹤、街道層級不到門牌、90 天保留期、admin 審批 join、Org-level toggle、可以 cancel join request — 全部都是 **privacy-aware** 的工人視角設計。品牌跟設計打架。

決定改名為「**班到**」（拼音 Bandao）：

- **動詞、動作感**：「我班到了」是工人會講的話，名字本身就是日常用語
- **覆蓋全產品功能**：打卡 / 轉場 / 請假（沒班到）/ 排班（何時班到）/ Gamification（連續 N 天班到）每一個都讀得通
- **TW 在地**：純中文、台味、跟 ccmos.tw 母品牌呼應
- **品牌敘事**：「先班到，再上工」「班到沒？」「準時班到」 — slogan 寫得出來
- **無撞名**：Google「班到」當作品牌沒有現有 SaaS 競品

順手把 GitHub repo / 根目錄路徑也統一到 bandao（你決定要不要動 GitHub repo rename）。

## What Changes

純識別字串替換 — 沒有 spec-level 功能變動、沒有產品行為變動。

### 程式碼識別

- `api/Cargo.toml`：`argus-api` → `bandao-api`（package name + binary name）
- `admin-web/package.json`：`argus-admin-web` → `bandao-admin-web`
- `app/pubspec.yaml` + Flutter package imports：`argus_app` → `bandao_app`
- iOS `Info.plist` `CFBundleName` / `CFBundleExecutable`：`argus_app` → `bandao_app`
- iOS bundle id：`tw.ccmos.app.argus` → `tw.ccmos.app.bandao`
- iOS BGTask identifier：`tw.ccmos.app.argus.queue-drain` → `tw.ccmos.app.bandao.queue-drain`
- Android `applicationId` / package：對應更新
- Nominatim User-Agent：`argus-api/0.1.0` → `bandao-api/0.1.0`

### Storage / runtime keys

- Flutter SecureStorage 前綴：`argus.location_tracking.*` → `bandao.location_tracking.*`
- Drift db 名（如有 prefix）：對應更新
- 任何 dart-define / env var 含 `ARGUS_*` 改成 `BANDAO_*`

⚠️ Storage prefix 改名 = 既有測試裝置的 token / pending queue / consent flag 全部失效，等同於登出重來。Pre-store-launch 階段可接受（測試帳號可丟）。

### 文件 / 命名

- 根 `README.md` 標題、描述
- `AGENTS.md` 標題、產品概述
- `ROADMAP.md`
- `api/README.md`、`admin-web/README.md`、`app/README.md`
- `openspec/specs/*` 內所有 `argus` 字面（多半是註解 / 例句）
- 既有 archived changes 內的 `argus` 字串保留不動（歷史紀錄）
- admin-web `<title>`、Privacy 頁面、login screen 中的「argus admin」改成「班到 admin」

### 測試

- 既有 test 中 hard-coded 的 `argus`（少數）改成 `bandao`
- 不重構測試結構

### 不動的部分

- **archived changes 內 `argus` 不動**（歷史紀錄）
- **GitHub repo 名稱、根目錄路徑：使用者自行決定**（Apply 階段詢問）
- 商業 / DB 邏輯一律不動
- spec 行為定義不動（純識別字串改動）

## Capabilities

### New Capabilities

- `brand-identity`：產品 brand identity 的 system commitment — 命名、bundle id、storage prefix、HTTP user-agent 等對外識別字串都統一在「bandao」前綴下。Documents the rebrand as the product's source-of-truth naming.

### Modified Capabilities

（無 — 這個 change 不改任何 spec 行為）

## Impact

- **`api/`**：~30 檔案 touch（package name + bundle 字串 + Cargo.toml + 部分 README + 部分 test assertion 字串）
- **`admin-web/`**：~10 檔案 touch（package.json + UI 標題 + 部分 README）
- **`app/`**：~50 檔案 touch（pubspec + Flutter import path 全部、iOS plist、Android manifest、storage keys、所有 Dart import `package:argus_app/...` → `package:bandao_app/...`）
- **`openspec/specs/`**：少數註解
- **CI / GitHub**：repo rename 視 user 決定
- **DB schema / 行為**：完全不動（commit 後 server 上線就生效，不需 migration）
- **既有測試裝置**：Storage 失效 = 等同登出。可接受（pre-launch 階段）。
- **既有 admin 帳號 / DB 資料**：完全不受影響（DB 不動，只是 client 端 prefix 改名）
- **依賴：** 無
