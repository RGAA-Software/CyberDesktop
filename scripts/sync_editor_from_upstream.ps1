# Sync vendored editor crates from zed-industries/zed into crates/editor/
# Usage:
#   .\scripts\sync_editor_from_upstream.ps1 -Tier 0
#   .\scripts\sync_editor_from_upstream.ps1 -Tier 0 -ZedRoot D:\source\zed

param(
    [ValidateSet(0, 1, 2, 3)]
    [int] $Tier = 0,
    [string] $ZedRoot = "D:\source\zed",
    [string] $DestRoot = "D:\source\CyberFiles\crates\editor"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path $PSScriptRoot -Parent
if ($DestRoot -eq "D:\source\CyberFiles\crates\editor") {
    $DestRoot = Join-Path $RepoRoot "crates\editor"
}

$ManifestPath = Join-Path $DestRoot "manifest-tiers.toml"
if (-not (Test-Path $ManifestPath)) {
    throw "Missing $ManifestPath"
}

function Get-TierCrates([int] $t) {
    $content = Get-Content $ManifestPath -Raw
    $key = switch ($t) {
        0 { "tier0_foundation" }
        1 { "tier1_language" }
        2 { "tier2_editor" }
        3 { "tier3_closure" }
    }
    if ($content -notmatch "(?s)\[$key\][^\[]*?crates\s*=\s*\[(.*?)\]") {
        throw "Could not parse [$key] in manifest-tiers.toml"
    }
    $block = $Matches[1]
    [regex]::Matches($block, '"([^"]+)"') | ForEach-Object { $_.Groups[1].Value }
}

function Copy-Crate([string] $name) {
    $src = Join-Path $ZedRoot "crates\$name"
    if (-not (Test-Path $src)) {
        $srcTool = Join-Path $ZedRoot "tooling\$name"
        if (Test-Path $srcTool) { $src = $srcTool }
    }
    if (-not (Test-Path $src)) {
        throw "Upstream crate not found: $name (looked in crates/ and tooling/)"
    }
    $dst = Join-Path $DestRoot $name
    if (Test-Path $dst) {
        Remove-Item -Recurse -Force $dst
    }
    Copy-Item -Recurse $src $dst
    Write-Host "Copied $name"
}

function Get-MembersUpToTier([int] $maxTier) {
    $names = @()
    for ($t = 0; $t -le $maxTier; $t++) {
        $names += Get-TierCrates $t
    }
    $names | Sort-Object -Unique
}

function Strip-DevDependencies([string] $crateDir) {
    $cargo = Join-Path $crateDir "Cargo.toml"
    if (-not (Test-Path $cargo)) { return }
    $lines = Get-Content $cargo
    $out = New-Object System.Collections.Generic.List[string]
    $inDev = $false
    foreach ($line in $lines) {
        if ($line -match '^\[dev-dependencies\]') {
            $inDev = $true
            continue
        }
        if ($inDev) {
            if ($line -match '^\[') { $inDev = $false }
            else { continue }
        }
        if (-not $inDev) { $out.Add($line) }
    }
    if ($out.Count -gt 0) {
        Set-Content -Path $cargo -Value ($out -join "`n") -Encoding utf8
    }
}

$toCopy = Get-TierCrates $Tier
Write-Host "Tier $Tier : $($toCopy -join ', ')"

foreach ($c in $toCopy) {
    Copy-Crate $c
    Strip-DevDependencies (Join-Path $DestRoot $c)
}

# Rewrite refineable paths if present
$derivePath = Join-Path $DestRoot "refineable\derive_refineable"
if (Test-Path (Join-Path $ZedRoot "crates\refineable")) {
    # handled when tier includes refineable
}

$zedCargo = Join-Path $ZedRoot "Cargo.toml"
$depsStart = Select-String -Path $zedCargo -Pattern '^\[workspace\.dependencies\]' | Select-Object -First 1
if (-not $depsStart) { throw "No [workspace.dependencies] in zed Cargo.toml" }

$lines = Get-Content $zedCargo
$depLines = New-Object System.Collections.Generic.List[string]
$lintLines = New-Object System.Collections.Generic.List[string]
$inDeps = $false
$inLints = $false
foreach ($line in $lines) {
    if ($line -match '^\[workspace\.dependencies\]') {
        $inDeps = $true
        $inLints = $false
        $depLines.Add($line)
        continue
    }
    if ($line -match '^\[workspace\.lints') {
        $inDeps = $false
        $inLints = $true
        $lintLines.Add($line)
        continue
    }
    if ($inDeps) {
        if ($line -match '^\[' -and $line -notmatch '^\[workspace\.dependencies\.') { $inDeps = $false }
        else {
            $rewritten = $line -replace 'path = "crates/', 'path = "' -replace 'path = "tooling/', 'path = "'
            if ($rewritten -match '^gpui\s*=') {
                $rewritten = 'gpui = { git = "https://github.com/zed-industries/zed", default-features = false }'
            } elseif ($rewritten -match '^gpui_platform\s*=') {
                $rewritten = 'gpui_platform = { git = "https://github.com/zed-industries/zed", package = "gpui_platform", default-features = false }'
            } elseif ($rewritten -match '^gpui_macros\s*=') {
                $rewritten = 'gpui_macros = { git = "https://github.com/zed-industries/zed", package = "gpui_macros" }'
            } elseif ($rewritten -match '^gpui_shared_string\s*=') {
                $rewritten = 'gpui_shared_string = { git = "https://github.com/zed-industries/zed", package = "gpui_shared_string" }'
            } elseif ($rewritten -match '^http_client\s*=') {
                $rewritten = 'http_client = { git = "https://github.com/zed-industries/zed", package = "http_client" }'
            } elseif ($rewritten -match '^refineable\s*=') {
                $rewritten = 'refineable = { git = "https://github.com/zed-industries/zed", package = "refineable" }'
            }
            $depLines.Add($rewritten)
            continue
        }
    }
    if ($inLints) {
        if ($line -match '^\[' -and $line -notmatch '^\[workspace\.lints') { $inLints = $false }
        else { $lintLines.Add($line) }
    }
}

$allMembers = Get-MembersUpToTier $Tier
$memberLines = $allMembers | ForEach-Object { "    `"$_`"," }

$workspaceToml = @"
# GENERATED / maintained by scripts/sync_editor_from_upstream.ps1
# Nested workspace for vendored editor engine crates.

[workspace]
resolver = "2"
members = [
$($memberLines -join "`n")
]

[workspace.package]
edition = "2024"
version = "0.1.0"
publish = false

$($depLines -join "`n")

$($lintLines -join "`n")

"@

$workspacePath = Join-Path $DestRoot "Cargo.toml"
Set-Content -Path $workspacePath -Value $workspaceToml -Encoding utf8
Write-Host "Wrote $workspacePath ($($allMembers.Count) members, workspace.dependencies from upstream)"

Write-Host "Next: cargo check --manifest-path crates/editor/Cargo.toml -p <crate>"
