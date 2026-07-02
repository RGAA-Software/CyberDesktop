# Build every application binary (release).
#   .\scripts\build-release.ps1
#
# Application binaries built:
#   cyber_files, cyber_editor, cyber_media_player, cyber_monitor, cyber_monitor_host
#
# Single target:
#   .\scripts\build-release-cyberfiles.ps1
#   .\scripts\build-release-cybereditor.ps1
#   .\scripts\build-release-cybermediaplayer.ps1
#   .\scripts\build-release-cybermonitor.ps1
#   .\scripts\build-release-cybermonitorhost.ps1
# Installer (release build + NSIS package):
#   .\scripts\build-setup-cybermonitor.ps1
# Or: .\scripts\release\all.ps1 | cyberfiles.ps1 | ...

$ErrorActionPreference = "Stop"

$targets = @("cyber_files", "cyber_editor", "cyber_media_player", "cyber_monitor", "cyber_monitor_host")
Write-Host "Building all application binaries (release): $($targets -join ', ')" -ForegroundColor Cyan

& (Join-Path $PSScriptRoot "Invoke-AppBuildAll.ps1") -Profile release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
