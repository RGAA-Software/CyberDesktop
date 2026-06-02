use files_fs::{parse_tag_color_hex, TagRef};
use gpui::{div, px, prelude::*, AnyElement, App, Hsla, Styled};
use gpui_component::{h_flex, ActiveTheme as _};

const MAX_VISIBLE_TAGS: usize = 3;
const DEFAULT_TAG_COLOR: u32 = 0x54_6E_7A;

pub(super) fn tag_color(color: Option<&str>) -> Hsla {
    gpui::rgb(color.and_then(parse_tag_color_hex).unwrap_or(DEFAULT_TAG_COLOR)).into()
}

pub(super) fn render_tag_badges(tags: &[TagRef], cx: &App) -> AnyElement {
    if tags.is_empty() {
        return div().into_any_element();
    }

    let extra = tags.len().saturating_sub(MAX_VISIBLE_TAGS);
    let visible = if extra > 0 {
        &tags[..MAX_VISIBLE_TAGS]
    } else {
        tags
    };

    h_flex()
        .gap_1()
        .items_center()
        .flex_none()
        .children(visible.iter().map(|tag| {
            div()
                .size(px(8.))
                .rounded_full()
                .bg(tag_color(tag.color.as_deref()))
                .into_any_element()
        }))
        .when(extra > 0, |row| {
            row.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("+{extra}")),
            )
        })
        .into_any_element()
}

pub(super) fn render_name_with_tags(
    name: impl IntoElement,
    tags: &[TagRef],
    cx: &App,
) -> AnyElement {
    if tags.is_empty() {
        return div().w_full().child(name).into_any_element();
    }

    h_flex()
        .w_full()
        .gap_2()
        .items_center()
        .min_w_0()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .child(name),
        )
        .child(render_tag_badges(tags, cx))
        .into_any_element()
}
