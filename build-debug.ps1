$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

cargo build --workspace --bins
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
