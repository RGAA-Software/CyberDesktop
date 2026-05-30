# Build all application binaries (debug).
#   .\scripts\build-debug.ps1
# Per target:
#   .\scripts\build-debug-cyberfiles.ps1
#   .\scripts\build-debug-cybereditor.ps1
# Or: .\scripts\debug\all.ps1 | cyberfiles.ps1 | cybereditor.ps1

$ErrorActionPreference = "Stop"
& "$PSScriptRoot\debug\all.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
# Build CyberEditor (debug).
# Usage:
#   .\scripts\build-debug.ps1
#   .\scripts\build-debug.ps1 -Bin cyber_files
#   .\scripts\build-debug.ps1 -NoZedEngine

param(
    [string] $Package = "cyber-desktop",
    [ValidateSet("cyber_editor", "cyber_files")]
    [string] $Bin = "cyber_editor",
    [switch] $NoZedEngine
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $RepoRoot

$args = @("build", "-p", $Package, "--bin", $Bin)
if ($Bin -eq "cyber_editor" -and $NoZedEngine) {
    Write-Warning "zed-engine was removed; -NoZedEngine has no effect."
}

Write-Host "cargo $($args -join ' ')" -ForegroundColor Cyan
& cargo @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Join-Path $RepoRoot "target\debug\$Bin.exe"
if (Test-Path $exe) {
    Write-Host "OK: $exe" -ForegroundColor Green
}
