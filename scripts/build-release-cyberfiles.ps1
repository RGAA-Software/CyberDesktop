$ErrorActionPreference = "Stop"
& "$PSScriptRoot\release\cyberfiles.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
