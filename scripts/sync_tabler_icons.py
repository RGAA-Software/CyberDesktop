#!/usr/bin/env python3
"""Download Tabler Icons (outline, 24px) into crates/app-assets/assets/icons/tabler/.

Source: https://tabler.io/icons (MIT) via jsDelivr @tabler/icons package.
"""

from __future__ import annotations

import re
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = REPO_ROOT / "crates" / "app-assets" / "assets" / "icons" / "tabler"
TABLER_VERSION = "3.44.0"
CDN_BASE = f"https://cdn.jsdelivr.net/npm/@tabler/icons@{TABLER_VERSION}/icons/outline"

# Local filename (without .svg) -> Tabler icon slug
TABLER_ICON_MAP: dict[str, str] = {
    # App chrome
    "files": "files",
    "home": "home",
    "folder": "folder",
    "folder-filled": "folder",
    "plus": "plus",
    "x": "x",
    "moon": "moon",
    "sun": "sun",
    "settings": "settings",
    "brand-github": "brand-github",
    "bell": "bell",
    "minus": "minus",
    "square": "square",
    # Navigation
    "arrow-left": "arrow-left",
    "arrow-right": "arrow-right",
    "arrow-up": "arrow-up",
    "arrow-back-up": "arrow-back-up",
    "refresh": "refresh",
    "chevron-right": "chevron-right",
    "chevron-down": "chevron-down",
    "chevron-up": "chevron-up",
    "external-link": "external-link",
    # Layout / panes
    "layout-columns": "layout-columns",
    "layout-sidebar-right": "layout-sidebar-right",
    "layout-sidebar-right-collapse": "layout-sidebar-right-collapse",
    "pin": "pin",
    "pinned": "pinned",
    # File commands
    "copy": "copy",
    "cut": "cut",
    "clipboard": "clipboard",
    "pencil": "pencil",
    "trash": "trash",
    "folder-plus": "folder-plus",
    "folder-pin": "folder-pin",
    "file-plus": "file-plus",
    "folder-open": "folder-open",
    "file-zip": "file-zip",
    "dots": "dots",
    # View modes
    "list-details": "list-details",
    "list": "list",
    "layout-grid": "layout-grid",
    "layout-board": "layout-board",
    "columns-3": "columns-3",
    # Misc UI
    "info-circle": "info-circle",
    "tag": "tag",
    "star": "star",
    "star-off": "star-off",
    "sort-ascending": "sort-ascending",
    "sort-descending": "sort-descending",
    "arrows-sort": "arrows-sort",
    "eye": "eye",
    "eye-off": "eye-off",
    "device-desktop": "device-desktop",
    "download": "download",
    "music": "music",
    "network": "network",
    "cloud": "cloud",
    "clock": "clock",
    "history": "history",
    "calendar": "calendar",
    "search": "search",
    "terminal-2": "terminal-2",
    "inbox": "inbox",
    "sort-ascending-letters": "sort-ascending-letters",
    # File types
    "file": "file",
    "file-text": "file-text",
    "file-code": "file-code",
    "file-type-pdf": "file-type-pdf",
    "file-type-html": "file-type-html",
    "file-type-ts": "file-type-ts",
    "file-type-js": "file-type-js",
    "file-type-cpp": "brand-cpp",
    "movie": "movie",
    "photo": "photo",
    "book": "book",
    "folder-off": "folder-off",
    "link": "link",
    "brand-windows": "brand-windows",
    "database": "database",
    "server": "server",
    "widget": "apps",
}


def normalize_svg(raw: str) -> str:
    svg = raw.strip()
    if 'stroke="currentColor"' not in svg:
        svg = svg.replace("<svg ", '<svg stroke="currentColor" ', 1)
    svg = re.sub(r'\sclass="[^"]*"', "", svg)
    svg = re.sub(r"<path stroke=\"none\" d=\"M0 0h24v24H0z\" fill=\"none\" />\s*", "", svg)
    if not svg.startswith("<?xml"):
        svg = '<?xml version="1.0" encoding="UTF-8"?>\n' + svg
    return svg.rstrip() + "\n"


def download_icon(slug: str) -> str:
    url = f"{CDN_BASE}/{slug}.svg"
    req = urllib.request.Request(url, headers={"User-Agent": "CyberFiles/sync_tabler_icons"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        return normalize_svg(resp.read().decode("utf-8"))


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    missing: list[str] = []
    for local_name, slug in sorted(TABLER_ICON_MAP.items()):
        dest = OUT_DIR / f"{local_name}.svg"
        if dest.is_file():
            continue
        try:
            content = download_icon(slug)
        except urllib.error.HTTPError as err:
            missing.append(f"{local_name} ({slug}): HTTP {err.code}")
            continue
        except OSError as err:
            missing.append(f"{local_name} ({slug}): {err}")
            continue
        (OUT_DIR / f"{local_name}.svg").write_text(content, encoding="utf-8", newline="\n")

    if missing:
        print("Failed downloads:")
        for line in missing:
            print(f"  - {line}")
        raise SystemExit(1)

    print(f"Synced {len(TABLER_ICON_MAP)} Tabler icons -> {OUT_DIR}")


if __name__ == "__main__":
    main()
