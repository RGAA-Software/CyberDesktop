#!/usr/bin/env python3
"""Legacy: extract Files ThemedIcon paths to SVG.

CyberFiles UI icons now use `scripts/sync_material_icons.py` (Google Material Symbols).
This script remains only for reference or one-off Files asset extraction.
"""

from __future__ import annotations

import re
import shutil
import xml.etree.ElementTree as ET
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
FILES_ROOT = REPO_ROOT.parent / "Files"
FILES_STYLES = (
    FILES_ROOT / "src" / "Files.App.Controls" / "ThemedIcon" / "Styles"
)
FILES_APP_ASSETS = FILES_ROOT / "src" / "Files.App" / "Assets"
OUT_ICONS = REPO_ROOT / "crates" / "assets" / "assets" / "icons"
OUT_FILES = OUT_ICONS / "files"
OUT_APP_ASSETS = REPO_ROOT / "crates" / "assets" / "assets" / "files-app"

# Lucide icons kept from gpui-component (title bar, theme, GitHub, tab close) — not synced from Files.
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

# CyberFiles gpui IconName (kebab-case file) -> Files App.ThemedIcons key suffix
ICON_MAP: dict[str, str] = {
    "arrow-left": "NavBack",
    "arrow-right": "NavForward",
    "arrow-up": "NavUp",
    "redo-2": "Refresh",
    "panel-left-open": "PanelLeft",
    "panel-left-close": "PanelLeftClose",
    "panel-right-open": "PanelRight",
    "panel-right-close": "PanelRightClose",
    "panel-left": "PanelLeft",
    "layout-dashboard": "Settings.General.Widgets",
    "star": "Favorite",
    "plus": "New.Item",
    "folder": "Folder",
    "file": "File",
    "gallery-vertical-end": "FavoritePin",
    "delete": "Actions.Recycle",
    "chevron-right": "NavForward.12",
    "chevron-down": "NavForward.12",
    "external-link": "Shortcut",
    "settings-2": "Settings",
    "inbox": "Tag",
    "info": "Info",
    "bell": "StatusCenter",
    "hard-drive": "Actions.Eject",
    "globe": "Settings.General.Connections",
    "calendar": "Settings.General.TimeDate",
}

XAML_NS = {"x": "http://schemas.microsoft.com/winfx/2006/xaml"}


def key_to_filename(key: str) -> str:
    return key.replace(".", "_").replace(" ", "_").lower()


def parse_themed_icons() -> dict[str, dict[str, str]]:
    icons: dict[str, dict[str, str]] = {}
    for xaml in sorted(FILES_STYLES.glob("Icons*.xaml")):
        text = xaml.read_text(encoding="utf-8")
        for match in re.finditer(
            r'x:Key="App\.ThemedIcons\.([^"]+)"(.*?)'
            r'(?:OutlineIconData|PathData)="([^"]+)"',
            text,
            flags=re.DOTALL,
        ):
            key = match.group(1)
            path_data = match.group(3)
            if key not in icons:
                icons[key] = {}
            if "OutlineIconData" in match.group(0):
                icons[key]["outline"] = path_data
            else:
                icons[key].setdefault("layers", path_data)

        # Styles that only set OutlineIconData on separate lines
        blocks = re.split(r'x:Key="App\.ThemedIcons\.([^"]+)"', text)
        for i in range(1, len(blocks), 2):
            key = blocks[i]
            block = blocks[i + 1]
            outline = re.search(r'OutlineIconData" Value="([^"]+)"', block)
            if outline:
                icons.setdefault(key, {})["outline"] = outline.group(1)
            layers = re.findall(r'PathData="([^"]+)"', block)
            if layers and "outline" not in icons.get(key, {}):
                icons.setdefault(key, {})["layers"] = " ".join(layers)

    return icons


def path_for_icon(icon: dict[str, str]) -> str | None:
    if "outline" in icon:
        return icon["outline"]
    if "layers" in icon:
        return icon["layers"]
    return None


def make_svg(path_d: str, *, rotate_90: bool = False) -> str:
    transform = ""
    if rotate_90:
        transform = ' transform="rotate(90 8 8)"'
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none">\n'
        f'  <path fill="currentColor"{transform} d="{path_d}"/>\n'
        "</svg>\n"
    )


def write_svg(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


def main() -> None:
    if not FILES_STYLES.is_dir():
        raise SystemExit(f"Files repo not found: {FILES_STYLES}")

    icons = parse_themed_icons()
    if OUT_ICONS.exists():
        shutil.rmtree(OUT_ICONS)
    OUT_FILES.mkdir(parents=True, exist_ok=True)

    for key, data in sorted(icons.items()):
        path_d = path_for_icon(data)
        if not path_d:
            continue
        name = key_to_filename(key)
        write_svg(OUT_FILES / f"{name}.svg", make_svg(path_d))

    mapped_missing: list[str] = []
    for gpui_name, files_key in ICON_MAP.items():
        data = icons.get(files_key)
        if not data:
            mapped_missing.append(files_key)
            continue
        path_d = path_for_icon(data)
        if not path_d:
            mapped_missing.append(files_key)
            continue
        rotate = gpui_name == "chevron-down"
        write_svg(OUT_ICONS / f"{gpui_name}.svg", make_svg(path_d, rotate_90=rotate))

    if mapped_missing:
        print("Warning: missing Files icons for:", ", ".join(sorted(set(mapped_missing))))

    if FILES_APP_ASSETS.is_dir():
        if OUT_APP_ASSETS.exists():
            shutil.rmtree(OUT_APP_ASSETS)
        shutil.copytree(FILES_APP_ASSETS, OUT_APP_ASSETS)

    print(f"Extracted {len(list(OUT_FILES.glob('*.svg')))} Files ThemedIcons -> {OUT_FILES}")
    print(f"Mapped {len(list(OUT_ICONS.glob('*.svg')))} gpui icon files -> {OUT_ICONS}")
    if OUT_APP_ASSETS.exists():
        print(f"Copied Files.App assets -> {OUT_APP_ASSETS}")

    if GPUI_COMPONENT_ICONS.is_dir():
        for name in LUCIDE_PRESERVE:
            src = GPUI_COMPONENT_ICONS / f"{name}.svg"
            if src.is_file():
                shutil.copy2(src, OUT_ICONS / f"{name}.svg")
        print(f"Preserved {len(LUCIDE_PRESERVE)} Lucide chrome icons from {GPUI_COMPONENT_ICONS}")
    else:
        print(f"Warning: gpui-component icons not found at {GPUI_COMPONENT_ICONS}")


if __name__ == "__main__":
    main()
