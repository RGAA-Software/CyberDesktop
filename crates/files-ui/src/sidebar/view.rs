use std::path::Path;

use files_core::{load_config, sidebar_is_compact, sidebar_is_offcanvas};
use app_platform_windows::open_item_properties;
use gpui::{prelude::*, ClickEvent, *};
use gpui_component::{
    sidebar::{Sidebar, SidebarCollapsible, SidebarItem},
    v_flex,
    ActiveTheme as _,
    StyledExt as _,
};
use rust_i18n::t;

use crate::app_state::AppNavigation;
use crate::drag::DraggedFilePaths;
use crate::icons::{chrome_icon_color, drive_tabler_icon, sidebar_tabler_icon};
use files_fs::parse_tag_color_hex;
use crate::main_page::MainPage;
use app_ui::popup_menu::{PopupMenu, PopupMenuItem};
use crate::shell::navigation::NavigationTarget;

use super::disk_ring::disk_usage_ring;
use super::menu_with_drop::SidebarMenuWithDrop;
use super::model::{SidebarEntry, SidebarSection, SidebarSectionKind};
use crate::tabler_icons;

pub fn render_sidebar(
    page: Entity<MainPage>,
    active: NavigationTarget,
    sections: &[SidebarSection],
    _window: &mut Window,
    cx: &mut Context<MainPage>,
) -> impl IntoElement {
    let config = load_config().unwrap_or_default();
    let collapsed = config.sidebar_collapsed;
    let collapsible = if sidebar_is_offcanvas(&config) {
        SidebarCollapsible::Offcanvas
    } else if sidebar_is_compact(&config) {
        SidebarCollapsible::Icon
    } else {
        SidebarCollapsible::None
    };

    let mut sidebar = Sidebar::new("files-sidebar")
        .collapsible(collapsible)
        .collapsed(collapsed)
        .w_full()
        .min_w_0()
        .border_0();

    for (index, section) in sections.iter().enumerate() {
        let mut menu = SidebarMenuWithDrop::new();
        for entry in &section.entries {
            append_sidebar_entry(&mut menu, &page, entry, section.kind, &active);
        }
        let block = SidebarSectionBlock::section(section.title.clone(), menu, index == 0);
        sidebar = sidebar.child(block);
    }

    div()
        .id("files-sidebar-wrap")
        .size_full()
        .min_h_0()
        .bg(cx.theme().secondary)
        .border_r_1()
        .border_color(cx.theme().border)
        .px(px(8.))
        .pt(px(4.))
        .pb(px(9.))
        .overflow_y_scroll()
        .child(sidebar)
}

/// Sidebar section heading + entries.
#[derive(Clone)]
enum SidebarSectionBlock {
    Section {
        title: String,
        menu: SidebarMenuWithDrop,
        first: bool,
    },
}

impl SidebarSectionBlock {
    fn section(title: String, menu: SidebarMenuWithDrop, first: bool) -> Self {
        Self::Section { title, menu, first }
    }
}

fn section_heading(
    title: impl Into<SharedString>,
    first: bool,
    cx: &App,
) -> impl IntoElement {
    let title: SharedString = title.into();
    div()
        .w_full()
        .px(px(9.))
        .pt(px(if first { 5. } else { 10. }))
        .pb(px(4.))
        .text_size(px(10.))
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .child(title)
}

impl gpui_component::Collapsible for SidebarSectionBlock {
    fn is_collapsed(&self) -> bool {
        match self {
            Self::Section { menu, .. } => menu.is_collapsed(),
        }
    }

    fn collapsed(self, collapsed: bool) -> Self {
        match self {
            Self::Section { title, menu, first } => Self::Section {
                title,
                menu: menu.collapsed(collapsed),
                first,
            },
        }
    }
}

impl SidebarItem for SidebarSectionBlock {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        match self {
            Self::Section { title, menu, first } => v_flex()
                .w_full()
                .gap(px(2.))
                .child(section_heading(title, first, cx))
                .child(menu.render(id, window, cx))
                .into_any_element(),
        }
    }
}

fn usage_ring_suffix(fraction: f32) -> std::rc::Rc<dyn Fn(&mut Window, &mut App) -> gpui::AnyElement> {
    std::rc::Rc::new(move |_window, cx| disk_usage_ring(fraction, cx))
}

fn append_sidebar_entry(
    menu: &mut SidebarMenuWithDrop,
    page: &Entity<MainPage>,
    entry: &SidebarEntry,
    section: SidebarSectionKind,
    active: &NavigationTarget,
) {
    let is_active = navigation_matches(active, &entry.target);
    let page_click = page.clone();
    let page_middle = page.clone();
    let page_menu = page.clone();
    let entry = entry.clone();
    let target = entry.target.clone();
    let label = SharedString::from(entry.label.clone());

    let target_click = target.clone();
    let handler = move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        let _ = page_click.update(cx, |page, cx| {
            page.navigate_to(target_click.clone(), cx);
        });
    };

    let middle_click: Option<std::rc::Rc<dyn Fn(&mut Window, &mut App)>> =
        if matches!(&target, NavigationTarget::Path(_)) {
            let target = target.clone();
            Some(std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
                if let NavigationTarget::Path(path) = &target {
                    let _ = page_middle.update(cx, |page, cx| {
                        page.open_path_in_new_tab(path.clone(), cx);
                    });
                }
            }))
        } else {
            None
        };

    let entry_menu = entry.clone();
    let context_menu: Option<std::rc::Rc<dyn Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu>> =
        Some(std::rc::Rc::new(move |menu, window, cx| {
            build_entry_context_menu(menu, &page_menu, &entry_menu, window, cx)
        }));

    let icon = sidebar_entry_icon(&entry, section, is_active);
    let suffix = entry.usage_fraction.map(usage_ring_suffix);

    if let Some(dest) = drop_destination(&entry.target) {
        let page_drop = page.clone();
        let page_drop_external = page.clone();
        let dest_drop = dest.clone();
        let dest_drop_external = dest.clone();
        menu.push_item_with_folder_drop(
            label,
            icon,
            is_active,
            handler,
            middle_click,
            context_menu,
            move |_, _| {},
            move |paths: &DraggedFilePaths, window, cx| {
                let path = dest_drop.clone();
                let _ = page_drop.update(cx, |page, cx| {
                    page.drop_paths_on_directory(path, paths.0.clone(), window, cx);
                });
            },
            move |paths: &ExternalPaths, window, cx| {
                let path = dest_drop_external.clone();
                let _ = page_drop_external.update(cx, |page, cx| {
                    page.drop_external_paths_on_directory(
                        path,
                        paths.paths().to_vec(),
                        window,
                        cx,
                    );
                });
            },
            suffix.clone(),
        );
    } else if let Some(suffix) = suffix {
        menu.push_item_with_suffix(
            label,
            icon,
            is_active,
            handler,
            middle_click,
            context_menu,
            Some(suffix),
        );
    } else {
        menu.push_item(label, icon, is_active, handler, middle_click, context_menu);
    }
}

fn tabler_path_for_sidebar_entry(
    entry: &SidebarEntry,
    section: SidebarSectionKind,
) -> &'static str {
    match (&entry.target, section) {
        (NavigationTarget::Home, _) => tabler_icons::HOME,
        (NavigationTarget::RecycleBin, _) => tabler_icons::TRASH,
        (NavigationTarget::FileTag(_), _) => tabler_icons::TAG,
        (NavigationTarget::Settings, _) => tabler_icons::SETTINGS,
        (NavigationTarget::SearchResults { .. }, _) => tabler_icons::SEARCH,
        (NavigationTarget::Path(path), SidebarSectionKind::Drives) => drive_tabler_icon(path),
        (_, SidebarSectionKind::Cloud) => tabler_icons::CLOUD,
        (_, SidebarSectionKind::Network) => tabler_icons::NETWORK,
        (_, SidebarSectionKind::Wsl) => tabler_icons::BRAND_WINDOWS,
        (_, SidebarSectionKind::Library) => tabler_icons::BOOK,
        (NavigationTarget::Path(_), _) => tabler_icons::FOLDER,
    }
}

fn sidebar_entry_icon(
    entry: &SidebarEntry,
    section: SidebarSectionKind,
    active: bool,
) -> impl Fn(&mut Window, &mut App) -> AnyElement + 'static {
    let path = tabler_path_for_sidebar_entry(entry, section);
    let tag_color = entry.color.clone();
    let is_file_tag = matches!(entry.target, NavigationTarget::FileTag(_));
    move |_window, cx| {
        if is_file_tag {
            let icon_color: Hsla = gpui::rgb(
                tag_color
                    .as_deref()
                    .and_then(parse_tag_color_hex)
                    .unwrap_or(0x54_6E_7A),
            )
            .into();
            sidebar_tabler_icon(tabler_icons::TAG, icon_color)
        } else {
            let color = if active {
                cx.theme().primary
            } else {
                chrome_icon_color(cx)
            };
            sidebar_tabler_icon(path, color)
        }
    }
}

fn drop_destination(target: &NavigationTarget) -> Option<std::path::PathBuf> {
    match target {
        NavigationTarget::Path(path) if path.is_dir() => Some(path.clone()),
        _ => None,
    }
}

fn build_entry_context_menu(
    menu: PopupMenu,
    page: &Entity<MainPage>,
    entry: &SidebarEntry,
    _window: &mut Window,
    cx: &mut App,
) -> PopupMenu {
    let target = entry.target.clone();
    let pinned = entry.pinned_in_settings;

    let page_nav = page.clone();
    let nav_target = target.clone();
    let mut menu = menu.item(PopupMenuItem::new(t!("sidebar.menu.open")).on_click(
        move |_, _, cx| {
            let _ = page_nav.update(cx, |p, cx| p.navigate_to(nav_target.clone(), cx));
        },
    ));

    if let NavigationTarget::Path(path) = target.clone() {
        let path_exists = path.exists();
        let path_string = path.to_string_lossy().to_string();

        let page_tab = page.clone();
        let path_tab = path.clone();
        menu = menu.item(
            PopupMenuItem::new(t!("sidebar.menu.open_new_tab")).on_click(move |_, _, cx| {
                let _ = page_tab.update(cx, |p, cx| p.open_path_in_new_tab(path_tab.clone(), cx));
            }),
        );

        if crate::shell::preferences::show_open_in_new_pane(cx) {
            let path_pane = path.clone();
            menu = menu.item(
                PopupMenuItem::new(t!("files.menu.open_in_new_pane")).on_click(move |_, _, cx| {
                    AppNavigation::open_path_in_secondary_pane(path_pane.clone(), cx);
                    cx.stop_propagation();
                }),
            );
        }

        if pinned {
            let page_unpin = page.clone();
            let ps_unpin = path_string.clone();
            menu = menu.item(PopupMenuItem::new(t!("sidebar.menu.unpin")).on_click(
                move |_, _, cx| {
                    let _ = page_unpin.update(cx, |p, cx| {
                        p.unpin_folder_path(&ps_unpin, cx);
                    });
                },
            ));
            let page_up = page.clone();
            let ps_up = path_string.clone();
            menu = menu.item(PopupMenuItem::new(t!("sidebar.menu.move_up")).on_click(
                move |_, _, cx| {
                    let _ = page_up.update(cx, |p, cx| p.move_pinned_folder(&ps_up, -1, cx));
                },
            ));
            let page_down = page.clone();
            let ps_down = path_string.clone();
            menu = menu.item(PopupMenuItem::new(t!("sidebar.menu.move_down")).on_click(
                move |_, _, cx| {
                    let _ = page_down.update(cx, |p, cx| p.move_pinned_folder(&ps_down, 1, cx));
                },
            ));
        } else if path_exists {
            let page_pin = page.clone();
            let path_pin = path.clone();
            menu = menu.item(PopupMenuItem::new(t!("sidebar.menu.pin")).on_click(
                move |_, _, cx| {
                    let _ = page_pin.update(cx, |p, cx| p.pin_folder_path(path_pin.clone(), cx));
                },
            ));
        }

        let path_props = path.clone();
        menu = menu.item(PopupMenuItem::new(t!("sidebar.menu.properties")).on_click(
            move |_, _, cx| {
                let _ = open_item_properties(&path_props);
                cx.stop_propagation();
            },
        ));
    }

    menu
}

pub fn navigation_matches(active: &NavigationTarget, entry: &NavigationTarget) -> bool {
    match (active, entry) {
        (NavigationTarget::Home, NavigationTarget::Home) => true,
        (NavigationTarget::RecycleBin, NavigationTarget::RecycleBin) => true,
        (NavigationTarget::FileTag(active), NavigationTarget::FileTag(entry)) => active == entry,
        (NavigationTarget::Path(current), NavigationTarget::Path(sidebar)) => {
            paths_match(sidebar, current)
        }
        _ => false,
    }
}

fn paths_match(sidebar: &Path, current: &Path) -> bool {
    if paths_equal(sidebar, current) {
        return true;
    }
    if let (Ok(a), Ok(b)) = (
        std::fs::canonicalize(sidebar),
        std::fs::canonicalize(current),
    ) {
        return is_strict_descendant(&a, &b);
    }
    is_strict_descendant(sidebar, current)
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        return a == b;
    }
    false
}

/// True when `path` is a strict child of `ancestor` (not equal).
fn is_strict_descendant(ancestor: &Path, path: &Path) -> bool {
    let ancestor_components: Vec<_> = ancestor.components().collect();
    let mut path_components: Vec<_> = path.components().collect();
    if path_components.len() <= ancestor_components.len() {
        return false;
    }
    path_components.truncate(ancestor_components.len());
    path_components == ancestor_components
}
