//! Shared Home widget chrome (card grids, icons, drive space bar).

use gpui::{
    div, prelude::*, px, AnyElement, App, Div, ElementId, InteractiveElement, MouseButton, Pixels,
    Stateful, Window,
};
use gpui_component::{h_flex, progress::Progress, v_flex, ActiveTheme as _};

pub const HOME_PAGE_PADDING_X: Pixels = px(26.);
pub const HOME_PAGE_PADDING_Y: Pixels = px(22.);
pub const HOME_SECTION_GAP: Pixels = px(26.);

/// Fixed width for quick-access and drive cards on the Home page.
pub const CARD_CELL_WIDTH: Pixels = px(220.);
/// Legacy alias — prefer [`CARD_CELL_WIDTH`].
#[allow(dead_code)]
pub const CARD_MIN_WIDTH: Pixels = CARD_CELL_WIDTH;
pub const GRID_GAP: Pixels = px(10.);
/// Minimum clearance between the last item in a row and the right edge.
const GRID_RIGHT_MARGIN: Pixels = px(20.);
/// Reserve space when the home scroll area shows a vertical scrollbar.
const GRID_SCROLLBAR_RESERVE: Pixels = px(12.);
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

fn effective_grid_width(container_width: Pixels) -> Pixels {
    (container_width - GRID_SCROLLBAR_RESERVE).max(CARD_CELL_WIDTH)
}

/// How many fixed-width cards fit on one row with at least [`GRID_RIGHT_MARGIN`]
/// clearance to the right edge. If the next card would leave less than 20px
/// margin, it wraps to the next row instead.
fn items_per_row(container_width: Pixels) -> usize {
    let w = effective_grid_width(container_width).as_f32();
    let cell = CARD_CELL_WIDTH.as_f32();
    let gap = GRID_GAP.as_f32();
    let margin = GRID_RIGHT_MARGIN.as_f32();
    for count in (1..=32).rev() {
        let n = count as f32;
        let row_width = n * cell + (n - 1.) * gap;
        if row_width + margin <= w + 0.5 {
            return count;
        }
    }
    1
}

/// Stop the Home page “show/hide widgets” menu from opening (bubble phase).
pub fn block_home_page_context_menu<T>(element: T) -> T
where
    T: InteractiveElement,
{
    element.on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
}

/// Quick access / drives: fixed 220px cards; wrap when the right margin would
/// fall below 20px.
pub fn home_card_grid(
    container_width: Pixels,
    children: impl IntoIterator<Item = AnyElement>,
) -> impl IntoElement {
    let children: Vec<_> = children.into_iter().collect();
    let columns = items_per_row(container_width);
    fixed_width_rows("home-card-grid", columns, CARD_CELL_WIDTH, children)
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
                        .max_w(cell_width)
                        .min_w_0()
                        .overflow_hidden()
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

pub fn space_progress_bar(
    id: impl Into<ElementId>,
    fraction: f32,
    cx: &App,
) -> impl IntoElement {
    Progress::new(id)
        .w_full()
        .h(px(4.))
        .rounded(px(2.))
        .color(crate::sidebar::drive_usage_color(fraction, cx))
        .value(fraction.clamp(0., 1.) * 100.)
}
