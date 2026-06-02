# CyberFiles PopupMenu (fork)

Vendored from [gpui-component](https://github.com/longbridge/gpui-component) `crates/files-app-ui/src/menu/`:

- `popup_menu.rs`
- `menu_item.rs`
- `context_menu.rs`
- `dropdown_menu.rs`
- `actions.rs` (menu keyboard actions)

## Why forked

Upstream `PopupMenu` cannot meet Files-style context menus in one surface:

- Fixed **26px** rows (we default **32px** via `item_row_h`)
- `Icon` / SVG tinting strips **color Shell PNG** icons
- `ElementItem` + native icon gutter causes **misaligned** labels

## Customizations (CyberFiles)

| API | Purpose |
|-----|---------|
| `DEFAULT_ITEM_ROW_HEIGHT` (32px) | Uniform item + submenu row height |
| `item_row_h()` | Override row height per menu |
| `ICON_SLOT_SIZE` (16px) | Left icon gutter width |
| `PopupMenuItem::icon_png()` | Full-color Shell bitmap in icon slot |

## Maintenance

When upgrading gpui-component, diff upstream `menu/popup_menu.rs` and merge into `popup_menu.rs`.
