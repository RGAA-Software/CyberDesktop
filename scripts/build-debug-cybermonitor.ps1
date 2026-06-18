$ErrorActionPreference = "Stop"
& "$PSScriptRoot\debug\cybermonitor.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
