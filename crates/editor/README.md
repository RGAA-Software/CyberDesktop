# Editor engine (vendored from Zed)

This directory contains a **vendored subset** of [zed-industries/zed](https://github.com/zed-industries/zed) crates used as CyberEditor’s **text engine** (large-file performance). The folder is named **`editor`**; it is not the Zed application.

## Layout

```text
crates/editor/
  Cargo.toml           # nested Cargo workspace for vendored crates
  manifest-tiers.toml  # which crates to sync per phase
  collections/         # package name = directory name
  text/
  rope/
  editor/              # Zed’s `editor` package (view widget)
  ...
```

## Sync from upstream

```powershell
# Default upstream: D:\source\zed (override with -ZedRoot)
.\scripts\sync_editor_from_upstream.ps1 -Tier 0
.\scripts\sync_editor_from_upstream.ps1 -Tier 1
# …
```

Record the pinned commit in `UPSTREAM`.

## Build (engine workspace only)

```powershell
cargo check --manifest-path crates/editor/Cargo.toml -p text
cargo check --manifest-path crates/editor/Cargo.toml -p editor
```

## Build CyberEditor app

```powershell
cargo build -p cyberfiles --bin cybereditor
```

## License

GPL-3.0-or-later applies to GPL-marked crates in this tree. See repo `NOTICE` when added.
