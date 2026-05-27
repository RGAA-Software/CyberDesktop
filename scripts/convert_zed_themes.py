#!/usr/bin/env python3
"""Convert Zed bundled theme JSON to gpui-component ThemeSet JSON for CyberFiles."""

from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
ZED_THEMES = ROOT.parent / "zed" / "assets" / "themes"
OUT_DIR = ROOT / "crates" / "assets" / "themes"

GPUI_SCHEMA = (
    "https://github.com/longbridge/gpui-component/raw/refs/heads/main/.theme-schema.json"
)

# Zed `style` key -> gpui-component `colors` key (1:1 UI chrome mapping).
ZED_STYLE_TO_COLORS: dict[str, str] = {
    "background": "background",
    "border": "border",
    "border.focused": "ring",
    "border.variant": "input.border",
    "border.selected": "list.active.border",
    "text": "foreground",
    "text.muted": "muted.foreground",
    "text.disabled": "muted.foreground",
    "text.placeholder": "muted.foreground",
    "text.accent": "primary.background",
    "icon": "foreground",
    "icon.muted": "muted.foreground",
    "icon.accent": "link",
    "element.background": "muted.background",
    "element.hover": "list.hover.background",
    "element.selected": "list.active.background",
    "element.active": "primary.active.background",
    "element.disabled": "muted.background",
    "surface.background": "secondary.background",
    "elevated_surface.background": "popover.background",
    "panel.background": "sidebar.background",
    "ghost_element.hover": "accent.background",
    "ghost_element.selected": "list.active.background",
    "title_bar.background": "title_bar.background",
    "title_bar.inactive_background": "tab.background",
    "tab_bar.background": "tab_bar.background",
    "tab.active_background": "tab.active.background",
    "tab.inactive_background": "tab.background",
    "toolbar.background": "table.head.background",
    "status_bar.background": "title_bar.background",
    "scrollbar.thumb.background": "scrollbar.thumb.background",
    "scrollbar.thumb.hover_background": "scrollbar.thumb.hover.background",
    "scrollbar.track.background": "scrollbar.background",
    "drop_target.background": "drop_target.background",
    "error": "danger.background",
    "error.background": "danger.background",
    "success": "success.background",
    "success.background": "success.background",
    "info": "info.background",
    "info.background": "info.background",
    "warning": "warning.background",
    "warning.background": "warning.background",
    "created": "success.background",
    "deleted": "danger.background",
    "modified": "warning.background",
    "hidden": "muted.foreground",
    "hint": "info.background",
    "terminal.ansi.red": "base.red",
    "terminal.ansi.bright_red": "base.red.light",
    "terminal.ansi.green": "base.green",
    "terminal.ansi.bright_green": "base.green.light",
    "terminal.ansi.blue": "base.blue",
    "terminal.ansi.bright_blue": "base.blue.light",
    "terminal.ansi.yellow": "base.yellow",
    "terminal.ansi.bright_yellow": "base.yellow.light",
    "terminal.ansi.magenta": "base.magenta",
    "terminal.ansi.bright_magenta": "base.magenta.light",
    "terminal.ansi.cyan": "base.cyan",
    "terminal.ansi.bright_cyan": "base.cyan.light",
}

SKIP_HIGHLIGHT_KEYS = frozenset({"accents"})


def style_to_colors(style: dict[str, Any]) -> dict[str, str]:
    colors: dict[str, str] = {}
    for zed_key, gpui_key in ZED_STYLE_TO_COLORS.items():
        value = style.get(zed_key)
        if value is None:
            continue
        if isinstance(value, str):
            colors[gpui_key] = value
    text = style.get("text")
    accent = style.get("text.accent") or style.get("icon.accent")
    if isinstance(text, str):
        colors.setdefault("primary.foreground", text)
        colors.setdefault("secondary.foreground", text)
        colors.setdefault("sidebar.foreground", text)
        colors.setdefault("tab.foreground", text)
        colors.setdefault("tab.active.foreground", text)
        colors.setdefault("popover.foreground", text)
    if isinstance(accent, str):
        colors.setdefault("link", accent)
        colors.setdefault("ring", accent)
        colors.setdefault("selection.background", accent)
        colors.setdefault("sidebar.primary.background", accent)
        colors.setdefault("sidebar.primary.foreground", text if isinstance(text, str) else "#ffffff")
    border = style.get("border")
    if isinstance(border, str):
        colors.setdefault("sidebar.border", border)
        colors.setdefault("title_bar.border", border)
        colors.setdefault("window.border", border)
        colors.setdefault("table.row.border", border)
    panel = style.get("panel.background")
    if isinstance(panel, str):
        colors.setdefault("sidebar.background", panel)
    element_bg = style.get("element.background")
    if isinstance(element_bg, str):
        colors.setdefault("list.background", element_bg)
        colors.setdefault("table.background", element_bg)
    element_hover = style.get("element.hover")
    if isinstance(element_hover, str):
        colors.setdefault("table.hover.background", element_hover)
    element_selected = style.get("element.selected")
    if isinstance(element_selected, str):
        colors.setdefault("table.active.background", element_selected)
    return colors


def style_to_highlight(style: dict[str, Any]) -> dict[str, Any]:
    highlight: dict[str, Any] = {}
    for key, value in style.items():
        if key in SKIP_HIGHLIGHT_KEYS:
            continue
        if value is None:
            continue
        highlight[key] = value
    return highlight


def convert_variant(zed_theme: dict[str, Any]) -> dict[str, Any]:
    appearance = zed_theme["appearance"]
    mode = "dark" if appearance == "dark" else "light"
    style = zed_theme.get("style") or {}
    return {
        "name": zed_theme["name"],
        "mode": mode,
        "colors": style_to_colors(style),
        "highlight": style_to_highlight(style),
    }


def convert_family(zed_family: dict[str, Any]) -> dict[str, Any]:
    return {
        "$schema": GPUI_SCHEMA,
        "name": zed_family["name"],
        "author": zed_family.get("author"),
        "url": "https://github.com/zed-industries/zed",
        "themes": [convert_variant(t) for t in zed_family["themes"]],
    }


def main() -> int:
    if not ZED_THEMES.is_dir():
        print(f"Zed themes not found: {ZED_THEMES}", file=sys.stderr)
        return 1

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for old in OUT_DIR.glob("cyberfiles-*.json"):
        old.unlink()

    for zed_dir in sorted(ZED_THEMES.iterdir()):
        if not zed_dir.is_dir():
            continue
        zed_json = next(zed_dir.glob("*.json"), None)
        if zed_json is None:
            continue
        family = json.loads(zed_json.read_text(encoding="utf-8"))
        out_name = zed_json.stem + ".json"
        out_path = OUT_DIR / out_name
        gpui_set = convert_family(family)
        out_path.write_text(
            json.dumps(gpui_set, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        license_src = zed_dir / "LICENSE"
        if license_src.is_file():
            shutil.copy2(license_src, OUT_DIR / f"{zed_json.stem}.LICENSE")
        print(f"wrote {out_path.name} ({len(gpui_set['themes'])} variants)")

    licenses = ZED_THEMES.parent / "LICENSES"
    if licenses.is_file():
        shutil.copy2(licenses, OUT_DIR / "ZED_THEMES_LICENSES")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
