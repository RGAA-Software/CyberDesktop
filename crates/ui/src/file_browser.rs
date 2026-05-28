use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::app_state::AppFileClipboard;
use crate::app_state::AppNavigation;
use crate::file_ops::{
    spawn_compress, spawn_file_transfer, spawn_paste_from_clipboard, FileTransferKind,
};
use crate::color_icon;
use crate::icons::{
    compact_icon, folder_icon_element, toolbar_icon,
};
use crate::list_icon_cache;
use crate::popup_menu::PopupMenu;
use crate::shell::navigation::NavigationTarget;
use crate::toolbar_button::TOOLBAR_BUTTON_PX;
use crate::toolbar_button::{toolbar_dropdown_button, toolbar_icon_button, toolbar_labeled_button};
use chrono::{DateTime, Local};
use cyberfiles_commands::{
    CancelRename, CompressItems, CopyItems, CopyPath, CutItems, DeleteItems,
    DeleteItemsPermanent, FocusSearch, NavigateNext, NavigatePrevious, NewFile, NewFolder,
    OpenItem, PasteItems, RefreshDirectory, RenameItem, SelectAll, ShellProperties, ViewCards,
    ViewColumns, ViewDetails, ViewGrid, ViewList, FILE_BROWSER,
};
use cyberfiles_core::{
    file_sort_prefs_from_config, file_view_mode_from_config, load_config, save_file_browser_prefs,
    VIEW_CARDS, VIEW_COLUMNS, VIEW_DETAILS, VIEW_GRID, VIEW_LIST,
};
use cyberfiles_fs::{
    column_trail_for, create_directory, create_file, delete_paths, file_items_for_tag_paths,
    filter_items_by_query, home_navigation_path, move_items, read_directory, read_recycle_bin,
    recycle_paths, rename_path, temp_zip_output_path, unique_new_file_name,
    unique_zip_output_path,
    unique_new_folder_name, ClipboardOperation, DirectoryReadOptions, DirectoryWatcher,
    FileClipboard, FileItem, FileItemKind, SortDirection, SortOption, SortPreferences,
};
use cyberfiles_platform_windows::{self as platform, ShellContextMenuEntry};
use gpui::{
    actions, anchored, deferred, prelude::*, ClickEvent, ClipboardItem, DismissEvent, Entity,
    FocusHandle, Focusable, ParentElement, ScrollStrategy, Subscription, Window, *,
};
use gpui_component::{
    dialog::DialogButtonProps,
    h_flex,
    input::{
        Input, InputEvent, InputState, Position, SelectAll as InputSelectAll,
        SelectToStartOfLine as InputSelectToStartOfLine,
    },
    notification::Notification,
    scroll::{ScrollableElement as _, Scrollbar, ScrollbarAxis, ScrollbarShow},
    v_flex, v_virtual_list, ActiveTheme as _, Disableable as _, ElementExt as _, IconName,
    StyledExt as _,
    Sizable as _, VirtualListScrollHandle, WindowExt as _,
};
use rust_i18n::t;

#[path = "file_browser/context_menu.rs"]
mod context_menu;
#[path = "file_browser/actions.rs"]
mod actions;
#[path = "file_browser/render.rs"]
mod render;
#[path = "file_browser/render_views.rs"]
mod render_views;
#[path = "file_browser/rename.rs"]
mod rename;
#[path = "file_browser/selection.rs"]
mod selection;
#[path = "file_browser/ops.rs"]
mod ops;
#[path = "file_browser/navigation.rs"]
mod navigation;
#[path = "file_browser/sweep.rs"]
pub(crate) mod sweep;
#[path = "file_browser/context_menu_state.rs"]
mod context_menu_state;
#[path = "file_browser/helpers.rs"]
mod helpers;
#[path = "file_browser/core.rs"]
mod core;

use helpers::*;

actions!(
    file_browser_prefs,
    [
        SortByName,
        SortByModified,
        SortByCreated,
        SortBySize,
        SortByType,
        ToggleSortDirection,
        ToggleShowHidden,
        ToggleShowFileExtensions,
        OpenInNewPane,
        OpenInNewWindow,
        OpenInTerminal,
        OpenWithDialog,
        CreateFolderFromSelection,
        CreateShortcut,
    ]
);

const FILE_ROW_SIZE_COMPACT: Size<Pixels> = size(px(1.), px(28.));
const FILE_ROW_SIZE: Size<Pixels> = size(px(1.), px(36.));
const FILE_ROW_SIZE_LARGE: Size<Pixels> = size(px(1.), px(44.));
const GRID_CELL_SIZE_SMALL: Size<Pixels> = size(px(96.), px(72.));
const GRID_CELL_SIZE: Size<Pixels> = size(px(112.), px(80.));
const GRID_CELL_SIZE_LARGE: Size<Pixels> = size(px(144.), px(104.));
const CARD_CELL_SIZE: Size<Pixels> = size(px(160.), px(120.));
const COLUMN_ROW_SIZE: Size<Pixels> = size(px(1.), px(32.));
const COLUMN_WIDTH: Pixels = px(200.);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Details,
    List,
    Grid,
    Cards,
    Columns,
}

impl ViewMode {
    fn from_config(value: &str) -> Self {
        match value {
            VIEW_GRID => Self::Grid,
            VIEW_CARDS => Self::Cards,
            VIEW_COLUMNS => Self::Columns,
            _ => Self::Details,
        }
    }

    fn config_value(self) -> &'static str {
        match self {
            Self::Details => VIEW_DETAILS,
            Self::List => VIEW_LIST,
            Self::Grid => VIEW_GRID,
            Self::Cards => VIEW_CARDS,
            Self::Columns => VIEW_COLUMNS,
        }
    }
}

pub use crate::drag::DraggedFilePaths;

const DRAG_PREVIEW_LABEL_FONT: Pixels = px(14.);
const DRAG_PREVIEW_HINT_FONT: Pixels = px(12.);

struct DragPathPreview {
    label: SharedString,
    /// Pointer-down position within the dragged row (used to center the preview on the cursor).
    grab_offset: Point<Pixels>,
    browser: Entity<FileBrowser>,
}

impl DragPathPreview {
    pub(super) fn new_entity(
        paths: &DraggedFilePaths,
        grab_offset: Point<Pixels>,
        browser: Entity<FileBrowser>,
        cx: &mut App,
    ) -> Entity<Self> {
        AppNavigation::cancel_breadcrumb_drag_preview(cx);
        let preview = cx.new(|_| Self {
            label: drag_preview_label(&paths.0).into(),
            grab_offset,
            browser: browser.clone(),
        });
        let _ = browser.update(cx, |this, _cx| {
            this.drag_preview = Some(preview.clone());
        });
        preview
    }
}

impl Render for DragPathPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let browser = self.browser.read(cx);
        let hint = browser.drag_hover_hint.as_deref();
        let invalid = browser.drag_hover_hint_invalid;
        let primary = browser.drag_hover_hint_primary;
        let hint_color = if invalid {
            cx.theme().danger
        } else if primary {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground
        };

        let preview_card = v_flex()
            .w(px(250.))
            .max_w(px(250.))
            .items_start()
            .gap_1()
            .px_2()
            .py(px(6.))
            .rounded(cx.theme().radius)
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().border)
            .when_some(hint, |this, hint| {
                this.child(
                    div()
                        .w_full()
                        .text_xs()
                        .font_bold()
                        .text_color(hint_color)
                        .child(hint.to_string()),
                )
            })
            .child(
                div()
                    .w_full()
                    .text_sm()
                    .text_color(cx.theme().popover_foreground)
                    .child(self.label.clone()),
            );

        div().size_full().child(
            preview_card
                .absolute()
                .left(self.grab_offset.x)
                .top(self.grab_offset.y),
        )
    }
}

struct RenameState {
    path: PathBuf,
    input: Entity<InputState>,
    _subscription: Subscription,
}

#[derive(Clone, Debug)]
pub(crate) struct ShellMenuCache {
    paths: Vec<PathBuf>,
    extended_verbs: bool,
    entries: Vec<ShellContextMenuEntry>,
}

/// Stable cache key for multi-select (order-independent).
pub(crate) fn normalize_paths_for_shell_cache(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut normalized: Vec<PathBuf> = paths.to_vec();
    normalized.sort();
    normalized
}

pub(crate) fn shell_cache_matches_selection(
    cache_paths: &[PathBuf],
    selection: &[PathBuf],
) -> bool {
    normalize_paths_for_shell_cache(cache_paths) == normalize_paths_for_shell_cache(selection)
}

/// Shell submenu content snapshot (built when the flyout is created — no `FileBrowser::read` in submenu callbacks).
#[derive(Clone, Debug)]
pub(crate) enum ShellSubmenuSnapshot {
    Loading,
    Empty,
    Ready(Vec<platform::ShellContextMenuEntry>),
}

pub(crate) fn shell_submenu_snapshot(
    cache: &Arc<RwLock<Option<ShellMenuCache>>>,
    paths: &[PathBuf],
    extended_verbs: bool,
) -> ShellSubmenuSnapshot {
    let Ok(guard) = cache.read() else {
        return ShellSubmenuSnapshot::Loading;
    };
    let Some(cache) = guard.as_ref() else {
        return ShellSubmenuSnapshot::Loading;
    };
    if !shell_cache_matches_selection(&cache.paths, paths) {
        return ShellSubmenuSnapshot::Loading;
    }
    if cache.extended_verbs != extended_verbs {
        return ShellSubmenuSnapshot::Loading;
    }
    if cache.entries.is_empty() {
        ShellSubmenuSnapshot::Empty
    } else {
        ShellSubmenuSnapshot::Ready(cache.entries.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BrowseLocation {
    Directory,
    RecycleBin,
    FileTag { tag_name: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SweepSelectionSurface {
    Main,
    Column(usize),
}

#[derive(Clone)]
struct SweepSelectionState {
    surface: SweepSelectionSurface,
    start_index: Option<usize>,
    current_index: Option<usize>,
    start_position: Point<Pixels>,
    current_position: Point<Pixels>,
    base_selection: BTreeSet<PathBuf>,
    modifiers: Modifiers,
}

pub struct FileBrowser {
    focus_handle: FocusHandle,
    browse_location: BrowseLocation,
    current_dir: PathBuf,
    back_stack: Vec<PathBuf>,
    forward_stack: Vec<PathBuf>,
    items: Vec<FileItem>,
    read_options: DirectoryReadOptions,
    sort_preferences: SortPreferences,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    scroll_handle: VirtualListScrollHandle,
    grid_scroll_handle: VirtualListScrollHandle,
    cards_scroll_handle: VirtualListScrollHandle,
    error: Option<String>,
    selected_paths: BTreeSet<PathBuf>,
    anchor_index: Option<usize>,
    focused_index: Option<usize>,
    renaming: Option<RenameState>,
    show_toolbar: bool,
    /// View/sort/actions row (Files `InnerNavigationToolbar`), below window nav + omnibar.
    show_content_toolbar: bool,
    show_info_pane: bool,
    view_mode: ViewMode,
    view_size_level: u8,
    search_query: String,
    display_items: Vec<FileItem>,
    column_trail: Vec<PathBuf>,
    column_listings: Vec<Vec<FileItem>>,
    column_scroll_handles: Vec<VirtualListScrollHandle>,
    columns_horizontal_scroll_handle: ScrollHandle,
    columns_shell_bounds: Option<Bounds<Pixels>>,
    columns_horizontal_overflow: bool,
    columns_horizontal_column_count: usize,
    _directory_watcher: Option<DirectoryWatcher>,
    _watcher_task: Option<Task<()>>,
    watched_dir: Option<PathBuf>,
    shell_menu_cache: Arc<RwLock<Option<ShellMenuCache>>>,
    _shell_menu_task: Option<Task<()>>,
    /// Selection key for an in-flight Shell fetch (dedupe rapid right-clicks).
    shell_menu_fetch_paths: Option<Vec<PathBuf>>,
    shell_menu_fetch_generation: u64,
    context_menu_extended_verbs: bool,
    context_menu_open: bool,
    context_menu_position: Point<Pixels>,
    context_menu_view: Option<Entity<PopupMenu>>,
    _context_menu_subscription: Option<Subscription>,
    /// Bumped when Shell entries finish loading; drives menu rebuild while open.
    shell_menu_revision: u64,
    context_menu_built_revision: u64,
    /// Bumped on each `refresh`; list icons warm once per bump (not per scroll).
    list_icon_warm_token: u64,
    list_icon_warm_scheduled: u64,
    _subscriptions: Vec<Subscription>,
    /// Cached measured cells-per-row for grid view.
    grid_cells_per_row: Option<usize>,
    /// Cached measured cells-per-row for cards view.
    cards_cells_per_row: Option<usize>,
    /// Last known viewport width; used to invalidate caches on window resize.
    last_viewport_width: Option<Pixels>,
    /// Selected file in columns view (col_index, path). Folders are tracked via column_trail.
    column_selected_path: Option<(usize, PathBuf)>,
    /// Active column in columns view. Determines which column receives actions like SelectAll.
    active_column_index: Option<usize>,
    drag_hover_target: Option<PathBuf>,
    drag_hover_hint: Option<String>,
    drag_hover_hint_invalid: bool,
    drag_hover_hint_primary: bool,
    drag_hover_generation: u64,
    drag_preview: Option<Entity<DragPathPreview>>,
    sweep_selection: Option<SweepSelectionState>,
    main_sweep_bounds: Option<Bounds<Pixels>>,
    column_sweep_bounds: BTreeMap<usize, Bounds<Pixels>>,
}

impl FileBrowser {
    /// File list for embedding in MainPage (window nav + omnibar live on `MainPage`).
    pub fn for_shell(cx: &mut Context<Self>, initial_dir: PathBuf) -> Self {
        Self::with_options(cx, initial_dir, false, true)
    }

    fn with_options(
        cx: &mut Context<Self>,
        current_dir: PathBuf,
        show_toolbar: bool,
        show_content_toolbar: bool,
    ) -> Self {
        let mut read_options = DirectoryReadOptions::default();
        let mut sort_preferences = SortPreferences::default();
        let (sort_option, sort_direction, show_hidden, show_file_extensions) =
            file_sort_prefs_from_config();
        {
            if let Some(option) = sort_option {
                sort_preferences.option = sort_option_from_config(&option);
            }
            if let Some(direction) = sort_direction {
                sort_preferences.direction = sort_direction_from_config(&direction);
            }
            if let Some(hidden) = show_hidden {
                read_options.show_hidden_items = hidden;
                read_options.show_dot_files = hidden;
            }
            read_options.show_file_extensions = show_file_extensions;
        }

        let view_mode = ViewMode::from_config(&file_view_mode_from_config());
        let (items, error) = load_files_dir(&current_dir, read_options, sort_preferences);
        let display_items = filter_items_by_query(&items, "");
        let column_trail = column_trail_for(&current_dir);
        let column_listings =
            column_listings_for(&column_trail, &read_options, sort_preferences, "");
        let column_scroll_handles = column_listings
            .iter()
            .map(|_| VirtualListScrollHandle::new())
            .collect();

        Self {
            focus_handle: cx.focus_handle(),
            browse_location: BrowseLocation::Directory,
            current_dir,
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            item_sizes: item_sizes_for(display_items.len(), ViewMode::Details, 2),
            scroll_handle: VirtualListScrollHandle::new(),
            grid_scroll_handle: VirtualListScrollHandle::new(),
            cards_scroll_handle: VirtualListScrollHandle::new(),
            items,
            read_options,
            sort_preferences,
            error,
            selected_paths: BTreeSet::new(),
            anchor_index: None,
            focused_index: None,
            renaming: None,
            show_toolbar,
            show_content_toolbar,
            show_info_pane: false,
            view_mode,
            view_size_level: 2,
            search_query: String::new(),
            display_items,
            column_trail,
            column_listings,
            column_scroll_handles,
            columns_horizontal_scroll_handle: ScrollHandle::default(),
            columns_shell_bounds: None,
            columns_horizontal_overflow: false,
            columns_horizontal_column_count: 0,
            _directory_watcher: None,
            _watcher_task: None,
            watched_dir: None,
            shell_menu_cache: Arc::new(RwLock::new(None)),
            _shell_menu_task: None,
            shell_menu_fetch_paths: None,
            shell_menu_fetch_generation: 0,
            context_menu_extended_verbs: false,
            context_menu_open: false,
            context_menu_position: Point::default(),
            context_menu_view: None,
            _context_menu_subscription: None,
            shell_menu_revision: 0,
            context_menu_built_revision: 0,
            list_icon_warm_token: 0,
            list_icon_warm_scheduled: u64::MAX,
            _subscriptions: Vec::new(),
            grid_cells_per_row: None,
            cards_cells_per_row: None,
            last_viewport_width: None,
            column_selected_path: None,
            active_column_index: None,
            drag_hover_target: None,
            drag_hover_hint: None,
            drag_hover_hint_invalid: false,
            drag_hover_hint_primary: false,
            drag_hover_generation: 0,
            drag_preview: None,
            sweep_selection: None,
            main_sweep_bounds: None,
            column_sweep_bounds: BTreeMap::new(),
        }
    }

}

impl Focusable for FileBrowser {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
