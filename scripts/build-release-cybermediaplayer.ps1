$ErrorActionPreference = "Stop"
& "$PSScriptRoot\release\cybermediaplayer.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
