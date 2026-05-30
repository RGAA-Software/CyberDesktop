# Shared build helpers for CyberDesktop application binaries.
# Dot-source from scripts under scripts/debug|release, or via scripts/Invoke-AppBuild.ps1.

$script:CyberDesktopRepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$script:CyberDesktopPackage = "cyber-desktop"

function Get-CyberAppTargets {
    $ordered = [ordered]@{
        cyber_files  = @{ Features = @() }
        cyber_editor = @{ Features = @() }
    }
    foreach ($key in $ordered.Keys) {
        [PSCustomObject]@{
            Key      = $key
            Features = $ordered[$key].Features
        }
    }
}

function Invoke-CyberAppBuild {
    param(
        [Parameter(Mandatory)]
        [ValidateSet("cyber_files", "cyber_editor")]
        [string] $Bin,

        [ValidateSet("debug", "release")]
        [string] $Profile = "debug"
    )

    $targets = [ordered]@{
        cyber_files  = @{ Features = @() }
        cyber_editor = @{ Features = @() }
    }
    $target = $targets[$Bin]
    if (-not $target) {
        throw "Unknown binary target: $Bin"
    }

    $cargoArgs = @("build", "-p", $script:CyberDesktopPackage, "--bin", $Bin)
    if ($Profile -eq "release") {
        $cargoArgs += "--release"
    }
    if ($target.Features.Count -gt 0) {
        $cargoArgs += @("--features", ($target.Features -join ","))
    }

    Push-Location $script:CyberDesktopRepoRoot
    try {
        Write-Host "cargo $($cargoArgs -join ' ')" -ForegroundColor Cyan
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            return $false
        }

        $outDir = if ($Profile -eq "release") { "release" } else { "debug" }
        $exe = Join-Path $script:CyberDesktopRepoRoot "target\$outDir\$Bin.exe"
        if (Test-Path $exe) {
            Write-Host "OK: $exe" -ForegroundColor Green
        }
        return $true
    }
    finally {
        Pop-Location
    }
}
