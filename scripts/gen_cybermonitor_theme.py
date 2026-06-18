#!/usr/bin/env python3
"""Generate crates/app-assets/themes/cybermonitor.json from cybereditor.json.

Replaces the CyberEditor blue accent (#2f6df6 family) with the new CyberMonitor
purple accent (#7548d8) and shifts the surrounding neutral surfaces toward a
purple tint so title bars, cards and sidebars visibly follow theme changes.
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "crates/app-assets/themes/cybereditor.json"
DST = ROOT / "crates/app-assets/themes/cybermonitor.json"

# Mapping of 6-digit base colors (without alpha). 8-digit hex values in the JSON
# are matched by base; the original alpha channel is preserved.
BASE_MAP = {
    # primary blue -> purple
    "2f6df6": "7548d8",
    "1a5ce0": "5a32b0",
    "4a82f8": "8a6be0",
    "5c9af8": "8a6be0",
    "7ba8f8": "9f7ae0",
    "69b1ff": "bfa6f3",
    "4096ff": "8a6be0",
    # info/link blues -> purple
    "1677ff": "7548d8",
    "0958d9": "5a32b0",
    "1668dc": "7548d8",
    "3c89e8": "8a6be0",
    "3d7af0": "7548d8",
    # dark accent surfaces
    "0a2454": "2e1065",
    "0e2f6f": "2e1065",
    "15417e": "3d1f7a",
    # light surfaces (blue tint -> purple tint)
    "d4e2f8": "e9d5ff",
    "e6effd": "f3e8ff",
    "e8eefaff": "f5eaff",
    "f0f4fb": "f5f3ff",
    "e8eef8": "ede9f7",
    "e0e8f5": "e8e0f5",
    "122d6f": "3d1f7a",
    "dcecff": "e9d5ff",
    "91caff": "c4b5fd",
    # dark neutral surfaces (blue-grey -> purple-grey)
    "0d1a2d": "0d0b14",
    "0e1a2a": "131225",
    "111820": "181525",
    "141c2b": "1a1725",
    "1a2a40": "1f1a35",
    "2a3a52": "2a2540",
    "0a101a": "110e18",
    "111b26": "181225",
    "141414": "14121a",
    "d0d8e8": "d8d0e8",
    "e8eefa": "f5eaff",
    "e3e5e8": "e3e0e8",
    "f3f5f7": "f5f3f7",
    # misc accent foregrounds
    "9abbfc": "c4b5fd",
}

HEX_RE = re.compile(r"#([0-9a-fA-F]{6})([0-9a-fA-F]{2})")


def replace_hex(text: str) -> str:
    def repl(m: re.Match) -> str:
        base = m.group(1).lower()
        alpha = m.group(2).lower()
        return f"#{BASE_MAP.get(base, base)}{alpha}"

    return HEX_RE.sub(repl, text)


def deep_update(obj, path, value):
    keys = path.split(".")
    cur = obj
    for k in keys[:-1]:
        cur = cur[k]
    cur[keys[-1]] = value


def main():
    text = SRC.read_text(encoding="utf-8")
    text = replace_hex(text)
    data = json.loads(text)

    # Theme family metadata
    data["name"] = "CyberMonitor"
    data["author"] = "CyberDesktop"
    data["url"] = "https://github.com/RGAA-Software/CyberDesktop"

    for theme in data["themes"]:
        theme["name"] = theme["name"].replace("CyberEditor", "CyberMonitor")
        colors = theme["colors"]
        is_dark = theme["mode"] == "dark"

        # Make title bar and secondary surfaces clearly different from the main
        # background so light/dark toggles are immediately visible.
        if is_dark:
            colors["title_bar.background"] = "#181525ff"
            colors["tab.background"] = "#181525ff"
            colors["tab_bar.background"] = "#181525ff"
            colors["secondary.background"] = "#1f1b32ff"
            colors["secondary.hover.background"] = "#28233fff"
            colors["secondary.active.background"] = "#28233fff"
            colors["muted.background"] = "#181525ff"
            colors["sidebar.background"] = "#181525ff"
            colors["accent.background"] = "#2e1065ff"
            colors["sidebar.accent.background"] = "#28233fff"
            colors["title_bar.border"] = "#2a2540ff"
            colors["sidebar.border"] = "#2a2540ff"
        else:
            colors["title_bar.background"] = "#f5f3ffff"
            colors["tab.background"] = "#f5f3ffff"
            colors["tab_bar.background"] = "#f5f3ffff"
            colors["secondary.background"] = "#ede9f7ff"
            colors["secondary.hover.background"] = "#f5eaffff"
            colors["secondary.active.background"] = "#e9d5ffff"
            colors["muted.background"] = "#ede9f7ff"
            colors["sidebar.background"] = "#ede9f7ff"
            colors["accent.background"] = "#f3e8ffff"
            colors["sidebar.accent.background"] = "#f3e8ffff"
            colors["title_bar.border"] = "#e8e0f5ff"
            colors["sidebar.border"] = "#e8e0f5ff"

        # Zed highlight section mirrors the UI surfaces used by the editor chrome.
        hl = theme.get("highlight", {})
        if is_dark:
            hl["title_bar.background"] = "#181525ff"
            hl["title_bar.inactive_background"] = "#14121aff"
            hl["toolbar.background"] = "#14121aff"
            hl["tab_bar.background"] = "#181525ff"
            hl["tab.inactive_background"] = "#14121aff"
            hl["tab.active_background"] = "#1a1725ff"
            hl["panel.background"] = "#14121aff"
            hl["status_bar.background"] = "#181525ff"
            hl["surface.background"] = "#1a1725ff"
            hl["elevated_surface.background"] = "#1a1725ff"
            hl["element.background"] = "#1a1725ff"
            hl["element.hover"] = "#28233fff"
            hl["element.active"] = "#2e1065ff"
            hl["element.selected"] = "#2e1065ff"
            hl["element.disabled"] = "#14121aff"
        else:
            hl["title_bar.background"] = "#ffffffff"
            hl["title_bar.inactive_background"] = "#ffffffff"
            hl["toolbar.background"] = "#ffffffff"
            hl["tab_bar.background"] = "#ffffffff"
            hl["tab.inactive_background"] = "#ffffffff"
            hl["tab.active_background"] = "#f3e8ffff"
            hl["panel.background"] = "#ffffffff"
            hl["status_bar.background"] = "#ffffffff"
            hl["surface.background"] = "#ffffffff"
            hl["elevated_surface.background"] = "#ffffffff"
            hl["element.background"] = "#ffffffff"
            hl["element.hover"] = "#f5eaffff"
            hl["element.active"] = "#e9d5ffff"
            hl["element.selected"] = "#f3e8ffff"
            hl["element.disabled"] = "#f5f3f7ff"

    DST.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {DST}")


if __name__ == "__main__":
    main()
