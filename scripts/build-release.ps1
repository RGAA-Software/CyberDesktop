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
#   .\scripts\build-release.ps1 -Bin cyberfiles
#   .\scripts\build-release.ps1 -NoZedEngine

param(
    [string] $Package = "cyberfiles",
    [ValidateSet("cybereditor", "cyberfiles")]
    [string] $Bin = "cybereditor",
    [switch] $NoZedEngine
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $RepoRoot

$args = @("build", "--release", "-p", $Package, "--bin", $Bin)
if ($Bin -eq "cybereditor" -and $NoZedEngine) {
    Write-Warning "zed-engine was removed; -NoZedEngine has no effect."
}

Write-Host "cargo $($args -join ' ')" -ForegroundColor Cyan
& cargo @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Join-Path $RepoRoot "target\release\$Bin.exe"
if (Test-Path $exe) {
    Write-Host "OK: $exe" -ForegroundColor Green
}
