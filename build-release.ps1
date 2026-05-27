$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

cargo build --workspace --bins --release
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
