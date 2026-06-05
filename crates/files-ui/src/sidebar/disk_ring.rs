use gpui::{
    div, img, prelude::*, px, AnyElement, App, Hsla, ImageCacheError, IntoElement, ObjectFit,
    ParentElement, Styled, Window,
};
use gpui_component::ActiveTheme as _;

fn ring_svg(fraction: f32) -> String {
    let fraction = fraction.clamp(0., 1.);
    let r = 7.0f32;
    let circumference = 2.0 * std::f32::consts::PI * r;
    let offset = circumference * (1.0 - fraction);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20">
  <circle cx="10" cy="10" r="7" fill="none" stroke="currentColor" stroke-width="2.5" opacity="0.25"/>
  <circle cx="10" cy="10" r="7" fill="none" stroke="currentColor" stroke-width="2.5"
    stroke-dasharray="{circumference:.2}" stroke-dashoffset="{offset:.2}"
    stroke-linecap="round" transform="rotate(-90 10 10)"/>
</svg>"##
    )
}

pub fn ring_color(fraction: f32, cx: &App) -> Hsla {
    if fraction > 0.9 {
        cx.theme().danger
    } else if fraction > 0.75 {
        cx.theme().warning
    } else {
        cx.theme().primary
    }
}

pub fn disk_usage_ring(fraction: f32, cx: &App) -> AnyElement {
    let svg = ring_svg(fraction);
    let color = ring_color(fraction, cx);
    let size = px(20.);
    div()
        .id("sb-disk-ring")
        .flex_none()
        .text_color(color)
        .child(
            img(move |_window: &mut Window, cx: &mut App| {
                Some(
                    cx.svg_renderer()
                        .render_single_frame(svg.as_bytes(), 1.0)
                        .map_err(ImageCacheError::from),
                )
            })
            .size(size)
            .object_fit(ObjectFit::Contain),
        )
        .into_any_element()
}
