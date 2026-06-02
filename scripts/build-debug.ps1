# Build all application binaries (debug): cyber_files + cyber_editor.
#   .\scripts\build-debug.ps1
# Single target:
#   .\scripts\build-debug-cyberfiles.ps1
#   .\scripts\build-debug-cybereditor.ps1
# Or: .\scripts\debug\all.ps1 | cyberfiles.ps1 | cybereditor.ps1

$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "Invoke-AppBuildAll.ps1") -Profile debug
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
