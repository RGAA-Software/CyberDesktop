//! Minimal initialization and helpers for embedding Zed's [`editor::Editor`] in CyberEditor.

use editor::{Editor, EditorElement, EditorMode, MultiBuffer};
use gpui::{
    App, AppContext, Context, Entity, Focusable as _, IntoElement, ParentElement, Styled, Window,
    div,
};
use language::Buffer;
use settings::SettingsStore;
use theme_settings;

/// Register globals required before creating an [`Editor`] (settings + base theme).
pub fn init(cx: &mut App) {
    if !cx.has_global::<SettingsStore>() {
        let store = SettingsStore::new(cx, &settings::default_settings());
        cx.set_global(store);
    }
    theme_settings::init(theme::LoadThemes::JustBase, cx);
}

/// Create a full-mode editor over a single in-memory buffer.
pub fn create_editor<T>(
    text: String,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Entity<Editor> {
    cx.new(|cx| {
        let buffer = cx.new(|cx| Buffer::local(text, cx));
        let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
        Editor::new(EditorMode::full(), multibuffer, None, window, cx)
    })
}

/// Replace the entire document text in an existing editor.
pub fn set_editor_text(
    editor: &Entity<Editor>,
    text: &str,
    window: &mut Window,
    cx: &mut impl AppContext,
) {
    editor.update(cx, |editor, cx| {
        editor.set_text(text, window, cx);
    });
}

/// Read the full buffer text.
pub fn editor_text(editor: &Entity<Editor>, cx: &App) -> String {
    editor.read(cx).text(cx)
}

/// Focus the editor on the next frame.
pub fn focus_editor_deferred<T>(
    editor: &Entity<Editor>,
    window: &mut Window,
    cx: &mut Context<T>,
) {
    let focus = editor.focus_handle(cx);
    window.defer(cx, move |window, cx| {
        focus.focus(window, cx);
    });
}

/// Render the editor element, filling available space.
pub fn render_editor<T>(
    editor: &Entity<Editor>,
    cx: &mut Context<T>,
) -> impl IntoElement {
    let style = editor.update(cx, |editor, cx| editor.style(cx).clone());
    div()
        .size_full()
        .min_h_0()
        .min_w_0()
        .child(EditorElement::new(editor, style))
}
