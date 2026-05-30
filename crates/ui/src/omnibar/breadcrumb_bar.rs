use std::path::{Path, PathBuf};
use std::cell::RefCell;
use std::rc::Rc;

use cyber_desktop_fs::{
    breadcrumb_dropdown_entries, breadcrumb_visible_layout_for_widths, BreadcrumbDropdownResult,
    BreadcrumbMenuSection, DirectoryReadOptions, OmnibarPathSuggestion, PathBreadcrumb,
    BREADCRUMB_BLOCK_GAP,
};
use gpui::{prelude::*, *};
use gpui_component::plot::label::measure_text_width;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, ActiveTheme as _, ElementExt as _, Sizable as _, Size,
};
use rust_i18n::t;

use super::breadcrumb_flyout::BreadcrumbFlyout;
use crate::app_state::AppNavigation;
use crate::file_browser::DraggedFilePaths;
use crate::icons::home_icon_element;
use crate::popup_menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use crate::toolbar_button::{toolbar_icon_button, TOOLBAR_BUTTON_PX};

/// Segment label at 14px.
const BREADCRUMB_SEGMENT_FONT_SIZE: Pixels = px(14.);
/// Horizontal inset on medium ghost labeled buttons (label + padding).
const BREADCRUMB_LABELED_BUTTON_PADDING: f32 = 12.;
const BREADCRUMB_CHEVRON_BUTTON_PX: Pixels = px(24.);

/// Breadcrumb dropdown outer width (Files-style flyout).
const BREADCRUMB_DROPDOWN_MIN_WIDTH: Pixels = px(220.);
const BREADCRUMB_DROPDOWN_MAX_WIDTH: Pixels = px(350.);
/// Content row inside the menu: 350 − scrollbar(16) − menu padding(8) − item padding(16).
const BREADCRUMB_DROPDOWN_ROW_WIDTH: Pixels = px(310.);
/// Menu item `text_sm()` (0.875rem @ 16px base).
const BREADCRUMB_MENU_FONT_SIZE: Pixels = px(14.);
const BREADCRUMB_MENU_ELLIPSIS: &str = "…";
/// App icon size (18px) + `gap_2()`.
const BREADCRUMB_MENU_ICON_WIDTH: f32 = 18.;
const BREADCRUMB_MENU_ICON_GAP: f32 = 8.;

/// Files-style path breadcrumb: home root + unified segment blocks (label + chevron), optional ellipsis.
#[derive(IntoElement)]
pub struct PathBreadcrumbBar {
    show_root: bool,
    segments: Vec<PathBreadcrumb>,
    available_width: f32,
    read_options: DirectoryReadOptions,
    working_directory: Option<PathBuf>,
    root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
    on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_home: Rc<dyn Fn(&mut Window, &mut App)>,
    on_drop_paths: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
    on_drag_hover: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
    /// Click on non-item chrome (empty bar area) shows the full path string.
    on_show_full_path: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl PathBreadcrumbBar {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        show_root: bool,
        segments: Vec<PathBreadcrumb>,
        available_width: f32,
        read_options: DirectoryReadOptions,
        working_directory: Option<PathBuf>,
        root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
        on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
        on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
        on_home: Rc<dyn Fn(&mut Window, &mut App)>,
        on_drop_paths: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
        on_drag_hover: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
        on_show_full_path: Rc<dyn Fn(&mut Window, &mut App)>,
    ) -> Self {
        Self {
            show_root,
            segments,
            available_width,
            read_options,
            working_directory,
            root_menu,
            on_navigate,
            on_navigate_new_tab,
            on_home,
            on_drop_paths,
            on_drag_hover,
            on_show_full_path,
        }
    }
}

fn breadcrumb_labeled_block_width(label: &str, window: &mut Window) -> f32 {
    let text = measure_text_width(
        &SharedString::from(label),
        BREADCRUMB_SEGMENT_FONT_SIZE,
        window,
    );
    text + BREADCRUMB_LABELED_BUTTON_PADDING
}

fn breadcrumb_segment_block_width(label: &str, has_chevron: bool, window: &mut Window) -> f32 {
    let mut w = breadcrumb_labeled_block_width(label, window);
    if has_chevron {
        w += f32::from(BREADCRUMB_CHEVRON_BUTTON_PX);
    }
    w
}

impl RenderOnce for PathBreadcrumbBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let n = self.segments.len();
        let segment_widths: Vec<f32> = self
            .segments
            .iter()
            .enumerate()
            .map(|(i, s)| breadcrumb_segment_block_width(&s.label, i + 1 < n, window))
            .collect();
        let root_width = if self.show_root {
            f32::from(TOOLBAR_BUTTON_PX)
                + f32::from(BREADCRUMB_CHEVRON_BUTTON_PX)
                + f32::from(px(2.))
        } else {
            0.0
        };
        let ellipsis_width = breadcrumb_labeled_block_width("…", window);
        let layout = breadcrumb_visible_layout_for_widths(
            &segment_widths,
            self.available_width,
            self.show_root,
            root_width,
            ellipsis_width,
            BREADCRUMB_BLOCK_GAP,
        );
        let on_navigate = self.on_navigate.clone();
        let on_navigate_new_tab = self.on_navigate_new_tab.clone();
        let on_home = self.on_home.clone();
        let on_drop_paths = self.on_drop_paths.clone();
        let on_drag_hover = self.on_drag_hover.clone();
        let on_show_full_path = self.on_show_full_path.clone();
        let root_menu = self.root_menu.clone();
        let read_options = self.read_options;
        let working_directory = self.working_directory.clone();

        let mut bar = h_flex()
            .id("path-breadcrumb-bar")
            .flex_1()
            .w_full()
            .min_w_0()
            .overflow_x_hidden()
            .gap(px(2.))
            .items_center()
            .cursor_pointer()
            .on_click(move |_, window, cx| on_show_full_path(window, cx));

        if self.show_root {
            bar = bar.child(render_root_item(on_home, root_menu, cx));
        }

        if layout.hidden_prefix_len > 0 {
            let collapsed: Vec<PathBreadcrumb> = self.segments[..layout.hidden_prefix_len].to_vec();
            bar = bar.child(render_ellipsis_item(collapsed, on_navigate.clone(), cx));
        }

        for &index in &layout.visible_indices {
            let Some(crumb) = self.segments.get(index) else {
                continue;
            };
            let is_last = index + 1 == self.segments.len();
            let show_chevron = !is_last;
            bar = bar.child(render_path_segment(
                index,
                crumb.clone(),
                show_chevron,
                is_last,
                read_options,
                working_directory.as_deref(),
                on_navigate.clone(),
                on_navigate_new_tab.clone(),
                on_drop_paths.clone(),
                on_drag_hover.clone(),
                cx,
            ));
        }

        // Fill remaining bar width so click-to-edit and layout use the full omnibar region.
        bar.child(div().flex_1().min_w_0())
    }
}

fn render_chevron_menu(
    button_id: impl Into<ElementId>,
    tooltip: String,
    menu_builder: impl Fn(
            Option<&BreadcrumbDropdownResult>,
            PopupMenu,
            &mut Window,
            &mut Context<PopupMenu>,
        ) -> PopupMenu
        + 'static,
) -> BreadcrumbFlyout {
    let button_id = button_id.into();
    BreadcrumbFlyout::new(
        SharedString::from(format!("breadcrumb-flyout-{button_id:?}")),
        button_id.clone(),
        tooltip,
        menu_builder,
    )
}

fn render_segment_chevron_menu(
    button_id: impl Into<ElementId>,
    path: PathBuf,
    read_options: DirectoryReadOptions,
    working_directory: Option<PathBuf>,
) -> BreadcrumbFlyout {
    segment_dropdown_menu_builder(button_id.into(), path, read_options, working_directory)
}

fn render_root_item(
    on_home: Rc<dyn Fn(&mut Window, &mut App)>,
    root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
    cx: &App,
) -> impl IntoElement {
    let root_menu_builder = root_dropdown_menu_builder(root_menu);
    let home_tip = t!("omnibar.breadcrumb.root_tooltip").to_string();
    let chevron_tip = t!("omnibar.breadcrumb.chevron_tooltip").to_string();
    h_flex()
        .id("breadcrumb-root")
        .items_center()
        .rounded_l(cx.theme().radius)
        .hover(|s| s.bg(cx.theme().secondary))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            toolbar_icon_button("breadcrumb-root-home")
                .child(home_icon_element())
                .tooltip(home_tip)
                .on_click(move |_, window, cx| on_home(window, cx)),
        )
        .child(render_chevron_menu(
            "breadcrumb-root-chevron",
            chevron_tip,
            root_menu_builder,
        ))
}

fn render_ellipsis_item(
    collapsed: Vec<PathBreadcrumb>,
    on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement {
    let tip = t!("omnibar.breadcrumb.ellipsis_tooltip").to_string();
    h_flex()
        .id("breadcrumb-ellipsis")
        .items_center()
        .rounded(cx.theme().radius)
        .hover(|s| s.bg(cx.theme().secondary))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
        .child(
            Button::new("breadcrumb-ellipsis-button")
                .with_size(Size::Small)
                .ghost()
                .label("…")
                .tooltip(tip)
                .dropdown_menu_with_anchor(Anchor::BottomLeft, {
                    let build = ellipsis_dropdown_menu_builder(collapsed, on_navigate);
                    move |menu, window, cx| build(None, menu, window, cx)
                }),
        )
}

fn render_path_segment(
    index: usize,
    crumb: PathBreadcrumb,
    show_chevron: bool,
    is_last: bool,
    read_options: DirectoryReadOptions,
    working_directory: Option<&Path>,
    on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_drop_paths: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
    on_drag_hover: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement {
    let path_nav = crumb.path.clone();
    let path_menu = crumb.path.clone();
    let label = crumb.label.clone();
    let tooltip = crumb.path.display().to_string();
    let working = working_directory.map(Path::to_path_buf);
    let navigate = on_navigate.clone();
    let new_tab = on_navigate_new_tab.clone();
    let drop_target = path_nav.clone();
    let hover_target = path_nav.clone();
    let drop = on_drop_paths.clone();
    let hover = on_drag_hover.clone();
    let is_dir = path_nav.is_dir();
    let hover_bounds = Rc::new(RefCell::new(None::<Bounds<Pixels>>));

    let mut segment = h_flex()
        .id(("breadcrumb-segment", index))
        .items_center()
        .rounded(cx.theme().radius)
        .hover(|s| s.bg(cx.theme().secondary))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Middle, {
            let path = path_nav.clone();
            let new_tab = new_tab.clone();
            move |_, window, cx| {
                cx.stop_propagation();
                if !is_last {
                    new_tab(path.clone(), window, cx);
                }
            }
        })
        .child({
            let navigate = navigate.clone();
            Button::new(("breadcrumb-segment-label", index))
                .with_size(Size::Small)
                .ghost()
                .label(label)
                .tooltip(tooltip)
                .on_click(move |_, window, cx| {
                    if is_last {
                        return;
                    }
                    navigate(path_nav.clone(), window, cx);
                })
        });

    if show_chevron {
        segment = segment.child(render_segment_chevron_menu(
            ("breadcrumb-segment-chevron", index),
            path_menu,
            read_options,
            working,
        ));
    }

    if is_dir {
        segment = segment.on_drag_move::<DraggedFilePaths>({
            let hover = hover.clone();
            let hover_target = hover_target.clone();
            let hover_bounds = hover_bounds.clone();
            move |event: &DragMoveEvent<DraggedFilePaths>, window, cx| {
                let Some(bounds) = *hover_bounds.borrow() else {
                    return;
                };
                if crate::file_browser::sweep::point_in_bounds(event.event.position, bounds) {
                    hover(hover_target.clone(), event.drag(cx).0.clone(), window, cx);
                }
            }
        });
    }

    if is_dir && !is_last {
        segment = segment.on_drop({
            let drop = drop.clone();
            let drop_target = drop_target.clone();
            move |paths: &DraggedFilePaths, window, cx| {
                drop(drop_target.clone(), paths.0.clone(), window, cx);
            }
        });
    } else if is_dir {
        segment = segment.on_drop({
            let drop = drop.clone();
            let drop_target = drop_target.clone();
            move |paths: &DraggedFilePaths, window, cx| {
                drop(drop_target.clone(), paths.0.clone(), window, cx);
            }
        });
    }

    segment.on_prepaint({
        let hover_bounds = hover_bounds.clone();
        move |bounds, _window, _cx| {
            *hover_bounds.borrow_mut() = Some(bounds);
        }
    })
}

fn apply_breadcrumb_menu_style(menu: PopupMenu) -> PopupMenu {
    menu.min_w(BREADCRUMB_DROPDOWN_MIN_WIDTH)
        .max_w(BREADCRUMB_DROPDOWN_MAX_WIDTH)
}

fn breadcrumb_menu_label_max_width(has_icon: bool) -> f32 {
    let row = f32::from(BREADCRUMB_DROPDOWN_ROW_WIDTH);
    if has_icon {
        row - BREADCRUMB_MENU_ICON_WIDTH - BREADCRUMB_MENU_ICON_GAP
    } else {
        row
    }
}

/// Truncate by shaped text width so CJK and Latin share one consistent `…` position.
fn truncate_breadcrumb_menu_label(label: &str, max_width: f32, window: &mut Window) -> String {
    let full = SharedString::from(label);
    if measure_text_width(&full, BREADCRUMB_MENU_FONT_SIZE, window) <= max_width {
        return label.to_string();
    }
    let ellipsis = SharedString::from(BREADCRUMB_MENU_ELLIPSIS);
    let ellipsis_w = measure_text_width(&ellipsis, BREADCRUMB_MENU_FONT_SIZE, window);
    let budget = (max_width - ellipsis_w).max(0.);
    let mut acc = String::new();
    for ch in label.chars() {
        acc.push(ch);
        if measure_text_width(&SharedString::from(&acc), BREADCRUMB_MENU_FONT_SIZE, window) > budget
        {
            acc.pop();
            if acc.is_empty() {
                return BREADCRUMB_MENU_ELLIPSIS.to_string();
            }
            acc.push_str(BREADCRUMB_MENU_ELLIPSIS);
            return acc;
        }
    }
    label.to_string()
}

/// One menu row: fixed label budget + pixel-accurate ellipsis (not CSS `truncate`).
fn breadcrumb_menu_row(
    label: String,
    icon: Option<AnyElement>,
    dimmed: bool,
    window: &mut Window,
) -> impl IntoElement {
    let display = truncate_breadcrumb_menu_label(
        &label,
        breadcrumb_menu_label_max_width(icon.is_some()),
        window,
    );
    h_flex()
        .w_full()
        .max_w(BREADCRUMB_DROPDOWN_ROW_WIDTH)
        .min_w_0()
        .overflow_hidden()
        .items_center()
        .gap_2()
        .text_sm()
        .when(dimmed, |row| row.opacity(0.55))
        .when_some(icon, |row, icon| row.child(icon))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(display),
        )
}

fn popup_menu_path_item(
    entry: OmnibarPathSuggestion,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> PopupMenuItem {
    let path = entry.path.clone();
    let label = entry.label.clone();
    let dimmed = entry.dimmed;
    PopupMenuItem::element(move |window, _| {
        breadcrumb_menu_row(
            label.clone(),
            Some(crate::shell_icon::shell_icon_for_path(
                &path,
                px(16.),
                window,
            )),
            dimmed,
            window,
        )
    })
    .on_click(on_click)
}

fn popup_menu_text_item(
    label: String,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> PopupMenuItem {
    PopupMenuItem::element(move |window, _| breadcrumb_menu_row(label.clone(), None, false, window))
        .on_click(on_click)
}

/// Builds segment chevron menu from optional async `read_dir` result (CyberFiles; not upstream API).
fn segment_dropdown_menu(menu: PopupMenu, result: Option<&BreadcrumbDropdownResult>) -> PopupMenu {
    let mut menu = apply_breadcrumb_menu_style(menu.scrollable(true));
    match result {
        None => menu
            .item(PopupMenuItem::new(t!("omnibar.breadcrumb.loading").to_string()).disabled(true)),
        Some(BreadcrumbDropdownResult::AccessDenied) => menu.item(
            PopupMenuItem::new(t!("omnibar.breadcrumb.access_denied").to_string()).disabled(true),
        ),
        Some(BreadcrumbDropdownResult::Empty) => {
            menu.item(PopupMenuItem::new(t!("omnibar.breadcrumb.empty").to_string()).disabled(true))
        }
        Some(BreadcrumbDropdownResult::Entries(entries)) => {
            for entry in entries {
                let target = entry.path.clone();
                menu = menu.item(popup_menu_path_item(entry.clone(), move |_, _, cx| {
                    AppNavigation::navigate_to_path(target.clone(), cx);
                }));
            }
            menu
        }
    }
}

fn segment_dropdown_menu_builder(
    button_id: ElementId,
    path: PathBuf,
    read_options: DirectoryReadOptions,
    working_directory: Option<PathBuf>,
) -> BreadcrumbFlyout {
    let fill_path = path.clone();
    let fill_options = read_options;
    let fill_working = working_directory.clone();
    BreadcrumbFlyout::new_async(
        SharedString::from(format!("breadcrumb-flyout-{button_id:?}")),
        button_id,
        t!("omnibar.breadcrumb.chevron_tooltip").to_string(),
        |result, menu, _, _| segment_dropdown_menu(menu, result),
        move || breadcrumb_dropdown_entries(&fill_path, fill_options, fill_working.as_deref()),
    )
}

fn root_dropdown_menu_builder(
    root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
) -> impl Fn(
    Option<&BreadcrumbDropdownResult>,
    PopupMenu,
    &mut Window,
    &mut Context<PopupMenu>,
) -> PopupMenu
       + 'static {
    move |_, menu, _, _| {
        let sections = root_menu();
        let mut menu = menu.scrollable(true);
        if sections.is_empty() {
            return apply_breadcrumb_menu_style(menu.item(
                PopupMenuItem::new(t!("omnibar.breadcrumb.empty").to_string()).disabled(true),
            ));
        }
        for (section_index, section) in sections.iter().enumerate() {
            if section_index > 0 {
                menu = menu.item(PopupMenuItem::separator());
            }
            if let Some(ref heading) = section.heading {
                menu = menu.item(PopupMenuItem::label(heading.clone()));
            }
            for entry in &section.entries {
                let target = entry.path.clone();
                menu = menu.item(popup_menu_path_item(entry.clone(), move |_, _, cx| {
                    AppNavigation::navigate_to_path(target.clone(), cx);
                }));
            }
        }
        apply_breadcrumb_menu_style(menu)
    }
}

fn ellipsis_dropdown_menu_builder(
    collapsed: Vec<PathBreadcrumb>,
    on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
) -> impl Fn(
    Option<&BreadcrumbDropdownResult>,
    PopupMenu,
    &mut Window,
    &mut Context<PopupMenu>,
) -> PopupMenu
       + 'static {
    move |_, menu, _, _| {
        let mut menu = menu.scrollable(true);
        for crumb in &collapsed {
            let path = crumb.path.clone();
            let label = crumb.label.clone();
            let navigate = on_navigate.clone();
            menu = menu.item(popup_menu_text_item(label, move |_, window, cx| {
                navigate(path.clone(), window, cx);
            }));
        }
        apply_breadcrumb_menu_style(menu)
    }
}
