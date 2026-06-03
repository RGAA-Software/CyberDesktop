# CLI entry for a single app binary.
#   .\scripts\Invoke-AppBuild.ps1 -Bin cyber_files -Profile debug
#   .\scripts\Invoke-AppBuild.ps1 -Bin cyber_editor -Profile release

param(
    [Parameter(Mandatory)]
    [ValidateSet("cyber_files", "cyber_editor", "cyber_media_player")]
    [string] $Bin,

    [ValidateSet("debug", "release")]
    [string] $Profile = "debug"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "Import-Build.ps1")
if (-not (Invoke-CyberAppBuild -Bin $Bin -Profile $Profile)) {
    exit 1
}
