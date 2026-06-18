# Shared build helpers for CyberDesktop application binaries.
# Dot-source from scripts under scripts/debug|release, or via scripts/Invoke-AppBuild.ps1.

$script:CyberDesktopRepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Get-CyberAppTargets {
    $ordered = [ordered]@{
        cyber_files       = @{ Package = "files-app"; Features = @() }
        cyber_editor      = @{ Package = "editor-app"; Features = @() }
        cyber_media_player = @{ Package = "media-player-app"; Features = @() }
        cyber_monitor      = @{ Package = "monitor-app"; Features = @() }
        cyber_monitor_host = @{ Package = "monitor-app"; Features = @() }
    }
    foreach ($key in $ordered.Keys) {
        [PSCustomObject]@{
            Key      = $key
            Package  = $ordered[$key].Package
            Features = $ordered[$key].Features
        }
    }
}

function Invoke-CyberAppBuild {
    param(
        [Parameter(Mandatory)]
        [ValidateSet("cyber_files", "cyber_editor", "cyber_media_player", "cyber_monitor", "cyber_monitor_host")]
        [string] $Bin,

        [ValidateSet("debug", "release")]
        [string] $Profile = "debug"
    )

    $targets = [ordered]@{
        cyber_files        = @{ Package = "files-app"; Features = @() }
        cyber_editor       = @{ Package = "editor-app"; Features = @() }
        cyber_media_player = @{ Package = "media-player-app"; Features = @() }
        cyber_monitor      = @{ Package = "monitor-app"; Features = @() }
        cyber_monitor_host = @{ Package = "monitor-app"; Features = @() }
    }
    $target = $targets[$Bin]
    if (-not $target) {
        throw "Unknown binary target: $Bin"
    }

    $cargoArgs = @("build", "-p", $target.Package, "--bin", $Bin)
    $previousRustflags = $env:RUSTFLAGS
    if ($Profile -eq "release") {
        $cargoArgs += "--release"
        $nativeFlag = "-C target-cpu=native"
        $env:RUSTFLAGS = if ($previousRustflags) { "$previousRustflags $nativeFlag" } else { $nativeFlag }
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
        $env:RUSTFLAGS = $previousRustflags
        Pop-Location
    }
}
