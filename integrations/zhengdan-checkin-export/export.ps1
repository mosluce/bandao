<#
.SYNOPSIS
  Pulls today's (+08:00) clock-in/clock-out events from the 班到 API and
  writes them as a fixed-width text file for 震旦雲 to import.

.DESCRIPTION
  Calls GET {ApiBaseUrl}/orgs/me/checkin/events/export?utc_offset=+08:00
  with the API token from config.ps1, renders the returned JSON events into
  the Zhengdan fixed-width row format (name padded to 20 characters,
  YYYYMMDDHHmmss, 上班/下班 with no separators), and writes the result as a
  UTF-8-without-BOM file named after the run's local timestamp
  (yyyyMMddHHmmss.txt) into TargetFolder.

  Intended to run hourly via Windows Task Scheduler. On ANY failure (HTTP
  error, timeout, network error, or an unexpected exception while
  formatting) this script writes NOTHING to TargetFolder and only appends
  to export.log — a bad run must never leave a partial/corrupt file for
  震旦雲 to misread as "no one clocked in today".

  A successful call that legitimately has zero events for the day DOES
  still write an (empty) file — that's a different signal from "the
  export itself failed" and 震旦雲 should be able to tell them apart.

.NOTES
  Written for Windows Server 2016 Datacenter's built-in PowerShell 5.1.
  Do NOT swap the file-write step for `Out-File -Encoding utf8` or
  `Set-Content -Encoding UTF8` — both silently prepend a UTF-8 BOM on
  PowerShell 5.1, which 震旦雲's importer may not tolerate. See README.md.
#>

$ErrorActionPreference = 'Stop'

# Built from Unicode code points, not literal characters, on purpose: Windows
# PowerShell 5.1 reads a BOM-less UTF-8 .ps1 file using the system codepage,
# which would corrupt any non-ASCII literal written directly in this source
# file (independent of, and in addition to, the HTTP response encoding issue
# handled further down). Code points make this script's correctness
# independent of how the .ps1 file itself happens to be saved/transferred.
# 上 U+4E0A, 班 U+73ED, 下 U+4E0B.
$ClockInWord = [string]([char]0x4E0A + [char]0x73ED)   # 上班
$ClockOutWord = [string]([char]0x4E0B + [char]0x73ED)  # 下班

# Windows Server 2016's .NET Framework defaults to SSL3/TLS1.0, which most
# modern HTTPS endpoints (including the production API) refuse — the call
# below fails with "無法建立 SSL/TLS 的安全通道" until TLS 1.2 is forced on.
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ConfigPath = Join-Path $ScriptDir 'config.ps1'
$LogPath = Join-Path $ScriptDir 'export.log'

function Write-Log {
    param([string]$Message)
    $Timestamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
    Add-Content -Path $LogPath -Value "[$Timestamp] $Message"
}

try {
    if (-not (Test-Path $ConfigPath)) {
        Write-Log "ERROR: config file not found at $ConfigPath (copy config.example.ps1 and fill it in)"
        exit 1
    }

    # Defines $ApiBaseUrl, $ApiToken, $TargetFolder.
    . $ConfigPath

    if (-not $ApiBaseUrl -or -not $ApiToken -or -not $TargetFolder) {
        Write-Log 'ERROR: config.ps1 must set $ApiBaseUrl, $ApiToken, and $TargetFolder'
        exit 1
    }
    if (-not (Test-Path $TargetFolder)) {
        Write-Log "ERROR: TargetFolder does not exist: $TargetFolder"
        exit 1
    }

    # +08:00 is hardcoded on purpose — this script is the customer-specific
    # client of a generic export API, not a generic tool. See design.md D3
    # in the add-zhengdan-checkin-export OpenSpec change.
    $Uri = "$ApiBaseUrl/orgs/me/checkin/events/export?utc_offset=%2B08:00"
    $Headers = @{ Authorization = "Bearer $ApiToken" }

    # Deliberately NOT Invoke-RestMethod: on PowerShell 5.1 it decodes the
    # response body using a guessed (non-UTF-8) encoding whenever the
    # server's Content-Type doesn't carry an explicit charset, silently
    # corrupting multi-byte display names before we ever see them. The API
    # now declares charset=utf-8 explicitly (see api/src/handlers/checkin_export.rs),
    # but this script doesn't rely on that alone — it reads the raw response
    # bytes and decodes them as UTF-8 itself, so it's correct regardless of
    # what any given server (or a future misconfiguration) sends.
    # -UseBasicParsing avoids a hard dependency on the IE engine, which is
    # often not initialized on a fresh Windows Server install.
    $WebResponse = Invoke-WebRequest -Uri $Uri -Headers $Headers -Method Get -TimeoutSec 30 -UseBasicParsing
    $RawBytes = $WebResponse.RawContentStream.ToArray()
    $JsonText = [System.Text.Encoding]::UTF8.GetString($RawBytes)
    $Response = $JsonText | ConvertFrom-Json

    # Wrapped in @(...): PowerShell assigns $null (not an empty array) to a
    # variable captured from a foreach loop that emits zero items, which
    # would blow up [string]::Join below. @(...) forces array context so
    # $Lines is always a real (possibly empty) array.
    $Lines = @(foreach ($evt in $Response.events) {
        $EventWord = $null
        if ($evt.event_type -eq 'clock_in') { $EventWord = $ClockInWord }
        elseif ($evt.event_type -eq 'clock_out') { $EventWord = $ClockOutWord }
        else { continue }  # defensive: the API only ever returns these two

        $NamePadded = $evt.app_user_display_name.PadRight(20)
        $LocalTime = [DateTimeOffset]::Parse($evt.occurred_at_client).ToOffset([TimeSpan]::FromHours(8))
        $TimeStamp = $LocalTime.ToString('yyyyMMddHHmmss')
        "$NamePadded$TimeStamp$EventWord"
    })

    $Content = [string]::Join("`r`n", $Lines)

    $RunTimestamp = Get-Date -Format 'yyyyMMddHHmmss'
    $OutFile = Join-Path $TargetFolder "$RunTimestamp.txt"

    # BOM-safe UTF-8 write — see .NOTES above.
    $Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($OutFile, $Content, $Utf8NoBom)

    Write-Log "OK: wrote $($Lines.Count) row(s) to $OutFile"
}
catch {
    Write-Log "ERROR: $($_.Exception.Message)"
    exit 1
}
