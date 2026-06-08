//! Render MarkdownDocument as GPUI elements.

use gpui::*;
use gpui_component::{v_flex, h_flex, ActiveTheme};

use crate::{MarkdownBlock, MarkdownDocument, MarkdownInline};

/// Render a markdown document as a GPUI element.
pub fn render_markdown(doc: &MarkdownDocument, _cx: &App) -> AnyElement {
    let blocks: Vec<AnyElement> = doc.blocks.iter()
        .map(|block| render_block(block, _cx))
        .collect();

    v_flex()
        .gap(px(12.))
        .px(px(16.))
        .py(px(16.))
        .children(blocks)
        .into_any_element()
}

fn render_block(block: &MarkdownBlock, cx: &App) -> AnyElement {
    match block {
        MarkdownBlock::Heading(level, inlines, _) => {
            let size = match level {
                1 => px(28.),
                2 => px(24.),
                3 => px(20.),
                4 => px(18.),
                5 => px(16.),
                _ => px(14.),
            };
            v_flex()
                .child(
                    h_flex()
                        .flex_wrap()
                        .text_size(size)
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().primary)
                        .children(inlines.iter().map(|i| render_inline(i, cx)))
                )
                .into_any_element()
        }
        MarkdownBlock::Paragraph(inlines, _) => {
            h_flex()
                .flex_wrap()
                .text_sm()
                .text_color(cx.theme().foreground)
                .children(inlines.iter().map(|i| render_inline(i, cx)))
                .into_any_element()
        }
        MarkdownBlock::CodeBlock(lang, code, _) => {
            let mut container = v_flex();

            if let Some(l) = lang {
                container = container.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .mb(px(4.))
                        .child(SharedString::from(l.clone()))
                );
            }

            let code_element = div()
                .bg(cx.theme().muted)
                .rounded_md()
                .p(px(12.))
                .child(
                    div()
                        .text_sm()
                        .font_family("JetBrains Mono")
                        .text_color(cx.theme().foreground)
                        .child(SharedString::from(code.clone()))
                );

            container.child(code_element).into_any_element()
        }
        MarkdownBlock::BlockQuote(inner, _) => {
            v_flex()
                .border_l_2()
                .border_color(cx.theme().border)
                .pl(px(12.))
                .gap(px(8.))
                .text_color(cx.theme().muted_foreground)
                .children(inner.iter().map(|b| render_block(b, cx)))
                .into_any_element()
        }
        MarkdownBlock::Rule(_) => {
            div()
                .h(px(1.))
                .bg(cx.theme().border)
                .w_full()
                .into_any_element()
        }
    }
}

fn render_inline(inline: &MarkdownInline, cx: &App) -> AnyElement {
    match inline {
        MarkdownInline::Text(text) => {
            div().child(SharedString::from(text.clone())).into_any_element()
        }
        MarkdownInline::Code(code) => {
            div()
                .bg(cx.theme().muted)
                .rounded_sm()
                .px(px(4.))
                .py(px(2.))
                .text_sm()
                .font_family("JetBrains Mono")
                .text_color(cx.theme().primary)
                .child(SharedString::from(code.clone()))
                .into_any_element()
        }
        MarkdownInline::Link(url, text) => {
            let url = url.clone();
            div()
                .id("md-link")
                .text_color(cx.theme().primary)
                .cursor_pointer()
                .child(SharedString::from(text.clone()))
                .on_click(move |_, _, cx: &mut App| {
                    cx.open_url(&url);
                })
                .into_any_element()
        }
        MarkdownInline::Image(url, alt) => {
            // V1: show placeholder for images
            div()
                .bg(cx.theme().muted)
                .rounded_md()
                .p(px(8.))
                .child(
                    v_flex()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from(format!("[Image: {}]", alt)))
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from(url.clone()))
                        )
                )
                .into_any_element()
        }
    }
}
