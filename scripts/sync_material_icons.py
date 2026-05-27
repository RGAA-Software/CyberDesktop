#!/usr/bin/env python3
"""Download Google Material Symbols (Rounded, default 24px) into gpui IconName SVG paths.

Source: https://fonts.google.com/icons?icon.style=Rounded
"""

from __future__ import annotations

import re
import shutil
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
OUT_ICONS = REPO_ROOT / "crates" / "assets" / "assets" / "icons"

# gpui IconName (kebab-case filename) -> Material Symbols icon id (underscores)
MATERIAL_ICON_MAP: dict[str, str] = {
    "arrow-left": "arrow_back",
    "arrow-right": "arrow_forward",
    "arrow-up": "arrow_upward",
    "redo-2": "refresh",
    "panel-left-open": "dock_to_left",
    "panel-left-close": "left_panel_close",
    "panel-right-open": "dock_to_right",
    "panel-right-close": "right_panel_close",
    "panel-left": "dock_to_left",
    "layout-dashboard": "dashboard",
    "pin": "push_pin",
    "plus": "add",
    "folder": "folder",
    "file": "description",
    "gallery-vertical-end": "bookmark",
    "delete": "delete",
    "chevron-right": "chevron_right",
    "chevron-down": "expand_more",
    "external-link": "open_in_new",
    "settings-2": "settings",
    "inbox": "label",
    "info": "info",
    "bell": "notifications",
    "hard-drive": "hard_drive",
    "globe": "public",
    "calendar": "calendar_today",
    "content_cut": "content_cut",
    "content_paste": "content_paste",
    "folder_zip": "folder_zip",
    "label": "label",
    "label_off": "label_off",
    "widgets": "widgets",
    "tab": "tab",
    "splitscreen": "splitscreen",
    "create_new_folder": "create_new_folder",
    "note_add": "note_add",
    "content_copy": "content_copy",
    "drive_file_rename_outline": "drive_file_rename_outline",
}

LUCIDE_PRESERVE: tuple[str, ...] = (
    "window-close",
    "window-minimize",
    "window-maximize",
    "window-restore",
    "github",
    "moon",
    "sun",
    "close",
)

GPUI_COMPONENT_ICONS = (
    REPO_ROOT.parent / "gpui-component" / "crates" / "assets" / "assets" / "icons"
)

MATERIAL_BASE = (
    "https://fonts.gstatic.com/s/i/short-term/release/"
    "materialsymbolsrounded/{name}/default/24px.svg"
)


def material_url(name: str) -> str:
    return MATERIAL_BASE.format(name=name)


def normalize_svg(raw: str) -> str:
    """Keep Material's 960×960 viewBox; paths are invisible if forced to 0 0 24 24."""
    svg = raw.strip()
    viewbox = "0 -960 960 960"
    if match := re.search(r'viewBox="([^"]+)"', svg):
        viewbox = match.group(1)
    path_match = re.search(r'<path[^>]*\sd="([^"]+)"', svg)
    if not path_match:
        raise ValueError("SVG has no <path d=...>")
    d = path_match.group(1)
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        f'<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" '
        f'viewBox="{viewbox}" fill="currentColor">\n'
        f'  <path d="{d}"/>\n'
        "</svg>\n"
    )


def download_icon(material_name: str) -> str:
    url = material_url(material_name)
    req = urllib.request.Request(url, headers={"User-Agent": "CyberFiles/sync_material_icons"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        return normalize_svg(resp.read().decode("utf-8"))


def main() -> None:
    OUT_ICONS.mkdir(parents=True, exist_ok=True)

    missing: list[str] = []
    for gpui_name, material_name in sorted(MATERIAL_ICON_MAP.items()):
        try:
            content = download_icon(material_name)
        except urllib.error.HTTPError as err:
            missing.append(f"{gpui_name} ({material_name}): HTTP {err.code}")
            continue
        except (OSError, ValueError) as err:
            missing.append(f"{gpui_name} ({material_name}): {err}")
            continue
        (OUT_ICONS / f"{gpui_name}.svg").write_text(content, encoding="utf-8", newline="\n")

    if missing:
        print("Failed downloads:")
        for line in missing:
            print(f"  - {line}")
        raise SystemExit(1)

    if GPUI_COMPONENT_ICONS.is_dir():
        for name in LUCIDE_PRESERVE:
            src = GPUI_COMPONENT_ICONS / f"{name}.svg"
            if src.is_file():
                shutil.copy2(src, OUT_ICONS / f"{name}.svg")
        print(f"Preserved {len(LUCIDE_PRESERVE)} Lucide chrome icons")
    else:
        print(f"Warning: gpui-component icons not found at {GPUI_COMPONENT_ICONS}")

    print(f"Synced {len(MATERIAL_ICON_MAP)} Material Rounded icons (default) -> {OUT_ICONS}")


if __name__ == "__main__":
    main()
