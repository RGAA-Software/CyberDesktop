# Shared build helpers for CyberFiles application binaries.
# Dot-source from scripts under scripts/debug|release, or via scripts/Invoke-AppBuild.ps1.

$script:CyberFilesRepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$script:CyberFilesPackage = "cyberfiles"

function Get-CyberAppTargets {
    $ordered = [ordered]@{
        cyberfiles  = @{ Features = @() }
        cybereditor = @{ Features = @("zed-engine") }
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
        [ValidateSet("cyberfiles", "cybereditor")]
        [string] $Bin,

        [ValidateSet("debug", "release")]
        [string] $Profile = "debug"
    )

    $targets = [ordered]@{
        cyberfiles  = @{ Features = @() }
        cybereditor = @{ Features = @("zed-engine") }
    }
    $target = $targets[$Bin]
    if (-not $target) {
        throw "Unknown binary target: $Bin"
    }

    $cargoArgs = @("build", "-p", $script:CyberFilesPackage, "--bin", $Bin)
    if ($Profile -eq "release") {
        $cargoArgs += "--release"
    }
    if ($target.Features.Count -gt 0) {
        $cargoArgs += @("--features", ($target.Features -join ","))
    }

    Push-Location $script:CyberFilesRepoRoot
    try {
        Write-Host "cargo $($cargoArgs -join ' ')" -ForegroundColor Cyan
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            return $false
        }

        $outDir = if ($Profile -eq "release") { "release" } else { "debug" }
        $exe = Join-Path $script:CyberFilesRepoRoot "target\$outDir\$Bin.exe"
        if (Test-Path $exe) {
            Write-Host "OK: $exe" -ForegroundColor Green
        }
        return $true
    }
    finally {
        Pop-Location
    }
}
