$ErrorActionPreference = "Stop"
& "$PSScriptRoot\debug\cyberfiles.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
