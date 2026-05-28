$ErrorActionPreference = "Stop"
& "$PSScriptRoot\release\cybereditor.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
