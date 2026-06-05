//! Shared Home widget chrome (card grids, icons, drive space bar).

use gpui::{
    div, prelude::*, px, AnyElement, App, Div, ElementId, InteractiveElement, MouseButton, Pixels,
    Stateful, Window,
};
use gpui_component::{h_flex, progress::Progress, v_flex, ActiveTheme as _};

pub const HOME_PAGE_PADDING_X: Pixels = px(26.);
pub const HOME_PAGE_PADDING_Y: Pixels = px(22.);
pub const HOME_SECTION_GAP: Pixels = px(26.);

/// Design: `repeat(auto-fill, minmax(200px, 1fr))`.
pub const CARD_MIN_WIDTH: Pixels = px(200.);
pub const GRID_GAP: Pixels = px(10.);
pub const TAG_COLUMNS: usize = 4;

pub const QA_ITEM_HEIGHT: Pixels = px(68.);
pub const QA_ITEM_PADDING_X: Pixels = px(14.);
pub const QA_ITEM_PADDING_Y: Pixels = px(12.);
pub const QA_ICON_TILE: Pixels = px(38.);
pub const QA_ICON_INNER: Pixels = px(20.);

pub const DRIVE_CARD_PADDING_X: Pixels = px(16.);
pub const DRIVE_CARD_PADDING_Y: Pixels = px(14.);
pub const DRIVE_ICON_TILE: Pixels = px(34.);

pub const RECENT_ROW_HEIGHT: Pixels = px(34.);
pub const RECENT_HEADER_HEIGHT: Pixels = px(30.);

/// Design `--radius-lg`.
pub const HOME_CARD_RADIUS: Pixels = px(12.);

/// Fallback before the scroll area is measured.
pub fn estimated_content_width(window: &Window) -> Pixels {
    let sidebar = px(214.);
    let padding = HOME_PAGE_PADDING_X * 2.;
    (window.viewport_size().width - sidebar - padding).max(px(400.))
}

pub fn auto_fill_column_count(container_width: Pixels, min_cell: Pixels, gap: Pixels) -> usize {
    let w = container_width.as_f32();
    let min = min_cell.as_f32();
    let g = gap.as_f32();
    ((w + g) / (min + g)).floor().max(1.) as usize
}

pub fn column_cell_width(container_width: Pixels, columns: usize, gap: Pixels) -> Pixels {
    let cols = columns.max(1) as f32;
    let w = container_width.as_f32();
    let g = gap.as_f32();
    px((w - g * (cols - 1.)) / cols)
}

/// Stop the Home page “show/hide widgets” menu from opening (bubble phase).
pub fn block_home_page_context_menu<T>(element: T) -> T
where
    T: InteractiveElement,
{
    element.on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
}

/// Quick access / drives: `auto-fill` with `minmax(200px, 1fr)`.
pub fn home_card_grid(
    container_width: Pixels,
    children: impl IntoIterator<Item = AnyElement>,
) -> impl IntoElement {
    let children: Vec<_> = children.into_iter().collect();
    let columns = auto_fill_column_count(container_width, CARD_MIN_WIDTH, GRID_GAP);
    let cell_width = column_cell_width(container_width, columns, GRID_GAP);
    fixed_width_rows("home-card-grid", columns, cell_width, children)
}

/// File tags: exactly 4 equal columns spanning the parent width.
pub fn tag_cols_grid(
    _container_width: Pixels,
    children: impl IntoIterator<Item = AnyElement>,
) -> impl IntoElement {
    let children: Vec<_> = children.into_iter().collect();
    equal_width_rows("home-tag-cols", TAG_COLUMNS, children)
}

fn fixed_width_rows(
    id: &'static str,
    columns: usize,
    cell_width: Pixels,
    children: Vec<AnyElement>,
) -> impl IntoElement {
    let columns = columns.max(1);
    let mut rows = Vec::new();
    let mut iter = children.into_iter();
    loop {
        let row: Vec<_> = iter.by_ref().take(columns).collect();
        if row.is_empty() {
            break;
        }
        rows.push(row);
    }
    v_flex()
        .id(id)
        .w_full()
        .gap(GRID_GAP)
        .children(rows.into_iter().map(|row| {
            h_flex()
                .w_full()
                .items_start()
                .gap(GRID_GAP)
                .children(row.into_iter().map(|child| {
                    div()
                        .flex_none()
                        .w(cell_width)
                        .child(child)
                        .into_any_element()
                }))
                .into_any_element()
        }))
}

fn equal_width_rows(
    id: &'static str,
    columns: usize,
    children: Vec<AnyElement>,
) -> impl IntoElement {
    let columns = columns.max(1);
    let mut rows = Vec::new();
    let mut iter = children.into_iter();
    loop {
        let row: Vec<_> = iter.by_ref().take(columns).collect();
        if row.is_empty() {
            break;
        }
        rows.push(row);
    }
    v_flex()
        .id(id)
        .w_full()
        .gap(GRID_GAP)
        .children(rows.into_iter().map(|row| {
            let filled = row.len();
            h_flex()
                .w_full()
                .items_start()
                .gap(GRID_GAP)
                .children(row.into_iter().map(|child| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .self_start()
                        .child(child)
                        .into_any_element()
                }))
                .children((filled..columns).map(|col| {
                    div()
                        .id(format!("{id}-pad-{col}"))
                        .flex_1()
                        .min_w_0()
                        .self_start()
                        .into_any_element()
                }))
                .into_any_element()
        }))
}

pub fn bordered_home_card(id: impl Into<ElementId>, cx: &App) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .rounded(HOME_CARD_RADIUS)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
}

pub fn net_notice(
    id: impl Into<ElementId>,
    icon: impl IntoElement,
    text: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    h_flex()
        .id(id)
        .w_full()
        .gap(px(8.))
        .px(px(16.))
        .py(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(cx.theme().info.opacity(0.16))
        .bg(cx.theme().info.opacity(0.08))
        .text_sm()
        .text_color(cx.theme().info)
        .items_center()
        .child(icon)
        .child(text)
}

pub fn space_progress_bar(id: impl Into<ElementId>, fraction: f32) -> impl IntoElement {
    Progress::new(id)
        .w_full()
        .h(px(4.))
        .rounded(px(2.))
        .value(fraction.clamp(0., 1.) * 100.)
}
