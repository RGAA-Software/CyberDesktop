use super::*;
use crate::app_state::AppFileClipboard;
use crate::exe_icon_cache;
use crate::file_type_icon_colors;
use crate::file_type_icons;
/// Opacity for items cut to the in-app clipboard but not pasted yet (Files `DimItemOpacity`).
pub(super) const CUT_PENDING_ITEM_OPACITY: f32 = 0.4;

fn file_type_tile_colors(item: &FileItem, cx: &App) -> (Hsla, Hsla) {
    let is_dark = cx.theme().mode.is_dark();
    let path = file_type_icons::svg_path_for_path(&item.path);
    file_type_icon_colors::tile_colors_for_svg_path(path, is_dark)
}

fn embedded_exe_tile_colors(cx: &App) -> (Hsla, Hsla) {
    if cx.theme().mode.is_dark() {
        (
            cx.theme().muted.opacity(0.32),
            cx.theme().muted_foreground,
        )
    } else {
        (cx.theme().secondary, cx.theme().muted_foreground)
    }
}

pub(super) fn path_is_cut_pending(path: &Path, cx: &App) -> bool {
    let Some(clipboard) = AppFileClipboard::peek(cx) else {
        return false;
    };
    clipboard.operation == ClipboardOperation::Cut
        && clipboard.paths.iter().any(|p| p == path)
}

impl FileBrowser {
#[allow(dead_code)]
    pub(super) fn file_item_kind_icon(kind: FileItemKind) -> AnyElement {
        match kind {
            FileItemKind::Folder => toolbar_tabler(tabler_icons::FOLDER).into_any_element(),
            FileItemKind::Symlink => compact_icon(IconName::ExternalLink).into_any_element(),
            FileItemKind::File | FileItemKind::Other => {
                compact_icon(IconName::File).into_any_element()
            }
        }
    }

    fn row_list_icon_inner(
        item: &FileItem,
        logical_size: Pixels,
        window: &Window,
    ) -> (AnyElement, bool) {
        #[cfg(windows)]
        if exe_icon_cache::is_exe_item(item) {
            let size_px =
                platform::shell_icon_pixel_size(logical_size.as_f32(), window.scale_factor());
            if let Some(png) = exe_icon_cache::cached_png(&item.path, size_px) {
                return (
                    img(std::sync::Arc::new(Image::from_bytes(
                        ImageFormat::Png,
                        (*png).clone(),
                    )))
                    .size(logical_size)
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                    true,
                );
            }
        }
        (
            div()
                .size(logical_size)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    toolbar_tabler(file_type_icons::svg_path_for_path(&item.path))
                        .with_size(gpui_component::Size::Size(logical_size)),
                )
                .into_any_element(),
            false,
        )
    }

#[allow(dead_code)]
    pub(super) fn row_list_icon_tile(
        item: &FileItem,
        icon: AnyElement,
        tile_size: Pixels,
        cx: &App,
    ) -> AnyElement {
        let (bg, fg) = file_type_tile_colors(item, cx);
        let radius = if tile_size >= px(32.) {
            px(10.)
        } else {
            px(7.)
        };
        div()
            .size(tile_size)
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(radius)
            .bg(bg)
            .text_color(fg)
            .child(icon)
            .into_any_element()
    }

    /// List row icon: embedded `.exe` icon when available, else bundled SVG on a type-colored tile.
    pub(super) fn row_list_icon(
        item: &FileItem,
        logical_size: Pixels,
        window: &Window,
        cx: &App,
    ) -> impl IntoElement {
        let (inner, embedded_exe) = Self::row_list_icon_inner(item, logical_size, window);
        let tile_size = if logical_size >= FILE_LIST_ICON_TILE {
            logical_size
        } else {
            FILE_LIST_ICON_TILE
        };
        let (bg, fg) = if embedded_exe {
            embedded_exe_tile_colors(cx)
        } else {
            file_type_tile_colors(item, cx)
        };
        let radius = if tile_size >= px(32.) {
            px(10.)
        } else {
            px(7.)
        };
        div()
            .size(tile_size)
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(radius)
            .bg(bg)
            .text_color(fg)
            .child(inner)
    }

    pub(super) fn file_list_row_shell(
        shell_id: impl Into<ElementId>,
        selected: bool,
        row: impl IntoElement,
        cx: &App,
    ) -> impl IntoElement {
        div()
            .id(shell_id)
            .relative()
            .w_full()
            .flex_none()
            .when(selected, |shell| {
                shell.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(5.))
                        .bottom(px(5.))
                        .w(px(3.))
                        .rounded_r_full()
                        .bg(cx.theme().primary),
                )
            })
            .child(row)
    }

    /// After directory refresh: extract embedded icons for visible `.exe` files (cached).
    pub(super) fn schedule_list_icon_warm(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.list_icon_warm_scheduled == self.list_icon_warm_token {
            return;
        }
        self.list_icon_warm_scheduled = self.list_icon_warm_token;
        #[cfg(windows)]
        {
            let paths = self
                .display_items
                .iter()
                .filter(|item| exe_icon_cache::is_exe_item(item))
                .map(|item| item.path.clone())
                .collect::<Vec<_>>();
            if paths.is_empty() {
                return;
            }
            let size_px = platform::shell_icon_pixel_size(
                FILE_LIST_ICON_SIZE.as_f32(),
                window.scale_factor(),
            );
            cx.spawn(async move |this, cx| {
                let _ = cx
                    .background_spawn(async move { exe_icon_cache::warm_exe_icons(paths, size_px) })
                    .await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        #[cfg(not(windows))]
        {
            let _ = (window, cx);
        }
    }

    pub(super) fn set_sort_option(&mut self, option: SortOption) {
        self.sort_preferences.option = option;
        self.refresh();
        self.persist_prefs();
    }

    pub(super) fn set_sort_direction(&mut self, direction: SortDirection, cx: &mut Context<Self>) {
        if self.sort_preferences.direction == direction {
            return;
        }
        self.sort_preferences.direction = direction;
        self.refresh();
        self.persist_prefs();
        cx.notify();
    }

    pub(super) fn toggle_sort_direction(&mut self, cx: &mut Context<Self>) {
        self.sort_preferences.direction = match self.sort_preferences.direction {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        };
        self.refresh();
        self.persist_prefs();
        cx.notify();
    }

    pub(super) fn set_group_option(&mut self, option: GroupOption, cx: &mut Context<Self>) {
        if self.group_option == option {
            return;
        }
        self.group_option = option;
        self.collapsed_groups.clear();
        self.apply_filter();
        self.persist_prefs();
        cx.notify();
    }

    pub(super) fn set_group_by_date(
        &mut self,
        option: GroupOption,
        unit: GroupByDateUnit,
        cx: &mut Context<Self>,
    ) {
        if self.group_option == option && self.group_date_unit == unit {
            return;
        }
        self.group_option = option;
        self.group_date_unit = unit;
        self.collapsed_groups.clear();
        self.apply_filter();
        self.persist_prefs();
        cx.notify();
    }

    pub(super) fn toggle_group_collapsed(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.collapsed_groups.contains(key) {
            self.collapsed_groups.remove(key);
        } else {
            self.collapsed_groups.insert(key.to_string());
        }
        self.apply_filter();
        cx.notify();
    }

    pub(super) fn grouping_enabled(&self) -> bool {
        self.group_option != GroupOption::None && view_supports_grouping(self.view_mode)
    }

    pub(super) fn sort_label(&self) -> String {
        let field = match self.sort_preferences.option {
            SortOption::Name => t!("files.sort.name"),
            SortOption::DateModified => t!("files.sort.modified"),
            SortOption::DateCreated => t!("files.sort.created"),
            SortOption::Size => t!("files.sort.size"),
            SortOption::FileType => t!("files.sort.type"),
            SortOption::Path => t!("files.sort.path"),
            SortOption::Tag => t!("files.sort.tag"),
        };
        let arrow = match self.sort_preferences.direction {
            SortDirection::Ascending => "↑",
            SortDirection::Descending => "↓",
        };
        format!("{field} {arrow}")
    }
}

fn menu_icon(name: IconName) -> gpui_component::Icon {
    compact_icon(name)
}

pub(crate) fn item_parent_path(item: &FileItem) -> String {
    item.path
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub(crate) fn view_supports_grouping(mode: ViewMode) -> bool {
    matches!(mode, ViewMode::Details | ViewMode::List)
}

fn sort_menu_group_item(
    menu: PopupMenu,
    label: impl Into<SharedString>,
    icon: gpui_component::Icon,
    checked: bool,
    action: Box<dyn gpui::Action>,
    available: bool,
) -> PopupMenu {
    if available {
        menu.menu_with_check_icon(label, icon, checked, action)
    } else {
        menu.menu_with_check_icon_and_disabled(label, icon, checked, action, true)
    }
}

fn sort_toolbar_group_item(
    menu: gpui_component::menu::PopupMenu,
    label: impl Into<gpui::SharedString>,
    checked: bool,
    action: Box<dyn gpui::Action>,
    available: bool,
) -> gpui_component::menu::PopupMenu {
    menu.menu_with_check_and_disabled(label, checked, action, !available)
}

pub(crate) fn build_sort_prefs_menu(
    menu: PopupMenu,
    sort: SortPreferences,
    group: GroupOption,
    group_date_unit: GroupByDateUnit,
    show_hidden: bool,
    show_extensions: bool,
    include_created: bool,
    grouping_available: bool,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let mut menu = menu
        .menu_with_check_icon(
            t!("files.sort.name"),
            menu_icon(IconName::ALargeSmall),
            sort.option == SortOption::Name,
            Box::new(SortByName),
        )
        .menu_with_check_icon(
            t!("files.sort.modified"),
            menu_icon(IconName::Calendar),
            sort.option == SortOption::DateModified,
            Box::new(SortByModified),
        );
    if include_created {
        menu = menu.menu_with_check_icon(
            t!("files.sort.created"),
            menu_icon(IconName::Calendar),
            sort.option == SortOption::DateCreated,
            Box::new(SortByCreated),
        );
    }
    menu = menu
        .menu_with_check_icon(
            t!("files.sort.size"),
            menu_icon(IconName::HardDrive),
            sort.option == SortOption::Size,
            Box::new(SortBySize),
        )
        .menu_with_check_icon(
            t!("files.sort.type"),
            menu_icon(IconName::File),
            sort.option == SortOption::FileType,
            Box::new(SortByType),
        )
        .menu_with_check_icon(
            t!("files.sort.tag"),
            menu_icon(IconName::Inbox),
            sort.option == SortOption::Tag,
            Box::new(SortByTag),
        )
        .menu_with_check_icon(
            t!("files.sort.path"),
            menu_icon(IconName::Folder),
            sort.option == SortOption::Path,
            Box::new(SortByPath),
        )
        .separator()
        .menu_with_check_icon(
            t!("files.sort.ascending"),
            menu_icon(IconName::SortAscending),
            sort.direction == SortDirection::Ascending,
            Box::new(SortAscending),
        )
        .menu_with_check_icon(
            t!("files.sort.descending"),
            menu_icon(IconName::SortDescending),
            sort.direction == SortDirection::Descending,
            Box::new(SortDescending),
        )
        .separator();
    menu = sort_menu_group_item(
        menu,
        t!("files.group.none"),
        menu_icon(IconName::GalleryVerticalEnd),
        group == GroupOption::None,
        Box::new(GroupByNone),
        grouping_available,
    );
    menu = sort_menu_group_item(
        menu,
        t!("files.group.name"),
        menu_icon(IconName::ALargeSmall),
        group == GroupOption::Name,
        Box::new(GroupByName),
        grouping_available,
    );
    menu = append_date_group_submenu(
        menu,
        t!("files.group.modified"),
        GroupOption::DateModified,
        group,
        group_date_unit,
        window,
        cx,
        true,
        grouping_available,
    );
    if include_created {
        menu = append_date_group_submenu(
            menu,
            t!("files.group.created"),
            GroupOption::DateCreated,
            group,
            group_date_unit,
            window,
            cx,
            true,
            grouping_available,
        );
    }
    menu = sort_menu_group_item(
        menu,
        t!("files.group.size"),
        menu_icon(IconName::HardDrive),
        group == GroupOption::Size,
        Box::new(GroupBySize),
        grouping_available,
    );
    menu = sort_menu_group_item(
        menu,
        t!("files.group.type"),
        menu_icon(IconName::File),
        group == GroupOption::FileType,
        Box::new(GroupByType),
        grouping_available,
    );
    menu = sort_menu_group_item(
        menu,
        t!("files.group.tag"),
        menu_icon(IconName::Inbox),
        group == GroupOption::Tag,
        Box::new(GroupByTag),
        grouping_available,
    );
    menu
        .separator()
        .menu_with_check_icon(
            t!("files.show_hidden.items"),
            menu_icon(if show_hidden {
                IconName::Eye
            } else {
                IconName::EyeOff
            }),
            show_hidden,
            Box::new(ToggleShowHidden),
        )
        .menu_with_check_icon(
            t!("files.show_extensions.items"),
            menu_icon(IconName::File),
            show_extensions,
            Box::new(ToggleShowFileExtensions),
        )
}

fn append_date_group_submenu(
    menu: PopupMenu,
    label: impl Into<SharedString>,
    option: GroupOption,
    group: GroupOption,
    unit: GroupByDateUnit,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
    with_icons: bool,
    grouping_available: bool,
) -> PopupMenu {
    menu.submenu_with_icon_and_disabled(
        Some(menu_icon(IconName::Calendar)),
        label,
        !grouping_available,
        window,
        cx,
        move |menu, _, _| {
            if with_icons {
                menu.menu_with_check_icon(
                    t!("files.group.date.year"),
                    menu_icon(IconName::Calendar),
                    group == option && unit == GroupByDateUnit::Year,
                    date_group_action(option, GroupByDateUnit::Year),
                )
                .menu_with_check_icon(
                    t!("files.group.date.month"),
                    menu_icon(IconName::Calendar),
                    group == option && unit == GroupByDateUnit::Month,
                    date_group_action(option, GroupByDateUnit::Month),
                )
                .menu_with_check_icon(
                    t!("files.group.date.day"),
                    menu_icon(IconName::Calendar),
                    group == option && unit == GroupByDateUnit::Day,
                    date_group_action(option, GroupByDateUnit::Day),
                )
            } else {
                menu.menu_with_check(
                    t!("files.group.date.year"),
                    group == option && unit == GroupByDateUnit::Year,
                    date_group_action(option, GroupByDateUnit::Year),
                )
                .menu_with_check(
                    t!("files.group.date.month"),
                    group == option && unit == GroupByDateUnit::Month,
                    date_group_action(option, GroupByDateUnit::Month),
                )
                .menu_with_check(
                    t!("files.group.date.day"),
                    group == option && unit == GroupByDateUnit::Day,
                    date_group_action(option, GroupByDateUnit::Day),
                )
            }
        },
    )
}

fn date_group_action(option: GroupOption, unit: GroupByDateUnit) -> Box<dyn gpui::Action> {
    match (option, unit) {
        (GroupOption::DateModified, GroupByDateUnit::Year) => Box::new(GroupByModifiedYear),
        (GroupOption::DateModified, GroupByDateUnit::Month) => Box::new(GroupByModifiedMonth),
        (GroupOption::DateModified, GroupByDateUnit::Day) => Box::new(GroupByModifiedDay),
        (GroupOption::DateCreated, GroupByDateUnit::Year) => Box::new(GroupByCreatedYear),
        (GroupOption::DateCreated, GroupByDateUnit::Month) => Box::new(GroupByCreatedMonth),
        (GroupOption::DateCreated, GroupByDateUnit::Day) => Box::new(GroupByCreatedDay),
        _ => Box::new(GroupByNone),
    }
}

pub(crate) fn build_sort_prefs_toolbar_menu(
    menu: gpui_component::menu::PopupMenu,
    sort: SortPreferences,
    group: GroupOption,
    group_date_unit: GroupByDateUnit,
    show_hidden: bool,
    show_extensions: bool,
    include_created: bool,
    grouping_available: bool,
    window: &mut Window,
    cx: &mut Context<gpui_component::menu::PopupMenu>,
) -> gpui_component::menu::PopupMenu {
    let mut menu = menu
        .menu_with_check(
            t!("files.sort.name"),
            sort.option == SortOption::Name,
            Box::new(SortByName),
        )
        .menu_with_check(
            t!("files.sort.modified"),
            sort.option == SortOption::DateModified,
            Box::new(SortByModified),
        );
    if include_created {
        menu = menu.menu_with_check(
            t!("files.sort.created"),
            sort.option == SortOption::DateCreated,
            Box::new(SortByCreated),
        );
    }
    menu = menu
        .menu_with_check(
            t!("files.sort.size"),
            sort.option == SortOption::Size,
            Box::new(SortBySize),
        )
        .menu_with_check(
            t!("files.sort.type"),
            sort.option == SortOption::FileType,
            Box::new(SortByType),
        )
        .menu_with_check(
            t!("files.sort.tag"),
            sort.option == SortOption::Tag,
            Box::new(SortByTag),
        )
        .menu_with_check(
            t!("files.sort.path"),
            sort.option == SortOption::Path,
            Box::new(SortByPath),
        )
        .separator()
        .menu_with_check(
            t!("files.sort.ascending"),
            sort.direction == SortDirection::Ascending,
            Box::new(SortAscending),
        )
        .menu_with_check(
            t!("files.sort.descending"),
            sort.direction == SortDirection::Descending,
            Box::new(SortDescending),
        );
    menu = menu.separator();
    menu = sort_toolbar_group_item(
        menu,
        t!("files.group.none"),
        group == GroupOption::None,
        Box::new(GroupByNone),
        grouping_available,
    );
    menu = sort_toolbar_group_item(
        menu,
        t!("files.group.name"),
        group == GroupOption::Name,
        Box::new(GroupByName),
        grouping_available,
    );
    menu = append_date_group_submenu_toolbar(
        menu,
        t!("files.group.modified"),
        GroupOption::DateModified,
        group,
        group_date_unit,
        !grouping_available,
        window,
        cx,
    );
    if include_created {
        menu = append_date_group_submenu_toolbar(
            menu,
            t!("files.group.created"),
            GroupOption::DateCreated,
            group,
            group_date_unit,
            !grouping_available,
            window,
            cx,
        );
    }
    menu = sort_toolbar_group_item(
        menu,
        t!("files.group.size"),
        group == GroupOption::Size,
        Box::new(GroupBySize),
        grouping_available,
    );
    menu = sort_toolbar_group_item(
        menu,
        t!("files.group.type"),
        group == GroupOption::FileType,
        Box::new(GroupByType),
        grouping_available,
    );
    menu = sort_toolbar_group_item(
        menu,
        t!("files.group.tag"),
        group == GroupOption::Tag,
        Box::new(GroupByTag),
        grouping_available,
    );
    menu
        .separator()
        .menu_with_check(
            t!("files.show_hidden.items"),
            show_hidden,
            Box::new(ToggleShowHidden),
        )
        .menu_with_check(
            t!("files.show_extensions.items"),
            show_extensions,
            Box::new(ToggleShowFileExtensions),
        )
}

fn append_date_group_submenu_toolbar(
    menu: gpui_component::menu::PopupMenu,
    label: impl Into<gpui::SharedString>,
    option: GroupOption,
    group: GroupOption,
    unit: GroupByDateUnit,
    disabled: bool,
    window: &mut Window,
    cx: &mut Context<gpui_component::menu::PopupMenu>,
) -> gpui_component::menu::PopupMenu {
    if disabled {
        return menu.menu_with_check_and_disabled(
            label,
            group == option,
            date_group_action(option, unit),
            true,
        );
    }
    menu.submenu(label, window, cx, move |menu, _, _| {
        menu.menu_with_check(
            t!("files.group.date.year"),
            group == option && unit == GroupByDateUnit::Year,
            date_group_action(option, GroupByDateUnit::Year),
        )
        .menu_with_check(
            t!("files.group.date.month"),
            group == option && unit == GroupByDateUnit::Month,
            date_group_action(option, GroupByDateUnit::Month),
        )
        .menu_with_check(
            t!("files.group.date.day"),
            group == option && unit == GroupByDateUnit::Day,
            date_group_action(option, GroupByDateUnit::Day),
        )
    })
}

pub(super) fn paths_for_file_tag(tag_name: &str) -> Vec<PathBuf> {
    files_fs::paths_for_file_tag(tag_name)
}

/// Right-side blank strip for drag-selection in Details/List views.
pub(super) const SWEEP_GUTTER_WIDTH: Pixels = px(100.);

pub(super) fn load_files_dir(
    path: &Path,
    options: DirectoryReadOptions,
    sort: SortPreferences,
) -> (Vec<FileItem>, Option<String>) {
    match read_directory(path, options, sort) {
        Ok(items) => (items, None),
        Err(error) => (Vec::new(), Some(error.to_string())),
    }
}

pub(super) fn item_sizes_for_display_rows(
    rows: &[DisplayRow],
    mode: ViewMode,
    size_level: u8,
) -> Rc<Vec<Size<Pixels>>> {
    let item_size = row_size_for_mode(mode, size_level);
    if rows.is_empty() {
        return Rc::new(vec![item_size]);
    }
    Rc::new(
        rows.iter()
            .map(|row| match row {
                DisplayRow::GroupHeader { .. } => GROUP_HEADER_ROW_SIZE,
                DisplayRow::Item(_) => item_size,
            })
            .collect(),
    )
}

fn row_size_for_mode(mode: ViewMode, size_level: u8) -> Size<Pixels> {
    match mode {
        ViewMode::Details | ViewMode::List => match size_level {
            1 => FILE_ROW_SIZE_COMPACT,
            3 => FILE_ROW_SIZE_LARGE,
            _ => FILE_ROW_SIZE,
        },
        ViewMode::Grid => match size_level {
            1 => GRID_CELL_SIZE_SMALL,
            3 => GRID_CELL_SIZE_LARGE,
            _ => GRID_CELL_SIZE,
        },
        ViewMode::Cards => CARD_CELL_SIZE,
        ViewMode::Columns => COLUMN_ROW_SIZE,
    }
}

pub(super) fn group_date_unit_from_config(value: &str) -> GroupByDateUnit {
    match value {
        "month" => GroupByDateUnit::Month,
        "day" => GroupByDateUnit::Day,
        _ => GroupByDateUnit::Year,
    }
}

pub(super) fn group_date_unit_config_value(unit: GroupByDateUnit) -> &'static str {
    match unit {
        GroupByDateUnit::Year => "year",
        GroupByDateUnit::Month => "month",
        GroupByDateUnit::Day => "day",
    }
}

pub(super) fn group_option_from_config(value: &str) -> GroupOption {
    match value {
        "name" => GroupOption::Name,
        "modified" => GroupOption::DateModified,
        "created" => GroupOption::DateCreated,
        "size" => GroupOption::Size,
        "type" => GroupOption::FileType,
        "tag" => GroupOption::Tag,
        _ => GroupOption::None,
    }
}

pub(super) fn group_option_config_value(option: GroupOption) -> &'static str {
    match option {
        GroupOption::None => GROUP_NONE,
        GroupOption::Name => GROUP_NAME,
        GroupOption::DateModified => GROUP_MODIFIED,
        GroupOption::DateCreated => GROUP_CREATED,
        GroupOption::Size => GROUP_SIZE,
        GroupOption::FileType => GROUP_TYPE,
        GroupOption::Tag => GROUP_TAG,
    }
}

pub(super) fn column_listings_for(
    trail: &[PathBuf],
    read_options: &DirectoryReadOptions,
    sort: SortPreferences,
    query: &str,
) -> Vec<Vec<FileItem>> {
    trail
        .iter()
        .map(|path| {
            let (items, _) = load_files_dir(path, *read_options, sort);
            filter_items_by_query(&items, query)
        })
        .collect()
}

pub(super) fn drag_preview_label(paths: &[PathBuf]) -> String {
    if paths.len() == 1 {
        paths[0]
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| t!("files.type.file").to_string())
    } else {
        format!("{} {}", paths.len(), t!("files.status.items"))
    }
}

pub(super) fn sort_option_from_config(value: &str) -> SortOption {
    match value {
        "modified" => SortOption::DateModified,
        "created" => SortOption::DateCreated,
        "size" => SortOption::Size,
        "type" => SortOption::FileType,
        "path" => SortOption::Path,
        "tag" => SortOption::Tag,
        _ => SortOption::Name,
    }
}

pub(super) fn sort_direction_from_config(value: &str) -> SortDirection {
    match value {
        "desc" => SortDirection::Descending,
        _ => SortDirection::Ascending,
    }
}

pub(super) fn sort_option_config_value(option: SortOption) -> &'static str {
    match option {
        SortOption::Name => "name",
        SortOption::DateModified => "modified",
        SortOption::DateCreated => "created",
        SortOption::Size => "size",
        SortOption::FileType => "type",
        SortOption::Path => "path",
        SortOption::Tag => "tag",
    }
}

#[cfg(windows)]
pub(super) fn open_paths_in_terminal(paths: &[PathBuf]) -> anyhow::Result<()> {
    use std::path::Path;
    use std::process::Command;

    let dirs = paths
        .iter()
        .map(|path| {
            if path.is_dir() {
                Ok(path.clone())
            } else {
                path.parent()
                    .map(Path::to_path_buf)
                    .ok_or_else(|| anyhow::anyhow!("no parent directory"))
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    if dirs.is_empty() {
        return Ok(());
    }

    let mut args = Vec::with_capacity(dirs.len() * 3);
    for (index, dir) in dirs.iter().enumerate() {
        let dir = dir.to_string_lossy().to_string();
        if index > 0 {
            args.push(";".to_string());
            args.push("nt".to_string());
        }
        args.push("-d".to_string());
        args.push(dir);
    }

    let wt = Command::new("wt.exe").args(&args).spawn();
    if wt.is_ok() {
        return Ok(());
    }

    let dir = dirs[0].to_string_lossy();
    Command::new("cmd")
        .args(["/C", "start", "", "wt.exe", "-d", &dir])
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn open_paths_in_terminal(_paths: &[PathBuf]) -> anyhow::Result<()> {
    anyhow::bail!("terminal launch is only supported on Windows")
}

fn create_shortcut_at(source: &Path, link_dir: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    let file_name = source
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Item".into());
    let mut link_path = link_dir.join(format!("Shortcut to {file_name}.lnk"));
    if link_path.exists() {
        let stem = link_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Shortcut".into());
        let mut n = 2u32;
        loop {
            let candidate = link_dir.join(format!("{stem} ({n}).lnk"));
            if !candidate.exists() {
                link_path = candidate;
                break;
            }
            n += 1;
        }
    }
    let target = source.to_string_lossy().replace('\'', "''");
    let link = link_path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{link}'); $s.TargetPath='{target}'; $s.Save()"
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("powershell shortcut creation failed")
    }
}

fn create_shortcut_for_path(path: &Path) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent directory"))?;
    create_shortcut_at(path, parent)
}

pub(crate) fn create_shortcuts_in_folder(
    sources: &[PathBuf],
    destination: &Path,
) -> anyhow::Result<()> {
    if !destination.is_dir() {
        anyhow::bail!("destination is not a directory");
    }
    for path in sources {
        create_shortcut_at(path, destination)?;
    }
    Ok(())
}

pub(super) fn create_shortcuts_for_paths(paths: &[PathBuf]) -> anyhow::Result<()> {
    for path in paths {
        create_shortcut_for_path(path)?;
    }
    Ok(())
}

pub(super) fn create_desktop_shortcuts(paths: &[PathBuf]) -> anyhow::Result<()> {
    let desktop = files_fs::user_desktop_directory()
        .ok_or_else(|| anyhow::anyhow!("desktop folder not found"))?;
    create_shortcuts_in_folder(paths, &desktop)
}

pub(super) fn sort_direction_config_value(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "asc",
        SortDirection::Descending => "desc",
    }
}

pub(super) fn item_type_label(item: &FileItem) -> String {
    match item.kind {
        FileItemKind::Folder => t!("files.type.folder").to_string(),
        FileItemKind::Symlink => t!("files.type.symlink").to_string(),
        FileItemKind::Other => t!("files.type.other").to_string(),
        FileItemKind::File => item
            .extension
            .as_ref()
            .map(|extension| format!("{} file", extension.to_uppercase()))
            .unwrap_or_else(|| t!("files.type.file").to_string()),
    }
}

pub(super) fn format_size(size: Option<u64>) -> String {
    let Some(size) = size else {
        return String::new();
    };

    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0;

    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", size, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub(super) fn format_system_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return String::new();
    };

    let local_time: DateTime<Local> = time.into();
    local_time.format("%Y-%m-%d %H:%M").to_string()
}

pub(super) fn create_compress_partial_file(path: &Path) -> anyhow::Result<bool> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn open_with_system(path: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }
}

pub(super) fn open_with_cybereditor(path: &Path) -> anyhow::Result<()> {
    let Some(editor) = resolve_cybereditor_exe() else {
        return open_with_system(path);
    };
    std::process::Command::new(&editor)
        .arg(path)
        .spawn()?;
    Ok(())
}

fn resolve_cybereditor_exe() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    let dir = current.parent()?;
    #[cfg(windows)]
    let name = "cyber_editor.exe";
    #[cfg(not(windows))]
    let name = "cyber_editor";
    let sibling = dir.join(name);
    sibling.is_file().then_some(sibling)
}

pub(super) fn open_with_cybermediaplayer(path: &Path) -> anyhow::Result<()> {
    let Some(player) = resolve_cybermediaplayer_exe() else {
        return open_with_system(path);
    };
    std::process::Command::new(&player)
        .arg(path)
        .spawn()?;
    Ok(())
}

fn resolve_cybermediaplayer_exe() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    let dir = current.parent()?;
    #[cfg(windows)]
    let name = "cyber_media_player.exe";
    #[cfg(not(windows))]
    let name = "cyber_media_player";
    let sibling = dir.join(name);
    sibling.is_file().then_some(sibling)
}

pub(super) fn is_media_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let Some(ext) = ext else { return false };
    matches!(
        ext.as_str(),
        // Video
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "mpeg" | "mpg"
        | "m4v" | "3gp" | "ts" | "m2ts" | "mts" | "vob" | "ogv" | "divx" | "xvid"
        | "rm" | "rmvb" | "asf" | "dv" | "f4v" | "swf"
        // Audio
        | "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" | "opus"
        | "ape" | "wv" | "dsf" | "dff" | "aiff" | "au" | "ra" | "mid" | "midi"
        | "amr" | "ac3" | "dts" | "eac3" | "mka" | "cda"
    )
}

pub(super) fn is_executable_or_script_path(path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());
        matches!(
            ext.as_deref(),
            Some("exe")
                | Some("com")
                | Some("bat")
                | Some("cmd")
                | Some("ps1")
                | Some("vbs")
                | Some("js")
                | Some("jse")
                | Some("wsf")
        )
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub(super) fn open_paths_with_target(
    target: &Path,
    source_paths: &[PathBuf],
) -> anyhow::Result<()> {
    if source_paths.is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let ext = target
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());

        if matches!(ext.as_deref(), Some("ps1")) {
            std::process::Command::new("powershell")
                .arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(target)
                .args(source_paths)
                .spawn()
                .map(|_| ())
                .map_err(Into::into)
        } else {
            std::process::Command::new(target)
                .args(source_paths)
                .spawn()
                .map(|_| ())
                .map_err(Into::into)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("open-with-target is only supported on Windows")
    }
}
