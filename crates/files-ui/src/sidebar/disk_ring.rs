use gpui::{
    div, img, prelude::*, px, AnyElement, App, Hsla, ImageCacheError, IntoElement, ObjectFit,
    ParentElement, Styled, Window,
};
use gpui_component::ActiveTheme as _;

fn hsla_to_svg_hex(color: Hsla) -> String {
    let rgba = color.to_rgb();
    let r = (rgba.r * 255.0).round() as u8;
    let g = (rgba.g * 255.0).round() as u8;
    let b = (rgba.b * 255.0).round() as u8;
    let a = (rgba.a * 255.0).round() as u8;
    if a == 255 {
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a)
    }
}

fn ring_svg(fraction: f32, arc_color: Hsla, track_color: Hsla) -> String {
    let fraction = fraction.clamp(0., 1.);
    let r = 7.0f32;
    let circumference = 2.0 * std::f32::consts::PI * r;
    let offset = circumference * (1.0 - fraction);
    let arc = hsla_to_svg_hex(arc_color);
    let track = hsla_to_svg_hex(track_color);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20">
  <circle cx="10" cy="10" r="7" fill="none" stroke="{track}" stroke-width="2.5"/>
  <circle cx="10" cy="10" r="7" fill="none" stroke="{arc}" stroke-width="2.5"
    stroke-dasharray="{circumference:.2}" stroke-dashoffset="{offset:.2}"
    stroke-linecap="round" transform="rotate(-90 10 10)"/>
</svg>"##
    )
}

pub fn drive_usage_color(fraction: f32, cx: &App) -> Hsla {
    let fraction = fraction.clamp(0., 1.);
    if fraction > 0.8 {
        cx.theme().danger
    } else if fraction >= 0.6 {
        cx.theme().warning
    } else {
        cx.theme().primary
    }
}

pub fn disk_usage_ring(fraction: f32, cx: &App) -> AnyElement {
    let arc_color = drive_usage_color(fraction, cx);
    let track_color = cx.theme().border.opacity(0.45);
    let svg = ring_svg(fraction, arc_color, track_color);
    let size = px(20.);
    div()
        .id("sb-disk-ring")
        .flex_none()
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
