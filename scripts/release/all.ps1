$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "..\Invoke-AppBuildAll.ps1") -Profile release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
