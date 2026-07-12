<#
.SYNOPSIS
  Registers the hourly Task Scheduler job that runs export.ps1.

.DESCRIPTION
  Idempotent: if a task with the same name already exists, it's
  unregistered and re-created rather than erroring out, so this is safe
  to re-run after moving the script or changing the settings below.

  Runs export.ps1 (expected in the same directory as this script) every
  hour starting immediately, by default as SYSTEM — no password to
  manage, keeps running whether or not anyone is logged in. If export.ps1
  ever needs to reach something that requires an interactive user's
  credentials (e.g. a mapped network drive), set $RunAsSystem to $false
  below; this script will then prompt for a password instead.

.NOTES
  Every string in this file is plain ASCII on purpose — see README.md's
  PowerShell 5.1 script-source-encoding section for why non-ASCII
  literals in a .ps1 file are risky on this platform. This script has no
  Chinese text to begin with, so there's nothing to guard here, but keep
  it that way in future edits.
#>

$ErrorActionPreference = 'Stop'

$TaskName = 'Zhengdan Checkin Export'
$RunAsSystem = $true   # set $false to run as an interactive user instead (prompts for a password)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ScriptPath = Join-Path $ScriptDir 'export.ps1'

if (-not (Test-Path $ScriptPath)) {
    Write-Error "export.ps1 not found next to this script at $ScriptPath"
    exit 1
}

$Action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument "-ExecutionPolicy Bypass -File `"$ScriptPath`""

# ScheduledTasks has no built-in "hourly" trigger shape — the standard
# idiom is a single trigger that repeats forever.
$Trigger = New-ScheduledTaskTrigger -Once -At (Get-Date) `
    -RepetitionInterval (New-TimeSpan -Hours 1) `
    -RepetitionDuration ([TimeSpan]::MaxValue)

$Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBattery -DontStopIfGoingOnBatteries -StartWhenAvailable

if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
    Write-Host "Task '$TaskName' already exists - replacing it."
    Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
}

if ($RunAsSystem) {
    $Principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
    Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger `
        -Settings $Settings -Principal $Principal `
        -Description 'Hourly Zhengdan checkin export' | Out-Null
}
else {
    $Credential = Get-Credential -Message "Credentials to run '$TaskName' as (this password does not auto-renew if it changes later)"
    Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger `
        -Settings $Settings -User $Credential.UserName `
        -Password $Credential.GetNetworkCredential().Password -RunLevel Highest `
        -Description 'Hourly Zhengdan checkin export' | Out-Null
}

Write-Host "Registered '$TaskName'. Triggering a test run now..."
Start-ScheduledTask -TaskName $TaskName
Start-Sleep -Seconds 5
Get-ScheduledTaskInfo -TaskName $TaskName | Format-List TaskName, LastRunTime, LastTaskResult, NextRunTime
Write-Host 'Check export.log next to export.ps1 for the result.'
