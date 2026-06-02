//! `EngineEditor` behavior split by domain.

mod clipboard;
mod close_confirm;
mod core;
mod disk_watch;
mod editing;
mod context_menu;
mod fold;
mod file_io;
mod find;
mod goto;
mod keyboard;
mod panel_drag;
mod mouse_scroll;
mod movement;
mod recent;
mod search_panel;
mod selection;
mod tabs;

pub(crate) use context_menu::EditorContextMenuState;
pub(crate) use file_io::external_paths_are_droppable;
pub(crate) use fold::FOLD_GUTTER_WIDTH;
