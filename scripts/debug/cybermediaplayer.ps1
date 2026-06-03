$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\Import-Build.ps1")
if (-not (Invoke-CyberAppBuild -Bin cyber_media_player -Profile debug)) {
    exit 1
}
