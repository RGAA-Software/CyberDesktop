$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "..\Invoke-AppBuildAll.ps1") -Profile debug
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
