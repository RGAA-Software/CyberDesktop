# Build all application binaries (release).
#   .\scripts\build-release.ps1
# Per target:
#   .\scripts\build-release-cyberfiles.ps1
#   .\scripts\build-release-cybereditor.ps1
# Or: .\scripts\release\all.ps1 | cyberfiles.ps1 | cybereditor.ps1

$ErrorActionPreference = "Stop"
& "$PSScriptRoot\release\all.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
# Build CyberEditor (release).
# Usage:
#   .\scripts\build-release.ps1
#   .\scripts\build-release.ps1 -Bin cyber_files
#   .\scripts\build-release.ps1 -NoZedEngine

param(
    [string] $Package = "cyber-desktop",
    [ValidateSet("cyber_editor", "cyber_files")]
    [string] $Bin = "cyber_editor",
    [switch] $NoZedEngine
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $RepoRoot

$args = @("build", "--release", "-p", $Package, "--bin", $Bin)
if ($Bin -eq "cyber_editor" -and $NoZedEngine) {
    Write-Warning "zed-engine was removed; -NoZedEngine has no effect."
}

Write-Host "cargo $($args -join ' ')" -ForegroundColor Cyan
& cargo @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Join-Path $RepoRoot "target\release\$Bin.exe"
if (Test-Path $exe) {
    Write-Host "OK: $exe" -ForegroundColor Green
}
