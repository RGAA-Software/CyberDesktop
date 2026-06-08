//! Markdown preview UI component.
//!
//! Integrates markdown preview with the editor.

use std::time::Duration;

use gpui::*;
use gpui_component::{v_flex, ActiveTheme};
use markdown_preview_engine::{MarkdownDocument, render::render_markdown};

/// Action to toggle markdown preview.
#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(namespace = markdown_preview)]
pub struct ToggleMarkdownPreview;

/// Markdown preview view that renders parsed markdown.
pub struct MarkdownPreviewView {
    focus_handle: FocusHandle,
    document: MarkdownDocument,
    debounce_timer: Option<Task<()>>,
}

impl MarkdownPreviewView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            document: MarkdownDocument::parse(""),
            debounce_timer: None,
        }
    }

    pub fn update_from_text(&mut self, text: String, cx: &mut Context<Self>) {
        // Cancel pending timer
        self.debounce_timer = None;

        // Debounce: wait 200ms before re-parsing
        let text_clone = text.clone();
        self.debounce_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            this.update(cx, |this, cx| {
                this.document = MarkdownDocument::parse(text_clone);
                cx.notify();
            })
            .ok();
        }));
    }
}

impl Focusable for MarkdownPreviewView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MarkdownPreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("markdown-preview")
            .size_full()
            .bg(cx.theme().background)
            .overflow_y_scroll()
            .child(render_markdown(&self.document, cx))
    }
}

/// Create a markdown preview entity.
pub fn markdown_preview_view(cx: &mut App) -> Entity<MarkdownPreviewView> {
    cx.new(|cx| MarkdownPreviewView::new(cx))
}
