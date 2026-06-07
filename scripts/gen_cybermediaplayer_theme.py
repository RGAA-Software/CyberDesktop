import json
import re

# Read the source CyberFiles theme
with open("crates/app-assets/themes/cyberfiles.json", "r", encoding="utf-8") as f:
    content = f.read()

data = json.loads(content)

# Helper to adjust color alpha
def replace_color(hex_str, mapping):
    """Replace hex colors with alpha channel support.
    hex_str format: #RRGGBBAA or #RRGGBB
    """
    if not hex_str.startswith("#"):
        return hex_str
    base = hex_str[:7]
    alpha = hex_str[7:] if len(hex_str) > 7 else ""
    if base in mapping:
        return mapping[base] + alpha
    return hex_str

def replace_in_obj(obj, mapping):
    if isinstance(obj, dict):
        return {k: replace_in_obj(v, mapping) for k, v in obj.items()}
    elif isinstance(obj, list):
        return [replace_in_obj(v, mapping) for v in obj]
    elif isinstance(obj, str):
        return replace_color(obj, mapping)
    return obj

# Light mode color mapping (green -> teal)
light_mapping = {
    "#5c8f18": "#009ca6",   # primary
    "#4a7314": "#008a93",   # primary hover
    "#a8cc70": "#4dd0e1",   # border selected / active
    "#e7ebdd": "#e0f5f7",   # hover background
    "#eaf4d9": "#e6f7f8",   # active / accent background
    "#dce8c8": "#cceff1",   # primary active background
}

# Dark mode color mapping (bright green -> teal)
dark_mapping = {
    "#78b828": "#009ca6",   # primary
    "#8fcc38": "#00b8c4",   # primary hover (brighter in dark)
    "#5d9117": "#007a82",   # border selected / active
    "#1e2d13": "#003d42",   # active background
    "#2a3d18": "#004d54",   # primary active background
    "#c8e998": "#80deef",   # accent foreground
}

# Process themes
themes = data["themes"]
for i, theme in enumerate(themes):
    if theme["mode"] == "light":
        themes[i] = replace_in_obj(theme, light_mapping)
    elif theme["mode"] == "dark":
        themes[i] = replace_in_obj(theme, dark_mapping)

# Update metadata
data["name"] = "CyberMediaPlayer"
data["themes"][0]["name"] = "CyberMediaPlayer Light"
data["themes"][1]["name"] = "CyberMediaPlayer Dark"

# Write output
output = json.dumps(data, indent=2, ensure_ascii=False)
with open("crates/app-assets/themes/cybermediaplayer.json", "w", encoding="utf-8") as f:
    f.write(output)

print("Generated crates/app-assets/themes/cybermediaplayer.json")
