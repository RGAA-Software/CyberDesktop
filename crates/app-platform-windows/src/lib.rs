//! Windows-only Shell helpers (icons, clipboard file lists, known folders).

#[cfg(windows)]
mod clipboard;
#[cfg(windows)]
mod com;
#[cfg(windows)]
mod context_menu;
#[cfg(windows)]
mod drag_leave;
#[cfg(windows)]
mod drag_out;
#[cfg(windows)]
mod eject;
#[cfg(windows)]
mod hybrid_shell_session;
#[cfg(windows)]
mod icons;
#[cfg(windows)]
mod paths;
#[cfg(windows)]
mod per_handler_shell;
#[cfg(windows)]
mod quick_access;
#[cfg(windows)]
mod recent_policy;
#[cfg(windows)]
mod recycle;
#[cfg(windows)]
mod search;
#[cfg(windows)]
mod sevenzip;
#[cfg(windows)]
mod shell;
#[cfg(windows)]
mod shell_folder;
#[cfg(windows)]
mod shell_icon;
#[cfg(windows)]
mod shell_menu_icon;
#[cfg(windows)]
mod shell_menu_session;
#[cfg(windows)]
mod shell_new;
#[cfg(windows)]
mod storage;
#[cfg(windows)]
mod volume;

#[cfg(windows)]
pub use clipboard::read_clipboard_file_paths;
#[cfg(windows)]
pub use com::ensure_com_multithreaded;
#[cfg(windows)]
pub use com::hard_exit_process;
#[cfg(windows)]
pub use com::log_current_apartment;
#[cfg(windows)]
pub use com::ThreadWithMessageQueueWithPump;
#[cfg(windows)]
pub use context_menu::create_context_menu_on_current_thread;
#[cfg(windows)]
pub use drag_leave::{
    arm_native_drag_leave, disarm_native_drag_leave, take_native_drag_leave_pending,
};
pub use drag_out::{begin_drag_out, is_cursor_outside_window, DragEffect};
#[cfg(windows)]
pub use eject::eject_volume;

#[cfg(windows)]
pub use icons::{
    icon_hint_for_path, icon_hint_from_extension, shell_dummy_icon_path, ShellIconHint,
};
#[cfg(windows)]
pub use paths::{
    is_recycle_bin_path, list_default_user_folders, recycle_bin_folder, DefaultUserFolder,
    DefaultUserFolderKind, SHELL_RECYCLE_BIN_PATH,
};
pub use quick_access::{
    list_shell_quick_access_folders, shell_pin_to_quick_access, shell_unpin_from_quick_access,
    ShellQuickAccessEntry,
};
pub use recent_policy::recent_documents_tracking_enabled;
#[cfg(windows)]
pub use recycle::{
    empty_recycle_bin, list_recycle_bin_entries, recycle_shell_paths_for_originals,
    restore_recycle_bin_items, RecycleBinEntry, RecycleBinItemKind,
};
#[cfg(windows)]
pub use search::search_indexed_aqs;
#[cfg(windows)]
pub use sevenzip::{bundled_dll_path, extract_in_process, SevenZipExtractError};
#[cfg(windows)]
pub use shell::{
    clear_shell_menu_session, format_shell_menu_label, invoke_shell_context_menu_item,
    invoke_shell_properties, load_lazy_submenu, open_in_new_explorer_window, open_item_properties,
    query_shell_context_menu_items, shell_execute_open, show_open_with_dialog,
    show_open_with_dialog_blocking, show_shell_context_menu, warm_up_hybrid_shell_menu,
    warm_up_query_context_menu, ShellContextMenuEntry,
};
#[cfg(windows)]
pub use hybrid_shell_session::{set_shell_menu_layer_b_enabled, shell_menu_layer_b_enabled};
#[cfg(windows)]
pub use shell_folder::{
    list_cloud_drive_roots, list_known_folder_folders, list_network_computers, list_network_shares,
    list_wsl_distro_roots, wsl_installed, NetworkItemCategory, ShellFolderEntry,
    FOLDERID_LIBRARIES, FOLDERID_NETWORK,
};
#[cfg(windows)]
pub use shell_icon::{
    menu_icon_pixel_size, shell_icon_pixel_size, shell_icon_png, shell_icon_png_batch,
    shell_icon_png_for_list_key, shell_icon_png_from_cache, shell_icon_png_scaled,
    shell_thumbnail_png_scaled, system_scale_factor,
};
#[cfg(windows)]
pub use shell_new::{
    clear_shell_new_menu_cache, create_shell_new_item, peek_shell_new_menu_items,
    query_shell_new_menu_items, refresh_shell_new_menu_cache, shell_new_item_is_folder,
    warm_shell_new_menu_cache, ShellNewMenuItem,
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DefaultUserFolderKind {
        Desktop,
        Documents,
        Downloads,
        Music,
        Videos,
        Pictures,
    }

    #[derive(Debug, Clone)]
    pub struct DefaultUserFolder {
        pub kind: DefaultUserFolderKind,
        pub display_name: String,
        pub path: PathBuf,
    }

    pub fn list_default_user_folders() -> Vec<DefaultUserFolder> {
        Vec::new()
    }

    pub fn read_clipboard_file_paths() -> Vec<PathBuf> {
        Vec::new()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DragEffect {
        None,
        Copy,
        Move,
    }

    pub fn begin_drag_out(
        _paths: &[PathBuf],
        _allow_move: bool,
        _host_hwnd: Option<isize>,
    ) -> anyhow::Result<DragEffect> {
        Ok(DragEffect::None)
    }

    pub fn is_cursor_outside_window(_host_hwnd: isize) -> bool {
        false
    }

    pub fn arm_native_drag_leave(_hwnd: isize) {}

    pub fn disarm_native_drag_leave() {}

    pub fn take_native_drag_leave_pending() -> bool {
        false
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
            handler_clsid: Option<String>,
        },
        Submenu {
            label: String,
            children: Vec<ShellContextMenuEntry>,
            icon_png: Option<Vec<u8>>,
            lazy_parent_index: Option<u32>,
            handler_clsid: Option<String>,
        },
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
        _handler_clsid: Option<&str>,
        _extended_verbs: bool,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn load_lazy_submenu(
        _handler_clsid: Option<String>,
        _parent_index: u32,
    ) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
        Ok(Vec::new())
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

    pub fn empty_recycle_bin() -> anyhow::Result<()> {
        Ok(())
    }

    pub fn restore_recycle_bin_items(_paths: &[PathBuf]) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn recycle_shell_paths_for_originals(
        _original_paths: &[PathBuf],
    ) -> anyhow::Result<Vec<PathBuf>> {
        Ok(Vec::new())
    }

    #[derive(Debug, Clone)]
    pub struct ShellQuickAccessEntry {
        pub display_name: String,
        pub path: PathBuf,
    }

    pub fn list_shell_quick_access_folders() -> anyhow::Result<Vec<ShellQuickAccessEntry>> {
        Ok(Vec::new())
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NetworkItemCategory {
        Computer,
        MediaDevice,
        Printer,
        OtherDevice,
        Infrastructure,
        Unknown,
    }

    #[derive(Debug, Clone)]
    pub struct ShellFolderEntry {
        pub display_name: String,
        pub path: PathBuf,
        pub category: NetworkItemCategory,
        pub item_type_text: Option<String>,
    }

    pub fn open_item_properties(_path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn shell_execute_open(_path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn search_indexed_aqs(
        _scope_root: &Path,
        _aqs_query: &str,
        _cancel: &std::sync::atomic::AtomicBool,
        _max_results: usize,
    ) -> anyhow::Result<Vec<PathBuf>> {
        anyhow::bail!("indexed search is only available on Windows")
    }
}
