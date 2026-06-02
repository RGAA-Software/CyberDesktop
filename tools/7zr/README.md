# Bundled 7-Zip engine (`7z.dll`)

CyberFiles loads the official **64-bit** 7-Zip engine in-process next to `cyber_files.exe`
(LGPL, Igor Pavlov — https://www.7-zip.org/). Extraction uses the 7-Zip COM API with UTF-16
paths (Chinese, spaces, etc.) and multi-threaded decode (`mt` = logical CPU count).

Supported: `.7z`, `.zip`, `.tar`, `.tar.gz`, `.tar.bz2`, `.tar.xz`, and related extensions
(`.tgz`, `.tbz2`, `.txz`, `.cbz`).

`.rar` / `.cbr` use the statically linked UnRAR library (`unrar-ng`) — `7z.dll` cannot open RAR.

We do **not** spawn `7z.exe`. The thin C++ wrapper in `crates/sevenzip-ffi` calls `CreateObject`
from `7z.dll` directly (same approach as Bandizip / NanaZip).

## Refresh from upstream (26.01)

1. Download https://www.7-zip.org/a/7z2601-x64.exe
2. Extract with any 7-Zip build: `7z x 7z2601-x64.exe -otools/7zr/_tmp`
3. Copy `_tmp/7z.dll` into this directory (overwrite).

`build.rs` copies `7z.dll` into `target/{debug,release}/` on each build.

Do not use the standalone `7zr.exe` from https://www.7-zip.org/a/7zr.exe — that build is
32-bit and fails on archives larger than ~1 GB.
