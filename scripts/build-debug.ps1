# Build every application binary (debug).
#   .\scripts\build-debug.ps1
#
# Application binaries built:
#   cyber_files, cyber_editor, cyber_media_player, cyber_monitor, cyber_monitor_host
#
# Single target:
#   .\scripts\build-debug-cyberfiles.ps1
#   .\scripts\build-debug-cybereditor.ps1
#   .\scripts\build-debug-cybermediaplayer.ps1
#   .\scripts\build-debug-cybermonitor.ps1
#   .\scripts\build-debug-cybermonitorhost.ps1
# Or: .\scripts\debug\all.ps1 | cyberfiles.ps1 | ...

$ErrorActionPreference = "Stop"

$targets = @("cyber_files", "cyber_editor", "cyber_media_player", "cyber_monitor", "cyber_monitor_host")
Write-Host "Building all application binaries (debug): $($targets -join ', ')" -ForegroundColor Cyan

& (Join-Path $PSScriptRoot "Invoke-AppBuildAll.ps1") -Profile debug
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
