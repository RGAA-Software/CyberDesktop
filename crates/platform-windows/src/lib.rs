//! Windows-only Shell helpers (icons, clipboard file lists, known folders).

#[cfg(windows)]
mod clipboard;
#[cfg(windows)]
mod com;
#[cfg(windows)]
mod context_menu;
#[cfg(windows)]
mod eject;
#[cfg(windows)]
mod icons;
#[cfg(windows)]
mod paths;
#[cfg(windows)]
mod quick_access;
#[cfg(windows)]
mod recent_policy;
#[cfg(windows)]
mod recycle;
#[cfg(windows)]
mod shell;
#[cfg(windows)]
mod shell_folder;
#[cfg(windows)]
mod shell_icon;
#[cfg(windows)]
mod shell_menu_session;
#[cfg(windows)]
mod storage;
#[cfg(windows)]
mod volume;

#[cfg(windows)]
pub use clipboard::read_clipboard_file_paths;
#[cfg(windows)]
pub use eject::eject_volume;
#[cfg(windows)]
pub use icons::{
    icon_hint_for_path, icon_hint_from_extension, shell_dummy_icon_path, ShellIconHint,
};
#[cfg(windows)]
pub use paths::{is_recycle_bin_path, recycle_bin_folder, SHELL_RECYCLE_BIN_PATH};
pub use quick_access::{
    list_shell_quick_access_folders, shell_pin_to_quick_access, shell_unpin_from_quick_access,
    ShellQuickAccessEntry,
};
pub use recent_policy::recent_documents_tracking_enabled;
#[cfg(windows)]
pub use recycle::{list_recycle_bin_entries, RecycleBinEntry};
#[cfg(windows)]
pub use shell::{
    clear_shell_menu_session, format_shell_menu_label, invoke_shell_context_menu_item,
    invoke_shell_properties, load_lazy_submenu, open_in_new_explorer_window, open_item_properties,
    query_shell_context_menu_items, show_open_with_dialog, show_shell_context_menu,
    warm_up_query_context_menu, ShellContextMenuEntry,
};
#[cfg(windows)]
#[cfg(windows)]
pub use shell_folder::{
    list_cloud_drive_roots, list_known_folder_folders, list_wsl_distro_roots, ShellFolderEntry,
    FOLDERID_LIBRARIES, FOLDERID_NETWORK,
};
#[cfg(windows)]
pub use shell_icon::{
    menu_icon_pixel_size, shell_icon_pixel_size, shell_icon_png, shell_icon_png_for_list_key,
    shell_icon_png_from_cache, shell_icon_png_scaled, shell_thumbnail_png_scaled,
    system_scale_factor,
};
pub use storage::open_storage_sense_settings;
pub use volume::{volume_details, DriveKind, VolumeDetails};

#[cfg(not(windows))]
pub use stubs::*;

#[cfg(not(windows))]
mod stubs {
    use std::path::{Path, PathBuf};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum ShellIconHint {
        Folder,
        File,
        Symlink,
        Executable,
        Image,
        Archive,
    }

    pub fn icon_hint_for_path(_path: &Path) -> ShellIconHint {
        ShellIconHint::File
    }

    pub fn recycle_bin_folder() -> Option<PathBuf> {
        None
    }

    pub fn is_recycle_bin_path(_path: &Path) -> bool {
        false
    }

    pub fn read_clipboard_file_paths() -> Vec<PathBuf> {
        Vec::new()
    }

    pub fn show_shell_context_menu(_paths: &[PathBuf]) -> anyhow::Result<()> {
        Ok(())
    }

    #[derive(Debug, Clone)]
    pub enum ShellContextMenuEntry {
        Separator,
        Item {
            label: String,
            command_offset: u32,
            command_string: Option<String>,
            icon_png: Option<Vec<u8>>,
        },
        Submenu {
            label: String,
            children: Vec<ShellContextMenuEntry>,
            icon_png: Option<Vec<u8>>,
            lazy_parent_index: Option<u32>,
        },
    }

    pub fn load_lazy_submenu(_parent_index: u32) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
        Ok(Vec::new())
    }

    pub fn warm_up_query_context_menu() {}

    pub fn format_shell_menu_label(raw: &str) -> String {
        raw.to_string()
    }

    pub fn query_shell_context_menu_items(
        _paths: &[PathBuf],
        _extended_verbs: bool,
        _menu_icon_extract_px: u32,
    ) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
        Ok(Vec::new())
    }

    pub fn menu_icon_pixel_size(_scale_factor: f32) -> u32 {
        16
    }

    pub fn system_scale_factor() -> f32 {
        1.0
    }

    pub fn invoke_shell_context_menu_item(
        _paths: &[PathBuf],
        _command_offset: u32,
        _extended_verbs: bool,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn clear_shell_menu_session() {}

    #[derive(Debug, Clone)]
    pub struct RecycleBinEntry {
        pub display_name: String,
        pub shell_path: PathBuf,
        pub size: Option<u64>,
        pub modified: Option<std::time::SystemTime>,
    }

    pub fn list_recycle_bin_entries() -> Vec<RecycleBinEntry> {
        Vec::new()
    }

    #[derive(Debug, Clone)]
    pub struct ShellQuickAccessEntry {
        pub display_name: String,
        pub path: PathBuf,
    }

    pub fn list_shell_quick_access_folders() -> anyhow::Result<Vec<ShellQuickAccessEntry>> {
        Ok(Vec::new())
    }

    #[derive(Debug, Clone)]
    pub struct ShellFolderEntry {
        pub display_name: String,
        pub path: PathBuf,
    }

    pub fn open_item_properties(_path: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
