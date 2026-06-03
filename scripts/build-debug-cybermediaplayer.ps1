$ErrorActionPreference = "Stop"
& "$PSScriptRoot\debug\cybermediaplayer.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
