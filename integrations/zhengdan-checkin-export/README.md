# 震旦雲打卡匯出（單一客戶客製整合）

把班到當日的上/下班紀錄，定期匯出成震旦雲能吃的固定寬 UTF-8 純文字檔，放進客戶
Windows Server 上的指定資料夾，讓震旦雲照原本的方式讀取匯入。這是**單一客戶的客製
部署**，不是給任何 Org 自助開通的通用功能——`export.ps1` 直接綁定震旦雲的目標格式
與 +08:00 時區。

底層打的 API（`GET /orgs/me/checkin/events/export`）本身是通用的 JSON endpoint，
格式化成震旦雲文字檔的邏輯全部在這支 script 裡，不在 API 端——細節見
`openspec/changes/archive/`（apply 完成後）或當前 `openspec/changes/add-zhengdan-checkin-export/design.md` 的 D4。

## 部署環境

Windows Server 2016 Datacenter，內建 PowerShell 5.1。

## 一次性設定

### 1. 在班到 admin-web 建立 API Token

1. 用組織 admin 帳號登入 admin-web → 「管理員工具」→「API Token」。
2. 「+ 建立 API Token」→ 名稱填 `震旦雲匯出` 之類好辨識的名字 → scope 勾選
   `checkin:read` → 建立。
3. 建立當下會顯示一次明碼，**立刻複製**（關閉視窗後系統不會再顯示第二次）。
4. 之後若懷疑外洩，用同一頁的「Rotate」按鈕換發新密鑰，並同步更新這台機器上的
   `config.ps1`——rotate 之後舊密鑰立即失效。

### 2. 準備設定檔

在 `export.ps1` **同一個資料夾**複製一份 `config.example.ps1`，改名成 `config.ps1`，
填入：

```powershell
$ApiBaseUrl = 'https://bandao-api.ccmos.tw'
$ApiToken = '<上一步拿到的明碼>'
$TargetFolder = 'C:\Zhengdan\Import'   # 震旦雲監看的資料夾，跟客戶 IT 確認實際路徑
```

`config.ps1` 已經被 `.gitignore` 排除，但**不要只依賴 `.gitignore`**——這個目錄如果
未來被納入版本控制系統管理，`config.ps1` 這種帶明碼憑證的檔案理想上應該放在這個
git checkout 目錄之外（例如另一個不受版控的資料夾），`export.ps1` 用絕對路徑指到
它。實際部署時看客戶機器上的檔案系統慣例調整。

### 3. 註冊 Task Scheduler 排程

1. 開啟「工作排程器」（Task Scheduler）→ 建立工作。
2. 觸發程序：每小時執行一次。
3. 動作：
   - 程式/指令碼：`powershell.exe`
   - 引數：
     ```
     -ExecutionPolicy Bypass -File "C:\path\to\export.ps1"
     ```
     用 `-ExecutionPolicy Bypass` 包在單一排程動作裡，**不要**去改整台機器的系統
     執行原則（`Set-ExecutionPolicy`）——影響範圍只限這個排程工作本身，比較安全。
4. 存檔後手動執行一次，確認 `TargetFolder` 底下出現一個新的
   `<yyyyMMddHHmmss>.txt`，且同目錄下的 `export.log` 顯示 `OK`。

## 檔案格式

```
郭文賓                 20260707064744上班
```

= 姓名（右補空白到固定 20 個**字元**寬）+ `YYYYMMDDHHmmss`（+08:00）+ `上班`/`下班`，
三段之間沒有任何分隔符。整份檔案 UTF-8（**無 BOM**）、`CRLF` 換行、結尾沒有多餘的
換行。這個格式是逐 byte 比對過震旦雲實際匯出的真實範例檔反推出來的，不是憑空猜的。

檔名是**執行當下的時間戳記**（`yyyyMMddHHmmss.txt`），每次執行都是新檔案，不覆蓋
舊檔——沿用震旦雲原本自己吐檔的命名慣例。

## PowerShell 5.1 的 UTF-8 BOM 陷阱

Windows Server 2016 內建的 PowerShell 5.1，`Out-File -Encoding utf8` 跟
`Set-Content -Encoding UTF8` **都會偷偷在檔案開頭加上 BOM**（`EF BB BF`）：

```powershell
# 錯誤示範 —— 這兩種寫法在 PS 5.1 都會產生帶 BOM 的檔案：
$Content | Out-File -FilePath $OutFile -Encoding utf8
Set-Content -Path $OutFile -Value $Content -Encoding UTF8
```

`export.ps1` 用的是不會加 BOM 的正確寫法：

```powershell
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($OutFile, $Content, $Utf8NoBom)
```

如果之後要修改這支 script 的寫檔邏輯，**務必維持這個寫法**——BOM 有沒有加，對震旦雲
那邊的匯入程式影響多大目前無法確認（可能整行、整批資料匯入失敗），沒有必要冒這個
風險。

## 已知限制

- **姓名比對，不是員工代號**：震旦雲那邊目前是用姓名比對匯入資料，同名同姓在客戶端
  是既有限制，這個整合不處理，也無法處理。
- **失敗不重試**：`export.ps1` 呼叫 API 失敗（逾時、網路中斷、伺服器錯誤）時不會在
  這次執行內重試，只會寫 log、等下一次整點的排程觸發。如果客戶機房斷線超過數小時，
  震旦雲會持續讀不到新資料，目前沒有告警機制，需要定期人工檢查 `export.log`。
