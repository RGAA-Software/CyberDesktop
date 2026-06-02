//! CyberFiles-maintained fork of gpui-component [`PopupMenu`](popup_menu::PopupMenu).
//!
//! Source copied from gpui-component v0.5.x (`crates/files-app-ui/src/menu/popup_menu.rs`, `menu_item.rs`).
//! Modify here for 32px rows, Shell color PNG icons, and text alignment.

mod actions;
mod context_menu;
mod dropdown_menu;
mod menu_item;
mod popup_menu;

pub use context_menu::ContextMenuExt;
pub use dropdown_menu::DropdownMenu;
pub use popup_menu::{init, PopupMenu, PopupMenuItem};
