$ErrorActionPreference = "Stop"
& "$PSScriptRoot\debug\cybermonitorhost.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
