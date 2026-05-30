use std::path::PathBuf;

use cyber_desktop_core::{self, save_config, AppConfig, WINDOW_HEIGHT, WINDOW_WIDTH};
use gpui::{px, App, SharedString};
use gpui_component::{scroll::ScrollbarShow, ActiveTheme as _, Theme, ThemeMode};

use crate::theme::{self, ThemeCatalog};

use crate::i18n;

fn persist_preferences(cx: &mut App) {
    let (window_width, window_height) = window_size_from_active(cx).unwrap_or_else(|| {
        cyber_desktop_core::load_config()
            .map(|c| (c.window_width, c.window_height))
            .unwrap_or((WINDOW_WIDTH, WINDOW_HEIGHT))
    });
    let _ = save_config(&capture_config(cx, window_width, window_height));
}

/// Reads the active window size in pixels (for persistence).
pub fn window_size_from_active(cx: &mut App) -> Option<(f32, f32)> {
    let window = cx.active_window()?;
    let mut size = None;
    let _ = window.update(cx, |_, window, _| {
        let bounds = window.window_bounds().get_bounds();
        size = Some((bounds.size.width.as_f32(), bounds.size.height.as_f32()));
    });
    size
}

/// Saves current window bounds into `settings.json` (theme fields unchanged).
pub fn persist_window_bounds(cx: &mut App) {
    let Some((window_width, window_height)) = window_size_from_active(cx) else {
        return;
    };
    if let Some(nav) = cx.try_global::<crate::app_state::AppNavigation>() {
        let page = nav.main_page();
        let _ = page.update(cx, |page, cx| page.persist_session(cx));
    }
    let _ = save_config(&capture_config(cx, window_width, window_height));
}

pub fn apply_locale(locale: &str, cx: &mut App) {
    i18n::set_locale(locale);
    super::app_menus::reload(cx);
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn apply_theme_mode(mode: ThemeMode, cx: &mut App) {
    let set_id = theme::current_theme_set_id(cx);
    theme::apply_set(set_id.as_ref(), mode, cx);
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn apply_theme_name(name: SharedString, cx: &mut App) {
    let mode = Theme::global(cx).mode;
    theme::apply_set(name.as_ref(), mode, cx);
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn apply_font_size(size: f32, cx: &mut App) {
    Theme::global_mut(cx).font_size = px(size);
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn apply_border_radius(radius: f32, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.radius = px(radius);
    theme.radius_lg = if theme.radius > px(0.) {
        theme.radius + px(2.)
    } else {
        px(0.)
    };
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn apply_scrollbar_show(show: ScrollbarShow, cx: &mut App) {
    Theme::global_mut(cx).scrollbar_show = show;
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn set_list_active_highlight(enabled: bool, cx: &mut App) {
    Theme::global_mut(cx).list.active_highlight = enabled;
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn current_locale(_cx: &App) -> SharedString {
    i18n::locale().to_string().into()
}

pub fn scrollbar_show_key(show: ScrollbarShow) -> SharedString {
    match show {
        ScrollbarShow::Scrolling => "scrolling".into(),
        ScrollbarShow::Hover => "hover".into(),
        ScrollbarShow::Always => "always".into(),
    }
}

fn mutate_config(cx: &mut App, mutate: impl FnOnce(&mut AppConfig)) {
    let mut config = cyber_desktop_core::load_config().unwrap_or_default();
    mutate(&mut config);
    let _ = save_config(&config);
    refresh_main_page_sidebar(cx);
    cx.refresh_windows();
}

fn refresh_main_page_sidebar(cx: &mut App) {
    let Some(nav) = cx.try_global::<crate::app_state::AppNavigation>() else {
        return;
    };
    let page = nav.main_page();
    let _ = page.update(cx, |page, cx| page.refresh_sidebar_cache(cx));
}

pub fn sidebar_display_mode(_cx: &App) -> SharedString {
    cyber_desktop_core::load_config()
        .map(|c| c.sidebar_display_mode.into())
        .unwrap_or_else(|| "expanded".into())
}

pub fn apply_sidebar_display_mode(mode: SharedString, cx: &mut App) {
    mutate_config(cx, |config| {
        config.sidebar_display_mode = mode.to_string();
    });
}

pub fn sidebar_section_pinned(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_sidebar_section_pinned)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_pinned(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_pinned = enabled);
}

pub fn sidebar_section_library(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_sidebar_section_library)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_library(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_library = enabled);
}

pub fn sidebar_section_drives(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_sidebar_section_drives)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_drives(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_drives = enabled);
}

pub fn sidebar_section_cloud(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_sidebar_section_cloud)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_cloud(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_cloud = enabled);
}

pub fn sidebar_section_network(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_sidebar_section_network)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_network(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_network = enabled);
}

pub fn sidebar_section_wsl(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_sidebar_section_wsl)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_wsl(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_wsl = enabled);
}

pub fn sidebar_section_file_tags(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_sidebar_section_file_tags)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_file_tags(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_file_tags = enabled);
}

pub fn add_file_tag(name: SharedString, cx: &mut App) {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    mutate_config(cx, |c| {
        if c.file_tags.iter().any(|tag| tag.name == trimmed) {
            return;
        }
        c.file_tags.push(cyber_desktop_core::FileTagConfig {
            name: trimmed,
            color: None,
            paths: Vec::new(),
        });
    });
    refresh_home_if_active(cx);
}

pub fn assign_paths_to_file_tag(tag_name: &str, paths: &[PathBuf], cx: &mut App) {
    if paths.is_empty() {
        return;
    }
    let path_strings: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    mutate_config(cx, |c| {
        let Some(tag) = c.file_tags.iter_mut().find(|t| t.name == tag_name) else {
            return;
        };
        for path in path_strings {
            if !tag.paths.iter().any(|p| p == &path) {
                tag.paths.push(path);
            }
        }
    });
    refresh_home_if_active(cx);
    refresh_file_tag_ui(tag_name, cx);
}

pub fn remove_file_tag(name: &str, cx: &mut App) {
    mutate_config(cx, |c| {
        c.file_tags.retain(|tag| tag.name != name);
    });
    refresh_home_if_active(cx);
}

pub fn remove_paths_from_file_tag(tag_name: &str, paths: &[PathBuf], cx: &mut App) {
    if paths.is_empty() {
        return;
    }
    let path_strings: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    mutate_config(cx, |c| {
        let Some(tag) = c.file_tags.iter_mut().find(|t| t.name == tag_name) else {
            return;
        };
        tag.paths.retain(|p| !path_strings.iter().any(|s| s == p));
    });
    refresh_home_if_active(cx);
    refresh_file_tag_ui(tag_name, cx);
}

fn refresh_file_tag_ui(tag_name: &str, cx: &mut App) {
    if let Some(nav) = cx.try_global::<crate::app_state::AppNavigation>() {
        let page = nav.main_page();
        let name = tag_name.to_string();
        let _ = page.update(cx, |page, cx| {
            page.refresh_sidebar_cache(cx);
            page.reload_file_tag_browsers(&name, cx);
        });
    }
}

/// Tag names that contain at least one of `paths` (canonical string match).
pub fn file_tags_containing_paths(paths: &[PathBuf]) -> Vec<String> {
    if paths.is_empty() {
        return Vec::new();
    }
    let path_strings: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    cyber_desktop_core::load_config()
        .map(|c| {
            c.file_tags
                .iter()
                .filter(|tag| {
                    tag.paths
                        .iter()
                        .any(|p| path_strings.iter().any(|s| s == p))
                })
                .map(|tag| tag.name.clone())
                .collect()
        })
        .unwrap_or_default()
}

pub fn context_menu_shell_submenu(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.context_menu_shell_extensions_submenu)
        .unwrap_or(true)
}

pub fn apply_context_menu_shell_submenu(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.context_menu_shell_extensions_submenu = enabled);
}

pub fn context_menu_show_compress(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.context_menu_show_compress)
        .unwrap_or(true)
}

pub fn apply_context_menu_show_compress(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.context_menu_show_compress = enabled);
}

pub fn context_menu_show_send_to(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.context_menu_show_send_to)
        .unwrap_or(true)
}

pub fn apply_context_menu_show_send_to(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.context_menu_show_send_to = enabled);
}

pub fn context_menu_show_pin(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.context_menu_show_pin)
        .unwrap_or(true)
}

pub fn apply_context_menu_show_pin(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.context_menu_show_pin = enabled);
}

pub fn context_menu_show_open_in_terminal(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.context_menu_show_open_in_terminal)
        .unwrap_or(true)
}

pub fn apply_context_menu_show_open_in_terminal(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.context_menu_show_open_in_terminal = enabled);
}

pub fn context_menu_show_file_tags(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.context_menu_show_file_tags)
        .unwrap_or(true)
}

pub fn apply_context_menu_show_file_tags(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.context_menu_show_file_tags = enabled);
}

pub fn context_menu_show_create_shortcut(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.context_menu_show_create_shortcut)
        .unwrap_or(true)
}

pub fn apply_context_menu_show_create_shortcut(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.context_menu_show_create_shortcut = enabled);
}

pub fn home_widget_quick_access(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_home_quick_access)
        .unwrap_or(true)
}

pub fn apply_home_widget_quick_access(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_home_quick_access = enabled);
    refresh_home_if_active(cx);
}

pub fn home_widget_drives(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_home_drives)
        .unwrap_or(true)
}

pub fn apply_home_widget_drives(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_home_drives = enabled);
    refresh_home_if_active(cx);
}

pub fn home_widget_network(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_home_network)
        .unwrap_or(true)
}

pub fn apply_home_widget_network(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_home_network = enabled);
    refresh_home_if_active(cx);
}

pub fn home_widget_file_tags(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_home_file_tags)
        .unwrap_or(true)
}

pub fn apply_home_widget_file_tags(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_home_file_tags = enabled);
    refresh_home_if_active(cx);
}

pub fn home_widget_recent(_cx: &App) -> bool {
    cyber_desktop_core::load_config()
        .map(|c| c.show_home_recent)
        .unwrap_or(true)
}

pub fn apply_home_widget_recent(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_home_recent = enabled);
    refresh_home_if_active(cx);
}

pub fn open_text_with_cybereditor(_cx: &App) -> bool {
    cyber_desktop_core::open_text_with_cybereditor_enabled()
}

pub fn apply_open_text_with_cybereditor(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.open_text_with_cybereditor = enabled);
}

fn refresh_home_if_active(cx: &mut App) {
    if let Some(nav) = cx.try_global::<crate::app_state::AppNavigation>() {
        let page = nav.main_page();
        let _ = page.update(cx, |page, cx| page.refresh_home_widgets(cx));
    }
}

pub fn scrollbar_show_from_key(key: &str) -> ScrollbarShow {
    match key {
        "hover" => ScrollbarShow::Hover,
        "always" => ScrollbarShow::Always,
        _ => ScrollbarShow::Scrolling,
    }
}

/// Apply saved settings at startup (before the window and app menus exist).
pub fn apply_config(config: &AppConfig, cx: &mut App) {
    i18n::set_locale(&config.locale);
    theme::apply_from_config(config, cx);
}

pub fn capture_config(cx: &App, window_width: f32, window_height: f32) -> AppConfig {
    let prior = cyber_desktop_core::load_config().unwrap_or_default();
    AppConfig {
        locale: i18n::locale().to_string(),
        dark_mode: cx.theme().mode.is_dark(),
        theme_name: theme::current_theme_set_id(cx).to_string(),
        font_size: cx.theme().font_size.as_f32(),
        border_radius: cx.theme().radius.as_f32(),
        scrollbar_show: scrollbar_show_key(cx.theme().scrollbar_show).to_string(),
        list_active_highlight: cx.theme().list.active_highlight,
        window_width,
        window_height,
        pinned_folders: prior.pinned_folders,
        show_info_pane: prior.show_info_pane,
        file_view_mode: prior.file_view_mode,
        file_sort_option: prior.file_sort_option,
        file_sort_direction: prior.file_sort_direction,
        file_show_hidden: prior.file_show_hidden,
        show_file_extensions: prior.show_file_extensions,
        path_history: prior.path_history,
        sidebar_display_mode: prior.sidebar_display_mode,
        sidebar_collapsed: prior.sidebar_collapsed,
        show_sidebar_section_pinned: prior.show_sidebar_section_pinned,
        show_sidebar_section_library: prior.show_sidebar_section_library,
        show_sidebar_section_drives: prior.show_sidebar_section_drives,
        show_sidebar_section_cloud: prior.show_sidebar_section_cloud,
        show_sidebar_section_network: prior.show_sidebar_section_network,
        show_sidebar_section_wsl: prior.show_sidebar_section_wsl,
        show_sidebar_section_file_tags: prior.show_sidebar_section_file_tags,
        file_tags: prior.file_tags,
        show_home_quick_access: prior.show_home_quick_access,
        show_home_drives: prior.show_home_drives,
        show_home_network: prior.show_home_network,
        show_home_file_tags: prior.show_home_file_tags,
        show_home_recent: prior.show_home_recent,
        home_quick_access_expanded: prior.home_quick_access_expanded,
        home_drives_expanded: prior.home_drives_expanded,
        home_network_expanded: prior.home_network_expanded,
        home_file_tags_expanded: prior.home_file_tags_expanded,
        home_recent_expanded: prior.home_recent_expanded,
        home_widget_order: prior.home_widget_order,
        context_menu_shell_extensions_submenu: prior.context_menu_shell_extensions_submenu,
        context_menu_show_compress: prior.context_menu_show_compress,
        context_menu_show_send_to: prior.context_menu_show_send_to,
        context_menu_show_pin: prior.context_menu_show_pin,
        context_menu_show_open_in_terminal: prior.context_menu_show_open_in_terminal,
        context_menu_show_file_tags: prior.context_menu_show_file_tags,
        context_menu_show_create_shortcut: prior.context_menu_show_create_shortcut,
        open_text_with_cybereditor: prior.open_text_with_cybereditor,
        session_tabs: prior.session_tabs,
        session_active_tab: prior.session_active_tab,
        session_pane_layouts: prior.session_pane_layouts,
        session_closed_tabs: prior.session_closed_tabs,
    }
}
