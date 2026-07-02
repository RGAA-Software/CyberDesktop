$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "..\setup\cyber_monitor\start_make.bat") @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
