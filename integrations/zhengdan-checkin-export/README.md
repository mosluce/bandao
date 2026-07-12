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

## PowerShell 5.1 的 TLS 1.2 陷阱

Windows Server 2016 內建 .NET Framework 預設的 `SecurityProtocol` 不含 TLS 1.2，
連現代 HTTPS 網站（包含正式環境 API、GitHub）都會直接失敗：

```
Invoke-WebRequest : 要求已經中止: 無法建立 SSL/TLS 的安全通道。
```

`export.ps1` 已經在最前面自動加了這行處理，不用另外做：

```powershell
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
```

但**下載 script 本身**這一步是在這行生效之前，如果要用 `Invoke-WebRequest` 從
GitHub 下載檔案，得先在當次 PowerShell session 手動下這行指令，再下載：

```powershell
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/mosluce/bandao/main/integrations/zhengdan-checkin-export/export.ps1" -OutFile "export.ps1"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/mosluce/bandao/main/integrations/zhengdan-checkin-export/config.example.ps1" -OutFile "config.example.ps1"
```

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

**選項 A：用 `register-task.ps1` 自動註冊（建議）**

跟 `export.ps1` 放同一個資料夾，下載後直接跑：

```powershell
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/mosluce/bandao/main/integrations/zhengdan-checkin-export/register-task.ps1" -OutFile "register-task.ps1"
.\register-task.ps1
```

這支 script 是**可重複執行**的——如果同名的工作已經存在，會先移除再重新建立，不會
報錯，改路徑或改設定後直接重跑就好。預設用 `SYSTEM` 帳號執行（不用存密碼、沒人登入
也照跑）；如果 `export.ps1` 之後需要用到必須是互動使用者才能存取的資源（例如網路
磁碟機），打開 `register-task.ps1` 把 `$RunAsSystem` 改成 `$false`，會改成互動輸入
密碼的方式註冊。跑完會自動觸發一次測試執行，並印出 `Get-ScheduledTaskInfo` 的結果。

**選項 B：手動用工作排程器 GUI 註冊**

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

**不管用哪個選項**，設定完都手動觸發一次，確認 `TargetFolder` 底下出現一個新的
`<yyyyMMddHHmmss>.txt`，且同目錄下的 `export.log` 顯示 `OK`。

### 4. 移除排程

用 `unregister-task.ps1`（同樣可重複執行，工作不存在時不會報錯，只會印訊息）：

```powershell
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/mosluce/bandao/main/integrations/zhengdan-checkin-export/unregister-task.ps1" -OutFile "unregister-task.ps1"
.\unregister-task.ps1
```

或用 GUI：工作排程器裡找到「Zhengdan Checkin Export」這個工作，右鍵刪除。

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

## PowerShell 5.1 的 `Invoke-RestMethod` 中文亂碼陷阱

實測發現：如果用 `Invoke-RestMethod` 打 API，回來的中文姓名會變成一堆重音拉丁字母
的亂碼（例如「陳聖夫」變成「é³èå¤«」），而且**用記事本手動選 UTF-8 開啟一樣是亂
碼**——不是顯示問題，是資料在 PowerShell 解析 HTTP 回應那一步就已經解壞了。

原因：PowerShell 5.1 的 `Invoke-RestMethod`（底層是舊版 .NET Framework）如果伺服器
回應的 `Content-Type` 沒有明確宣告 `charset=utf-8`，會用猜的（通常不是 UTF-8）去把
回應內容解碼成字串——這一步錯了，後面不管檔案怎麼寫都救不回來。

兩層修法都做了：

1. **API 端**（`api/src/handlers/checkin_export.rs`）明確回傳
   `Content-Type: application/json; charset=utf-8`，不再讓客戶端用猜的。
2. **`export.ps1` 端**不用 `Invoke-RestMethod`，改用 `Invoke-WebRequest` 拿
   `RawContentStream` 原始位元組，自己手動用 `[System.Text.Encoding]::UTF8` 解碼，
   完全不依賴任何一方的編碼猜測：

   ```powershell
   $WebResponse = Invoke-WebRequest -Uri $Uri -Headers $Headers -Method Get -UseBasicParsing
   $RawBytes = $WebResponse.RawContentStream.ToArray()
   $JsonText = [System.Text.Encoding]::UTF8.GetString($RawBytes)
   $Response = $JsonText | ConvertFrom-Json
   ```

   `-UseBasicParsing` 也一併加上——沒有這個參數，`Invoke-WebRequest` 在某些沒初始化
   過 IE 引擎的乾淨 Windows Server 上會直接報錯。

如果之後要換掉 API 呼叫的方式，這兩層防護（API 明確宣告 charset + script 自己手動
解碼）都要保留，不要只依賴其中一邊。

## PowerShell 5.1 讀取腳本檔案本身的編碼陷阱（跟上面是不同方向的問題）

這個踩到的地方很細，容易跟上面「輸出檔案不能有 BOM」搞混，務必分清楚：

- **輸出的 txt 檔案**（震旦雲要讀的那個）：不能有 BOM。
- **`export.ps1` 這個腳本檔案本身**：如果裡面直接寫死中文字面值（例如
  `$EventWord = '上班'`），Windows PowerShell 5.1 在**沒有 BOM 的情況下讀取 .ps1
  原始碼**時，是用系統內碼（不是 UTF-8）去解析檔案內容的——這跟輸出檔案的 BOM 規則
  完全相反的方向：腳本檔案這邊反而是「沒有 BOM 才會壞」。

實測就是在這裡踩到的：用編輯器打開下載下來的 `export.ps1`，程式碼裡原本該是
「上班」「下班」的地方變成亂碼，這代表 PowerShell 執行這支腳本時，那兩個字面值在
剛被解析出來的當下就已經是錯的，不管後面 API 或寫檔邏輯多正確都救不回來。

`export.ps1` 現在的寫法完全繞開這個問題——**不在原始碼裡直接寫中文字**，改用
Unicode 碼位組出來：

```powershell
$ClockInWord = [string]([char]0x4E0A + [char]0x73ED)   # 上班
$ClockOutWord = [string]([char]0x4E0B + [char]0x73ED)  # 下班
```

這樣腳本原始碼本身全部是純 ASCII，不管這個 `.ps1` 檔案將來被誰用什麼工具、什麼編碼
存過都不會壞。之後如果要新增其他中文字面值到這支腳本裡，**用同樣的碼位寫法，不要
直接貼中文字**。

## 已知限制

- **姓名比對，不是員工代號**：震旦雲那邊目前是用姓名比對匯入資料，同名同姓在客戶端
  是既有限制，這個整合不處理，也無法處理。
- **失敗不重試**：`export.ps1` 呼叫 API 失敗（逾時、網路中斷、伺服器錯誤）時不會在
  這次執行內重試，只會寫 log、等下一次整點的排程觸發。如果客戶機房斷線超過數小時，
  震旦雲會持續讀不到新資料，目前沒有告警機制，需要定期人工檢查 `export.log`。
