# Build all application binaries (release): cyber_files + cyber_editor.
#   .\scripts\build-release.ps1
# Single target:
#   .\scripts\build-release-cyberfiles.ps1
#   .\scripts\build-release-cybereditor.ps1
# Or: .\scripts\release\all.ps1 | cyberfiles.ps1 | cybereditor.ps1

$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "Invoke-AppBuildAll.ps1") -Profile release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
