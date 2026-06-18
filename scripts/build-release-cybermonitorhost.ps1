$ErrorActionPreference = "Stop"
& "$PSScriptRoot\release\cybermonitorhost.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
