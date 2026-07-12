<#
.SYNOPSIS
  Removes the hourly Task Scheduler job created by register-task.ps1.

.DESCRIPTION
  Safe to run even if the task doesn't exist (no-op with a message rather
  than an error) — useful for decommissioning this integration or before
  re-registering with different settings.

.NOTES
  Plain ASCII throughout, same reasoning as register-task.ps1 / README.md's
  PowerShell 5.1 script-source-encoding section.
#>

$ErrorActionPreference = 'Stop'

$TaskName = 'Zhengdan Checkin Export'

if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
    Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
    Write-Host "Removed scheduled task '$TaskName'."
}
else {
    Write-Host "No scheduled task named '$TaskName' found - nothing to remove."
}
