# Build every application binary in debug and release.
#   .\scripts\build-all.ps1

$ErrorActionPreference = "Stop"

foreach ($profile in @("debug", "release")) {
    Write-Host ""
    Write-Host "========== $profile ==========" -ForegroundColor Magenta
    & (Join-Path $PSScriptRoot "Invoke-AppBuildAll.ps1") -Profile $profile
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

Write-Host ""
Write-Host "All binaries built (debug + release)." -ForegroundColor Green
