$ErrorActionPreference = "Stop"
& "$PSScriptRoot\debug\cybereditor.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
