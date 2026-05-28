//! Minimal initialization and helpers for embedding Zed's [`editor::Editor`] in CyberEditor.

mod languages;

use std::path::Path;

use editor::{Editor, EditorElement, EditorMode, MultiBuffer};
use gpui::{
    App, AppContext, Context, Entity, Focusable as _, IntoElement, ParentElement, Styled, Window,
    div,
};
use language::Buffer;
use settings::SettingsStore;
use theme::{ActiveTheme as _, ThemeRegistry};
use theme_settings;

pub use languages::{
    init_language_registry, language_registry, lookup_key_for_language_id, sync_language_settings,
};

/// Register globals required before creating an [`Editor`] (settings + base theme + languages).
pub fn init(cx: &mut App) {
    if !cx.has_global::<SettingsStore>() {
        let store = SettingsStore::new(cx, &settings::default_settings());
        cx.set_global(store);
    }
    theme_settings::init(theme::LoadThemes::All(Box::new(assets::Assets)), cx);
    theme_settings::load_bundled_themes(&ThemeRegistry::global(cx));
    init_language_registry(cx);
    sync_language_registry_theme(cx);
    if let Err(err) = assets::Assets.load_fonts(cx) {
        log::warn!("failed to load editor fonts: {err:#}");
    }
    init_editor_keymap(cx);
    // Push language settings on the next async turn to avoid re-entering GPUI during init.
    cx.spawn(async move |cx| {
        cx.update(|cx| {
            sync_language_settings(cx);
        });
    })
    .detach();
}

fn sync_language_registry_theme(cx: &App) {
    language_registry(cx).set_theme(cx.theme().clone());
}

/// Bind Zed's default editor keymap (arrows, backspace, cut/copy/paste, etc.).
fn init_editor_keymap(cx: &mut App) {
    let key_bindings = settings::KeymapFile::load_asset_allow_partial_failure(
        settings::DEFAULT_KEYMAP_PATH,
        cx,
    )
    .expect("failed to load built-in editor keymap");
    cx.bind_keys(key_bindings);
}

/// Create a full-mode editor over a single in-memory buffer.
pub fn create_editor<T>(
    text: String,
    language_id: &str,
    file_path: Option<&Path>,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Entity<Editor> {
    let editor = cx.new(|cx| {
        let registry = language_registry(cx);
        let buffer = cx.new(|cx| {
            let buffer = Buffer::local(text, cx);
            buffer.set_language_registry(registry);
            buffer
        });
        let registry = language_registry(cx);
        let language = languages::load_language_blocking(&registry, language_id, file_path);
        buffer.update(cx, |buffer, cx| {
            buffer.set_language_registry(registry);
            buffer.set_language(language, cx);
        });
        let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
        Editor::new(EditorMode::full(), multibuffer, None, window, cx)
    });
    editor
}

/// Set tree-sitter language on the singleton buffer behind an editor.
pub fn apply_editor_language<C: AppContext>(
    editor: &Entity<Editor>,
    language_id: &str,
    file_path: Option<&Path>,
    cx: &mut C,
) {
    let registry = language_registry(cx);
    let language = languages::load_language_blocking(&registry, language_id, file_path);
    editor.update(cx, |editor, cx| {
        let multibuffer = editor.buffer();
        let Some(buffer) = multibuffer.read(cx).as_singleton() else {
            return;
        };
        buffer.update(cx, |buffer, cx| {
            buffer.set_language_registry(registry);
            buffer.set_language(language, cx);
        });
    });
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

/// Mark the singleton buffer as saved (clears dirty in the engine).
pub fn mark_editor_saved<C: AppContext>(editor: &Entity<Editor>, cx: &mut C) {
    editor.update(cx, |editor, cx| {
        let multibuffer = editor.buffer();
        let Some(buffer) = multibuffer.read(cx).as_singleton() else {
            return;
        };
        buffer.update(cx, |buffer, cx| {
            let version = buffer.version().clone();
            buffer.did_save(version, None, cx);
        });
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
