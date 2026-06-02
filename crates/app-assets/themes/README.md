# UI themes (from Zed)

Built-in color themes are ported from [Zed](https://github.com/zed-industries/zed) `assets/themes/`:

| File | Zed source | Variants |
|------|------------|----------|
| `ant.json` | Hand-mapped from Ant Design color tokens | Ant Light, Ant Dark |
| `one.json` | `zed/assets/themes/one/` | One Light, One Dark |
| `ayu.json` | `zed/assets/themes/ayu/` | Ayu Light, Ayu Dark, Ayu Mirage |
| `gruvbox.json` | `zed/assets/themes/gruvbox/` | Light/Dark × standard, hard, soft |

Regenerate after updating the Zed checkout:

```bash
python scripts/convert_zed_themes.py
```

License files: `*.LICENSE` and `ZED_THEMES_LICENSES` from the Zed tree.
