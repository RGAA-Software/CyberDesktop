mod archive;
mod audio_metadata;
mod clipboard;
mod columns;
mod drives;
mod file_tag;
mod folder_search;
mod folder_stats;
mod group;
mod history;
mod info_summary;
mod home;
mod item;
mod local;
mod omnibar;
mod ops;
mod path_util;
mod preview;
mod recent;
mod recycle;
mod search;
mod sort;
mod watcher;

pub use app_media::AudioFileMetadata;
pub use audio_metadata::{audio_file_duration, read_audio_metadata};
pub use archive::{
    compress_paths_to_zip, compress_paths_to_zip_at_path_cancellable,
    compress_paths_to_zip_cancellable, detect_archive_format, extract_archive_cancellable,
    compress_zip_file_display_name, extract_to_child_dir, is_archive_path, archive_progress_total,
    log_extract_environment, temp_zip_output_path, unique_zip_output_path, zip_output_path,
    ArchiveFormat, CompressCancelled, ExtractCancelled,
};
pub use clipboard::{
    copy_items, move_items, paths_conflict, transfer_items, transfer_one, transfer_one_cancellable,
    ClipboardOperation, ConflictResolution, FileClipboard, TransferCancelled, TransferConflict,
    TransferOutcome,
};
pub use columns::column_trail_for;
pub use drives::{
    default_user_profile, home_navigation_path, list_drives, user_desktop_directory, DriveInfo,
};
pub use file_tag::{
    apply_tags_to_items, build_path_tag_index, build_path_tag_index_from_config,
    build_path_tag_index_from_configs, file_items_for_tag_paths, parse_tag_color_hex,
    paths_for_file_tag, pick_random_tag_color, tags_for_path, TAG_COLOR_PRESETS,
};
pub use folder_search::{
    parse_search_query, search_folder, search_scope_path, SearchHit, SearchQuery, SearchScope,
};
pub use history::{apply_redo, apply_undo, FileOperation, OperationHistory};
pub use group::{
    build_display_rows, group_items, group_key_for, item_index_at_row, row_for_item_index,
    DisplayRow, FileGroup, GroupByDateUnit, GroupOption,
};
pub use folder_stats::{
    count_directory_entries, directory_tree_size, FolderEntryCounts,
};
pub use info_summary::{extension_type_counts, multi_select_summary, MultiSelectSummary};
pub use home::{
    eject_drive, file_tag_previews, list_quick_access_entries, load_home_file_tags,
    open_storage_sense_settings, quick_access_automatic_destinations_dir,
    sync_pin_to_shell_quick_access, sync_unpin_from_shell_quick_access, FileTagPreview,
    QuickAccessEntry, QuickAccessFolderKind,
};
pub use item::{DirectoryReadOptions, FileItem, FileItemKind, TagRef};
pub use local::read_directory;
pub use omnibar::{
    breadcrumb_dropdown_entries, breadcrumb_root_menu_sections, breadcrumb_visible_layout,
    breadcrumb_visible_layout_for_width, breadcrumb_visible_layout_for_widths,
    omnibar_path_suggestions, omnibar_search_suggestions, path_breadcrumbs, BreadcrumbDropdownResult, BreadcrumbMenuSection,
    BreadcrumbVisibleLayout, OmnibarPathSuggestion, PathBreadcrumb, BREADCRUMB_BLOCK_GAP,
};
pub use path_util::{
    all_direct_children_of, are_paths_on_same_drive, is_direct_child_of, normalize_directory_path,
    normalize_path, path_drive_root, paths_equal, paths_equal_directory,
};
pub use ops::{
    count_delete_items, create_directory, create_file, delete_paths, delete_paths_cancellable,
    recycle_paths, recycle_paths_cancellable, rename_path, unique_new_file_name,
    unique_new_folder_name, DeleteCancelled,
};
pub use preview::{is_image_path, is_text_preview_path, preview_kind, read_text_preview, PreviewKind};
pub use recent::{list_recent_files, recent_documents_enabled, RecentItem};
pub use recycle::{
    empty_recycle_bin, read_recycle_bin, restore_recycle_items, restore_recycled_originals,
};
pub use search::filter_items_by_query;
pub use sort::{sort_items, SortDirection, SortOption, SortPreferences};
pub use watcher::DirectoryWatcher;
