//! Files-style content page context flyout — CyberFiles [`crate::popup_menu::PopupMenu`].

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use cyberfiles_commands::{
    CompressItems, CopyItems, CopyPath, CutItems, DeleteItems, DeleteItemsPermanent, NewFile,
    NewFolder, OpenItem, PasteItems, RefreshDirectory, RenameItem, ShellProperties, ViewCards,
    ViewColumns, ViewDetails, ViewGrid,
};
use cyberfiles_core::{context_menu_item_prefs, load_config};
use cyberfiles_fs::SortOption;
use cyberfiles_platform_windows::{self as platform, ShellContextMenuEntry};
use gpui::{px, Context, Entity, Pixels, SharedString, Window};
use gpui_component::{notification::Notification, Icon, IconName, WindowExt as _};

use crate::popup_menu::{PopupMenu, PopupMenuItem};
use rust_i18n::t;

use super::{
    normalize_paths_for_shell_cache, shell_submenu_snapshot, BrowseLocation,
    CreateFolderFromSelection, CreateShortcut, FileBrowser, OpenInNewPane, OpenInNewWindow,
    OpenInTerminal, OpenWithDialog, ShellMenuCache, ShellSubmenuSnapshot, SortByCreated,
    SortByModified, SortByName, SortBySize, SortByType, ToggleShowHidden, ToggleSortDirection,
    ViewMode,
};
use crate::app_state::{AppFileClipboard, AppNavigation};

use crate::shell::preferences::{
    assign_paths_to_file_tag, context_menu_shell_submenu, file_tags_containing_paths,
    remove_paths_from_file_tag,
};

const SHELL_MENU_MAX_HEIGHT: Pixels = px(620.);

fn menu_icon(name: IconName) -> Icon {
    Icon::new(name)
}

fn menu_action(
    menu: PopupMenu,
    label: impl Into<SharedString>,
    icon: IconName,
    action: Box<dyn gpui::Action>,
) -> PopupMenu {
    menu.menu_with_icon(label, menu_icon(icon), action)
}

fn menu_action_enabled(
    menu: PopupMenu,
    label: impl Into<SharedString>,
    icon: IconName,
    action: Box<dyn gpui::Action>,
    enabled: bool,
) -> PopupMenu {
    menu.menu_with_icon_and_disabled(label, menu_icon(icon), action, !enabled)
}

fn menu_checked_action(
    menu: PopupMenu,
    label: impl Into<SharedString>,
    icon: impl Into<Icon>,
    checked: bool,
    action: Box<dyn gpui::Action>,
) -> PopupMenu {
    menu.menu_with_check_icon(label, icon, checked, action)
}

fn menu_click_item(
    label: impl Into<SharedString>,
    icon: IconName,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> PopupMenuItem {
    PopupMenuItem::new(label.into())
        .icon(menu_icon(icon))
        .on_click(on_click)
}

fn menu_click_item_with_icon(
    label: impl Into<SharedString>,
    icon: impl Into<Icon>,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> PopupMenuItem {
    PopupMenuItem::new(label.into())
        .icon(icon.into())
        .on_click(on_click)
}

fn menu_notice_item(
    label: impl Into<SharedString>,
    icon: IconName,
    message: SharedString,
) -> PopupMenuItem {
    PopupMenuItem::new(label.into())
        .icon(menu_icon(icon))
        .on_click(move |_, window, cx| {
            window.push_notification(Notification::info(message.clone()), cx);
        })
}

fn shell_menu_item_is_properties(command_string: Option<&str>, label: &str) -> bool {
    if command_string.is_some_and(|v| v.eq_ignore_ascii_case("properties")) {
        return true;
    }
    let lower = label.to_ascii_lowercase();
    lower.contains("properties") || lower.contains("属性")
}

fn shell_menu_click_item(
    label: String,
    icon_png: Option<Vec<u8>>,
    paths: Vec<PathBuf>,
    command_offset: u32,
    command_string: Option<String>,
    extended_verbs: bool,
) -> PopupMenuItem {
    let display_label: SharedString = platform::format_shell_menu_label(&label).into();
    let is_properties = shell_menu_item_is_properties(command_string.as_deref(), &label);
    let invoke = move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| {
        let result = if is_properties {
            platform::invoke_shell_properties(&paths)
        } else {
            platform::invoke_shell_context_menu_item(&paths, command_offset, extended_verbs)
        };
        let _ = result;
    };

    let mut item = PopupMenuItem::new(display_label);
    if let Some(png) = icon_png {
        item = item.icon_png(std::sync::Arc::new(png));
    }
    item.on_click(invoke)
}

fn is_send_to_submenu_label(label: &str) -> bool {
    let lower = label.to_ascii_lowercase();
    lower.contains("send to")
        || lower.contains("发送到")
        || lower.contains("傳送到")
        || lower.contains("寄送到")
}

fn is_open_with_submenu_label(label: &str) -> bool {
    let lower = label.to_ascii_lowercase();
    lower.contains("open with")
        || lower.contains("打开方式")
        || lower.contains("開啟方式")
        || lower.contains("開啟檔案")
}

fn extract_labeled_submenu(
    entries: &[ShellContextMenuEntry],
    label_pred: fn(&str) -> bool,
) -> Vec<ShellContextMenuEntry> {
    for entry in entries {
        if let ShellContextMenuEntry::Submenu {
            label,
            children,
            lazy_parent_index,
            ..
        } = entry
        {
            if label_pred(label) {
                return resolve_submenu_entries(*lazy_parent_index, children);
            }
        }
    }
    Vec::new()
}

fn shell_feature_entries(
    cache: &std::sync::Arc<RwLock<Option<ShellMenuCache>>>,
    paths: &[PathBuf],
    extended_verbs: bool,
    label_pred: fn(&str) -> bool,
    icon_px: u32,
) -> Vec<ShellContextMenuEntry> {
    let key = normalize_paths_for_shell_cache(paths);
    let top_level = cache
        .read()
        .ok()
        .and_then(|guard| guard.as_ref().cloned())
        .filter(|cache| cache.paths == key && cache.extended_verbs == extended_verbs)
        .map(|cache| cache.entries)
        .unwrap_or_default();

    let from_cache = extract_labeled_submenu(&top_level, label_pred);
    if !from_cache.is_empty() {
        return from_cache;
    }

    let paths = paths.to_vec();
    match std::thread::spawn(move || {
        platform::query_shell_context_menu_items(&paths, extended_verbs, icon_px)
    })
    .join()
    {
        Ok(Ok(entries)) => extract_labeled_submenu(&entries, label_pred),
        Ok(Err(_error)) => Vec::new(),
        Err(_) => Vec::new(),
    }
}

fn append_remove_from_tags_submenu(
    menu: PopupMenu,
    paths: Vec<PathBuf>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let tag_names = file_tags_containing_paths(&paths);
    menu.submenu_with_icon(
        Some(Icon::new(IconName::Inbox).path("icons/label_off.svg")),
        t!("files.menu.remove_from_tag"),
        window,
        cx,
        move |sub, _window, _cx| {
            let tag_names = tag_names.clone();
            if tag_names.is_empty() {
                sub.item(PopupMenuItem::new(t!("files.menu.not_in_file_tags")).disabled(true))
            } else {
                let mut sub = sub;
                for name in tag_names {
                    let paths_for_tag = paths.clone();
                    sub = sub.item(PopupMenuItem::new(name.clone()).on_click(move |_, _, cx| {
                        remove_paths_from_file_tag(&name, &paths_for_tag, cx);
                    }));
                }
                sub
            }
        },
    )
}

fn append_file_tags_submenu(
    menu: PopupMenu,
    paths: Vec<PathBuf>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let tags = load_config().map(|c| c.file_tags).unwrap_or_default();
    let tag_names: Vec<String> = tags.iter().map(|t| t.name.clone()).collect();
    menu.submenu_with_icon(
        Some(Icon::new(IconName::Inbox).path("icons/label.svg")),
        t!("files.menu.add_to_tag"),
        window,
        cx,
        move |sub, _window, _cx| {
            let tag_names = tag_names.clone();
            if tag_names.is_empty() {
                sub.item(PopupMenuItem::new(t!("files.menu.no_file_tags")).disabled(true))
            } else {
                let mut sub = sub;
                for name in tag_names {
                    let paths_for_tag = paths.clone();
                    sub = sub.item(PopupMenuItem::new(name.clone()).on_click(move |_, _, cx| {
                        assign_paths_to_file_tag(&name, &paths_for_tag, cx);
                    }));
                }
                sub
            }
        },
    )
}

fn append_send_to_submenu(
    menu: PopupMenu,
    children: &[ShellContextMenuEntry],
    paths: &[PathBuf],
    extended_verbs: bool,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let paths_for_sub = paths.to_vec();
    let browser_sub = browser.clone();
    let children_stash = children.to_vec();
    menu.submenu_with_icon(
        Some(menu_icon(IconName::ExternalLink)),
        t!("files.menu.send_to"),
        window,
        cx,
        move |sub, window, cx| {
            let loaded = resolve_submenu_entries(None, &children_stash);
            if loaded.is_empty() {
                sub.item(PopupMenuItem::new(t!("files.menu.shell_empty")).disabled(true))
            } else {
                append_shell_entries(
                    sub,
                    &loaded,
                    &paths_for_sub,
                    extended_verbs,
                    browser_sub.clone(),
                    window,
                    cx,
                )
            }
        },
    )
}

fn append_open_with_submenu(
    menu: PopupMenu,
    children: &[ShellContextMenuEntry],
    paths: &[PathBuf],
    extended_verbs: bool,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let paths_for_sub = paths.to_vec();
    let browser_sub = browser.clone();
    let children_stash = children.to_vec();
    let choose_path = paths[0].clone();
    menu.submenu_with_icon(
        Some(Icon::new(IconName::Settings2).path("icons/widgets.svg")),
        t!("files.menu.open_with"),
        window,
        cx,
        move |sub, window, cx| {
            let loaded = resolve_submenu_entries(None, &children_stash);
            let sub = if loaded.is_empty() {
                sub.item(PopupMenuItem::new(t!("files.menu.shell_empty")).disabled(true))
            } else {
                append_shell_entries(
                    sub,
                    &loaded,
                    &paths_for_sub,
                    extended_verbs,
                    browser_sub.clone(),
                    window,
                    cx,
                )
            };
            sub.item(
                PopupMenuItem::new(t!("files.menu.open_with_choose"))
                    .icon(Icon::new(IconName::Settings2).path("icons/widgets.svg"))
                    .on_click({
                        let choose_path = choose_path.clone();
                        move |_, _, _| {
                            let _ = platform::show_open_with_dialog(&choose_path);
                        }
                    }),
            )
        },
    )
}

fn resolve_submenu_entries(
    lazy_parent_index: Option<u32>,
    children: &[ShellContextMenuEntry],
) -> Vec<ShellContextMenuEntry> {
    if let Some(index) = lazy_parent_index {
        match std::thread::spawn(move || platform::load_lazy_submenu(index)).join() {
            Ok(Ok(items)) => items,
            Ok(Err(_error)) => Vec::new(),
            Err(_) => Vec::new(),
        }
    } else {
        children.to_vec()
    }
}

fn entries_contain_submenu(entries: &[ShellContextMenuEntry]) -> bool {
    entries
        .iter()
        .any(|entry| matches!(entry, ShellContextMenuEntry::Submenu { .. }))
}

fn append_shell_flat_items(
    mut menu: PopupMenu,
    flat_entries: &[ShellContextMenuEntry],
    paths: &[PathBuf],
    extended_verbs: bool,
) -> PopupMenu {
    for entry in flat_entries {
        match entry {
            ShellContextMenuEntry::Separator => {
                menu = menu.separator();
            }
            ShellContextMenuEntry::Item {
                label,
                command_offset,
                command_string,
                icon_png,
                ..
            } => {
                menu = menu.item(shell_menu_click_item(
                    label.clone(),
                    icon_png.clone(),
                    paths.to_vec(),
                    *command_offset,
                    command_string.clone(),
                    extended_verbs,
                ));
            }
            ShellContextMenuEntry::Submenu { .. } => {}
        }
    }
    menu
}

fn append_shell_submenu(
    menu: PopupMenu,
    label: String,
    children: &[ShellContextMenuEntry],
    lazy_parent_index: Option<u32>,
    paths: &[PathBuf],
    extended_verbs: bool,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let display_label = platform::format_shell_menu_label(&label);
    let log_label = display_label.clone();
    let paths_for_sub = paths.to_vec();
    let browser_sub = browser.clone();
    let lazy_index = lazy_parent_index;
    let children_stash = children.to_vec();
    menu.submenu(display_label, window, cx, move |sub, window, cx| {
        let loaded = resolve_submenu_entries(lazy_index, &children_stash);
        let _ = &log_label;
        if loaded.is_empty() {
            sub.item(PopupMenuItem::new(t!("files.menu.shell_empty")).disabled(true))
        } else {
            append_shell_entries(
                sub,
                &loaded,
                &paths_for_sub,
                extended_verbs,
                browser_sub.clone(),
                window,
                cx,
            )
        }
    })
}

pub(crate) fn append_shell_entries(
    mut menu: PopupMenu,
    entries: &[ShellContextMenuEntry],
    paths: &[PathBuf],
    extended_verbs: bool,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let browser = browser.clone();
    if entries_contain_submenu(entries) {
        let mut flat_batch = Vec::new();
        for entry in entries {
            match entry {
                ShellContextMenuEntry::Submenu {
                    label,
                    children,
                    lazy_parent_index,
                    ..
                } => {
                    if !flat_batch.is_empty() {
                        menu = append_shell_flat_items(menu, &flat_batch, paths, extended_verbs);
                        flat_batch.clear();
                    }
                    menu = append_shell_submenu(
                        menu,
                        label.clone(),
                        children,
                        *lazy_parent_index,
                        paths,
                        extended_verbs,
                        browser.clone(),
                        window,
                        cx,
                    );
                }
                _ => flat_batch.push(entry.clone()),
            }
        }
        if !flat_batch.is_empty() {
            menu = append_shell_flat_items(menu, &flat_batch, paths, extended_verbs);
        }
        menu.max_h(SHELL_MENU_MAX_HEIGHT)
    } else {
        for entry in entries {
            match entry {
                ShellContextMenuEntry::Separator => {
                    menu = menu.separator();
                }
                ShellContextMenuEntry::Item {
                    label,
                    command_offset,
                    command_string,
                    icon_png,
                    ..
                } => {
                    menu = menu.item(shell_menu_click_item(
                        label.clone(),
                        icon_png.clone(),
                        paths.to_vec(),
                        *command_offset,
                        command_string.clone(),
                        extended_verbs,
                    ));
                }
                ShellContextMenuEntry::Submenu { .. } => {}
            }
        }
        menu.scrollable(true).max_h(SHELL_MENU_MAX_HEIGHT)
    }
}

fn append_show_more_options(
    menu: PopupMenu,
    paths: Vec<PathBuf>,
    extended_verbs: bool,
    shell_menu_cache: Arc<RwLock<Option<super::ShellMenuCache>>>,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let paths_for_sub = paths.clone();
    menu.submenu_with_icon(
        Some(menu_icon(IconName::Ellipsis)),
        t!("files.menu.show_more_options"),
        window,
        cx,
        move |sub, window, cx| match shell_submenu_snapshot(
            &shell_menu_cache,
            &paths_for_sub,
            extended_verbs,
        ) {
            ShellSubmenuSnapshot::Loading => {
                sub.item(PopupMenuItem::new(t!("files.menu.shell_loading")).disabled(true))
            }
            ShellSubmenuSnapshot::Empty => {
                sub.item(PopupMenuItem::new(t!("files.menu.shell_empty")).disabled(true))
            }
            ShellSubmenuSnapshot::Ready(entries) => append_shell_entries(
                sub,
                &entries,
                &paths_for_sub,
                extended_verbs,
                browser.clone(),
                window,
                cx,
            ),
        },
    )
}

fn append_clipboard_commands(menu: PopupMenu, has_selection: bool, can_paste: bool) -> PopupMenu {
    let menu = menu.menu_with_icon_and_disabled(
        t!("files.menu.cut"),
        Icon::new(IconName::Replace).path("icons/content_cut.svg"),
        Box::new(CutItems),
        !has_selection,
    );
    let menu = menu_action_enabled(
        menu,
        t!("files.menu.copy"),
        IconName::Copy,
        Box::new(CopyItems),
        has_selection,
    );
    let menu = menu.menu_with_icon_and_disabled(
        t!("files.menu.paste"),
        Icon::new(IconName::Replace).path("icons/content_paste.svg"),
        Box::new(PasteItems),
        !can_paste,
    );
    let menu = menu.menu_with_icon_and_disabled(
        t!("files.menu.rename"),
        Icon::new(IconName::File).path("icons/drive_file_rename_outline.svg"),
        Box::new(RenameItem),
        !has_selection,
    );
    let menu = menu_action_enabled(
        menu,
        t!("files.menu.delete"),
        IconName::Delete,
        Box::new(DeleteItems),
        has_selection,
    );
    menu_action_enabled(
        menu,
        t!("files.menu.properties"),
        IconName::Info,
        Box::new(ShellProperties),
        has_selection,
    )
}

/// Build the file list context menu (background or item flyout).
pub fn build_context_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    if browser.read(cx).is_background_context_menu() {
        build_background_menu(menu, browser, window, cx)
    } else {
        build_item_menu(menu, browser, window, cx)
    }
}

fn build_background_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let can_paste = AppFileClipboard::has_items(cx);
    let state = browser.read(cx);
    let focus = state.focus_handle.clone();
    let in_recycle = state.browse_location == BrowseLocation::RecycleBin;
    let in_file_tag = matches!(state.browse_location, BrowseLocation::FileTag { .. });

    let browser_layout = browser.clone();
    let browser_sort = browser.clone();
    let browser_new = browser.clone();

    let mut menu = menu.action_context(focus);

    if !in_recycle && !in_file_tag {
        menu = menu.menu_with_icon_and_disabled(
            t!("files.menu.paste"),
            Icon::new(IconName::Replace).path("icons/content_paste.svg"),
            Box::new(PasteItems),
            !can_paste,
        )
        .separator();
    }

    menu = menu.submenu_with_icon(
        Some(menu_icon(IconName::LayoutDashboard)),
        t!("files.menu.layout"),
        window,
        cx,
        move |menu, _, cx| {
            let focus = browser_layout.read(cx).focus_handle.clone();
            let view_mode = browser_layout.read(cx).view_mode;
            let mut menu = menu.action_context(focus);
            menu = menu_checked_action(
                menu,
                t!("files.view.details"),
                Icon::new(IconName::GalleryVerticalEnd).path("icons/view_headline.svg"),
                view_mode == ViewMode::Details,
                Box::new(ViewDetails),
            );
            menu = menu_checked_action(
                menu,
                t!("files.view.grid"),
                IconName::LayoutDashboard,
                view_mode == ViewMode::Grid,
                Box::new(ViewGrid),
            );
            menu = menu_checked_action(
                menu,
                t!("files.view.cards"),
                IconName::LayoutDashboard,
                view_mode == ViewMode::Cards,
                Box::new(ViewCards),
            );
            menu_checked_action(
                menu,
                t!("files.view.columns"),
                IconName::PanelLeft,
                view_mode == ViewMode::Columns,
                Box::new(ViewColumns),
            )
        },
    );

    menu = menu.submenu_with_icon(
        Some(menu_icon(IconName::ChevronsUpDown)),
        t!("files.menu.sort"),
        window,
        cx,
        move |menu, _, cx| {
            let state = browser_sort.read(cx);
            let focus = state.focus_handle.clone();
            let sort = state.sort_preferences;
            let show_hidden = state.read_options.show_hidden_items;
            let hidden_label = if show_hidden {
                t!("files.show_hidden.off")
            } else {
                t!("files.show_hidden.on")
            };
            let hidden_icon = if show_hidden {
                IconName::EyeOff
            } else {
                IconName::Eye
            };
            menu.action_context(focus)
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
                )
                .menu_with_check_icon(
                    t!("files.sort.created"),
                    menu_icon(IconName::Calendar),
                    sort.option == SortOption::DateCreated,
                    Box::new(SortByCreated),
                )
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
                .separator()
                .menu_with_icon(
                    t!("files.sort.toggle_direction"),
                    menu_icon(IconName::ChevronsUpDown),
                    Box::new(ToggleSortDirection),
                )
                .menu_with_icon(
                    hidden_label,
                    menu_icon(hidden_icon),
                    Box::new(ToggleShowHidden),
                )
        },
    );

    if !in_recycle && !in_file_tag {
        menu = menu.separator().submenu_with_icon(
            Some(menu_icon(IconName::Plus)),
            t!("files.menu.new"),
            window,
            cx,
            move |menu, _, cx| {
                let focus = browser_new.read(cx).focus_handle.clone();
                menu.action_context(focus)
                    .item(
                        PopupMenuItem::new(t!("files.new_folder"))
                            .icon(menu_icon(IconName::Folder))
                            .action(Box::new(NewFolder)),
                    )
                    .item(
                        PopupMenuItem::new(t!("files.new_file"))
                            .icon(menu_icon(IconName::File))
                            .action(Box::new(NewFile)),
                    )
            },
        );
    }

    menu_action(
        menu,
        t!("files.menu.refresh"),
        IconName::Replace,
        Box::new(RefreshDirectory),
    )
}

fn build_item_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    match browser.read(cx).browse_location.clone() {
        BrowseLocation::RecycleBin => build_recycle_item_menu(menu, browser, window, cx),
        BrowseLocation::FileTag { .. } => build_file_tag_item_menu(menu, browser, window, cx),
        BrowseLocation::Directory => build_directory_item_menu(menu, browser, window, cx),
    }
}

fn build_directory_item_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let can_paste = AppFileClipboard::has_items(cx);
    let state = browser.read(cx);
    let paths = state.selected_paths_vec();
    let has_selection = !paths.is_empty();
    let single = paths.len() == 1;
    let single_dir = single && paths[0].is_dir();
    let multi = paths.len() > 1;
    let all_dirs = has_selection && paths.iter().all(|path| path.is_dir());
    let has_shortcut = paths.iter().any(|path| {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
    });
    let focus = state.focus_handle.clone();
    let extended = state.context_menu_extended_verbs;
    let shell_menu_cache = state.shell_menu_cache.clone();
    let menu_icon_px = platform::menu_icon_pixel_size(window.scale_factor());
    let item_prefs = context_menu_item_prefs();

    let mut menu = menu.action_context(focus);
    menu = append_clipboard_commands(menu, has_selection, can_paste);
    menu = menu.separator();

    menu = menu_action(
        menu,
        t!("files.menu.open"),
        IconName::FolderOpen,
        Box::new(OpenItem),
    );

    if single && !paths[0].is_dir() {
        let open_with_children = shell_feature_entries(
            &shell_menu_cache,
            &paths,
            extended,
            is_open_with_submenu_label,
            menu_icon_px,
        );
        if open_with_children.is_empty() {
            menu = menu.menu_with_icon(
                t!("files.menu.open_with"),
                Icon::new(IconName::Settings2).path("icons/widgets.svg"),
                Box::new(OpenWithDialog),
            );
        } else {
            menu = append_open_with_submenu(
                menu,
                &open_with_children,
                &paths,
                extended,
                browser.clone(),
                window,
                cx,
            );
        }
    }

    if single {
        let path = paths[0].clone();
        let tab_path = path.clone();
        menu = menu.item(menu_click_item_with_icon(
            t!("sidebar.menu.open_new_tab"),
            Icon::new(IconName::File).path("icons/tab.svg"),
            move |_, _, cx| AppNavigation::open_path_in_new_tab(tab_path.clone(), cx),
        ));
        menu = menu.menu_with_icon(
            t!("files.menu.open_in_new_window"),
            Icon::new(IconName::ExternalLink).path("icons/external-link.svg"),
            Box::new(OpenInNewWindow),
        );
        menu = menu.menu_with_icon(
            t!("files.menu.open_in_new_pane"),
            Icon::new(IconName::PanelLeftOpen).path("icons/splitscreen.svg"),
            Box::new(OpenInNewPane),
        );
    }

    menu = menu.separator();
    menu = menu_action(
        menu,
        t!("files.menu.copy_path"),
        IconName::Copy,
        Box::new(CopyPath),
    );

    if multi {
        menu = menu_action(
            menu,
            t!("files.menu.create_folder_from_selection"),
            IconName::Folder,
            Box::new(CreateFolderFromSelection),
        );
    }

    if item_prefs.create_shortcut && has_selection && !has_shortcut {
        menu = menu_action(
            menu,
            t!("files.menu.create_shortcut"),
            IconName::ExternalLink,
            Box::new(CreateShortcut),
        );
    }

    if item_prefs.compress {
        menu = menu.menu_with_icon(
            t!("files.menu.compress"),
            Icon::new(IconName::File).path("icons/folder_zip.svg"),
            Box::new(CompressItems),
        );
    }

    let not_implemented: SharedString = t!("files.menu.not_implemented").into();

    if item_prefs.send_to && has_selection {
        let send_to_children = shell_feature_entries(
            &shell_menu_cache,
            &paths,
            extended,
            is_send_to_submenu_label,
            menu_icon_px,
        );
        if send_to_children.is_empty() {
            menu = menu.item(menu_notice_item(
                t!("files.menu.send_to"),
                IconName::ExternalLink,
                not_implemented.clone(),
            ));
        } else {
            menu = append_send_to_submenu(
                menu,
                &send_to_children,
                &paths,
                extended,
                browser.clone(),
                window,
                cx,
            );
        }
    }

    if item_prefs.pin && single_dir {
        let path = paths[0].clone();
        let path_string = path.to_string_lossy().to_string();
        let pin_label = if path_is_pinned(&path_string) {
            t!("sidebar.menu.unpin")
        } else {
            t!("sidebar.menu.pin")
        };
        if path_is_pinned(&path_string) {
            let ps = path_string.clone();
            menu = menu.item(
                PopupMenuItem::new(pin_label)
                    .icon(menu_icon(IconName::StarOff))
                    .on_click(move |_, _, cx| AppNavigation::unpin_folder(&ps, cx)),
            );
        } else if path.exists() {
            let pin_path = path.clone();
            menu = menu.item(
                PopupMenuItem::new(pin_label)
                    .icon(menu_icon(IconName::Star))
                    .on_click(move |_, _, cx| AppNavigation::pin_folder(pin_path.clone(), cx)),
            );
        }
    }

    if single {
        if let Some(parent) = paths[0].parent() {
            let loc_parent = parent.to_path_buf();
            menu = menu.item(menu_click_item(
                t!("files.menu.open_file_location"),
                IconName::FolderOpen,
                move |_, _, cx| AppNavigation::navigate_to_path(loc_parent.clone(), cx),
            ));
        }
    }

    if item_prefs.open_in_terminal && all_dirs {
        menu = menu.separator();
        menu = menu_action(
            menu,
            t!("files.menu.open_in_terminal"),
            IconName::SquareTerminal,
            Box::new(OpenInTerminal),
        );
    }
    if item_prefs.file_tags {
        menu = menu.separator();
        menu = append_file_tags_submenu(menu, paths.clone(), window, cx);
        menu = append_remove_from_tags_submenu(menu, paths.clone(), window, cx);
    }
    menu = menu.separator();

    if has_selection {
        if context_menu_shell_submenu(cx) {
            menu = append_show_more_options(
                menu,
                paths,
                extended,
                shell_menu_cache,
                browser,
                window,
                cx,
            );
        } else {
            menu =
                append_inline_shell_extensions(menu, paths, shell_menu_cache, browser, window, cx);
        }
    }

    menu
}

fn append_inline_shell_extensions(
    menu: PopupMenu,
    paths: Vec<PathBuf>,
    shell_menu_cache: Arc<RwLock<Option<super::ShellMenuCache>>>,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    match shell_submenu_snapshot(&shell_menu_cache, &paths, false) {
        ShellSubmenuSnapshot::Loading => {
            menu.item(PopupMenuItem::new(t!("files.menu.shell_loading")).disabled(true))
        }
        ShellSubmenuSnapshot::Empty => menu,
        ShellSubmenuSnapshot::Ready(entries) => {
            append_shell_entries(menu, &entries, &paths, false, browser, window, cx)
        }
    }
}

fn build_recycle_item_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    _window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let can_paste = AppFileClipboard::has_items(cx);
    let state = browser.read(cx);
    let has_selection = !state.selected_paths.is_empty();
    let focus = state.focus_handle.clone();

    let mut menu = menu.action_context(focus);
    menu = menu_action(
        menu,
        t!("files.menu.open"),
        IconName::FolderOpen,
        Box::new(OpenItem),
    );
    menu = menu.separator();
    menu = menu_action_enabled(
        menu,
        t!("files.menu.copy"),
        IconName::Copy,
        Box::new(CopyItems),
        has_selection,
    );
    menu = menu.separator();
    menu = menu_action_enabled(
        menu,
        t!("files.menu.delete_permanent"),
        IconName::Delete,
        Box::new(DeleteItemsPermanent),
        has_selection,
    );
    menu = menu_action(
        menu,
        t!("files.menu.properties"),
        IconName::Info,
        Box::new(ShellProperties),
    );
    menu.menu_with_icon_and_disabled(
        t!("files.menu.paste"),
        Icon::new(IconName::Replace).path("icons/content_paste.svg"),
        Box::new(PasteItems),
        !can_paste,
    )
}

fn build_file_tag_item_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    _window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let state = browser.read(cx);
    let paths = state.selected_paths_vec();
    let single_dir = paths.len() == 1 && paths[0].is_dir();
    let focus = state.focus_handle.clone();

    let mut menu = menu.action_context(focus);
    menu = menu_action(
        menu,
        t!("files.menu.open"),
        IconName::FolderOpen,
        Box::new(OpenItem),
    );

    if single_dir {
        let path = paths[0].clone();
        menu = menu.item(menu_click_item_with_icon(
            t!("sidebar.menu.open_new_tab"),
            Icon::new(IconName::Plus).path("icons/tab.svg"),
            move |_, _, cx| AppNavigation::open_path_in_new_tab(path.clone(), cx),
        ));
    }

    menu = menu_action(
        menu,
        t!("files.menu.copy_path"),
        IconName::Copy,
        Box::new(CopyPath),
    );
    menu_action(
        menu,
        t!("files.menu.properties"),
        IconName::Info,
        Box::new(ShellProperties),
    )
}

fn path_is_pinned(path_string: &str) -> bool {
    load_config()
        .map(|c| c.pinned_folders.iter().any(|p| p == path_string))
        .unwrap_or(false)
}

impl FileBrowser {
    pub(crate) fn is_background_context_menu(&self) -> bool {
        if !self.selected_paths.is_empty() {
            return false;
        }
        matches!(
            self.browse_location,
            BrowseLocation::Directory | BrowseLocation::RecycleBin | BrowseLocation::FileTag { .. }
        )
    }

    pub(crate) fn set_context_menu_extended_verbs(&mut self, extended: bool) {
        self.context_menu_extended_verbs = extended;
        let mismatch = self
            .shell_menu_cache
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|c| c.extended_verbs != extended))
            .unwrap_or(false);
        if mismatch {
            if let Ok(mut guard) = self.shell_menu_cache.write() {
                *guard = None;
            }
            self.shell_menu_revision = self.shell_menu_revision.wrapping_add(1);
        }
    }
}
