# CLI entry for a single app binary.
#   .\scripts\Invoke-AppBuild.ps1 -Bin cyberfiles -Profile debug
#   .\scripts\Invoke-AppBuild.ps1 -Bin cybereditor -Profile release

param(
    [Parameter(Mandatory)]
    [ValidateSet("cyberfiles", "cybereditor")]
    [string] $Bin,

    [ValidateSet("debug", "release")]
    [string] $Profile = "debug"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "Import-Build.ps1")
if (-not (Invoke-CyberAppBuild -Bin $Bin -Profile $Profile)) {
    exit 1
}
