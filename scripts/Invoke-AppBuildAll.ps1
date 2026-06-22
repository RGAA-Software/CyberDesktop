# Build every application binary.
#   .\scripts\Invoke-AppBuildAll.ps1 -Profile debug
#   .\scripts\Invoke-AppBuildAll.ps1 -Profile release
#
# Targets: cyber_files, cyber_editor, cyber_media_player, cyber_monitor, cyber_monitor_host

param(
    [ValidateSet("debug", "release")]
    [string] $Profile = "debug"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "Import-Build.ps1")

$failed = $false
foreach ($target in Get-CyberAppTargets) {
    Write-Host ""
    Write-Host "=== $($target.Key) ($Profile) ===" -ForegroundColor Yellow
    if (-not (Invoke-CyberAppBuild -Bin $target.Key -Profile $Profile)) {
        $failed = $true
    }
}

if ($failed) {
    exit 1
}

Write-Host ""
Write-Host "All application targets built ($Profile)." -ForegroundColor Green
