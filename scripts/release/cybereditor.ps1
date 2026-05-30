$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "..\Import-Build.ps1")
if (-not (Invoke-CyberAppBuild -Bin cyber_editor -Profile release)) {
    exit 1
}
