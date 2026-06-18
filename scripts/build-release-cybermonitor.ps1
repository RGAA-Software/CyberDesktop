$ErrorActionPreference = "Stop"
& "$PSScriptRoot\release\cybermonitor.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
