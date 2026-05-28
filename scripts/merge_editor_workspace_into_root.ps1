# Merge crates/editor nested workspace into the CyberFiles root Cargo.toml.
# Run from repo root: .\scripts\merge_editor_workspace_into_root.ps1

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path $PSScriptRoot -Parent
$RootCargo = Join-Path $RepoRoot "Cargo.toml"
$EditorCargo = Join-Path $RepoRoot "crates\editor\Cargo.toml"

if (-not (Test-Path $EditorCargo)) {
    throw "Missing $EditorCargo — run sync_editor_from_upstream.ps1 first."
}

$editorToml = Get-Content $EditorCargo -Raw
$rootToml = Get-Content $RootCargo -Raw

# --- members ---
if ($editorToml -notmatch '(?s)\[workspace\]\s*resolver\s*=\s*"2"\s*members\s*=\s*\[(.*?)\]') {
    throw "Could not parse editor workspace members"
}
$memberBlock = $Matches[1]
$editorMembers = [regex]::Matches($memberBlock, '"([^"]+)"') | ForEach-Object { $_.Groups[1].Value }
$prefixedMembers = $editorMembers | ForEach-Object { "crates/editor/$_" }
$prefixedMembers += "crates/cyber-editor-engine"

# --- workspace.dependencies (main block, before subtables like windows) ---
if ($editorToml -notmatch '(?s)\[workspace\.dependencies\]\s*\n(.*?)(?=\n\[workspace\.dependencies\.)') {
    if ($editorToml -notmatch '(?s)\[workspace\.dependencies\]\s*\n(.*?)(?=\n\[workspace\.lints)') {
        throw "Could not parse editor [workspace.dependencies]"
    }
}
$depsBlock = $Matches[1]
$depsBlock = $depsBlock -replace 'path = "([^"]+)"', 'path = "crates/editor/$1"'

# Keys already defined at root — keep root versions (gpui git pin, cyberfiles crates).
$skipDeps = @(
    'gpui', 'gpui_platform', 'gpui-component', 'gpui-component-assets',
    'anyhow', 'chrono', 'serde', 'serde_json', 'sys-locale', 'unicode-width',
    'cyberfiles-assets', 'cyberfiles-commands', 'cyberfiles-core',
    'cyberfiles-fs', 'cyberfiles-platform-windows', 'cyberfiles-ui',
    'embed-resource'
)

function Filter-DepsBlock([string] $block, [string[]] $skip) {
    $lines = $block -split "`n"
    $out = New-Object System.Collections.Generic.List[string]
    $skipLine = $false
    foreach ($line in $lines) {
        if ($line -match '^([a-zA-Z0-9_-]+)\s*=') {
            $key = $Matches[1]
            if ($skip -contains $key) {
                $skipLine = $true
                continue
            }
            $skipLine = $false
        }
        if ($skipLine) { continue }
        $out.Add($line)
    }
    $out -join "`n"
}

$depsBlock = Filter-DepsBlock $depsBlock $skipDeps

# --- workspace.dependencies subtables (e.g. windows) ---
$depSubtables = ""
foreach ($m in [regex]::Matches($editorToml, '(?ms)(\[workspace\.dependencies\.[^\]]+\][^\[]*)')) {
    $depSubtables += "`n$($m.Groups[1].Value.TrimEnd())`n"
}

# --- workspace.lints ---
$lintsBlock = ""
if ($editorToml -match '(?s)(\[workspace\.lints\.[^\]]+\][^\[]*)') {
    $lintsBlock = $Matches[1].TrimEnd()
}

# --- patch root Cargo.toml ---
if ($rootToml -notmatch '(?s)(\[workspace\]\s*members\s*=\s*\[)(.*?)(\])') {
    throw "Could not parse root [workspace] members"
}
$existingMembers = [regex]::Matches($Matches[2], '"([^"]+)"') | ForEach-Object { $_.Groups[1].Value }
$allMembers = ($existingMembers + $prefixedMembers) | Sort-Object -Unique
$memberLines = ($allMembers | ForEach-Object { "    `"$_`"," }) -join "`n"
$memberLines = $memberLines.TrimEnd(',')

$newMembersSection = @"
[workspace]
members = [
$memberLines
]
default-members = ["crates/app"]
resolver = "2"
"@

# workspace.package — edition 2024 for vendored editor crates
$newPackageSection = @"
[workspace.package]
version = "0.1.0"
edition = "2024"
publish = false
"@

# Keep root cyberfiles deps, append editor deps
if ($rootToml -notmatch '(?s)\[workspace\.dependencies\]\s*\n(.*?)($|\[workspace\.package\]|\[workspace\.lints\]|\[package\])') {
    throw "Could not parse root [workspace.dependencies]"
}
$rootDeps = $Matches[1].TrimEnd()

$newDepsSection = @"
[workspace.dependencies]
$rootDeps

# --- vendored editor engine (from crates/editor/Cargo.toml) ---
$depsBlock
$depSubtables
"@

# cyber-editor-engine workspace alias
if ($newDepsSection -notmatch 'cyber-editor-engine\s*=') {
    $newDepsSection += "`ncyber-editor-engine = { path = `"crates/cyber-editor-engine`" }`n"
}

$tail = ""
if ($lintsBlock) {
    $tail = "`n$lintsBlock`n"
}

# Rebuild root file: preserve nothing after workspace deps except we drop duplicate sections
$header = @"
$newMembersSection

$newPackageSection

$newDepsSection
$tail
"@

Set-Content -Path $RootCargo -Value $header.TrimEnd() -NoNewline
Add-Content -Path $RootCargo -Value "`n"

Write-Host "Merged $($editorMembers.Count) editor members and editor workspace.dependencies into $RootCargo"
Write-Host "Set workspace.package.edition = 2024"
