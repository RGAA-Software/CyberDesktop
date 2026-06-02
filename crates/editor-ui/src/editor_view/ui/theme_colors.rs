//! Theme-derived colors for the editor surface and chrome.

use gpui::Hsla;
use gpui_component::ActiveTheme as _;

/// Colors read from the active gpui-component theme (Zed JSON highlight + UI palette).
#[derive(Debug, Clone, Copy)]
pub(crate) struct EditorColors {
    pub background: Hsla,
    pub foreground: Hsla,
    pub line_number: Hsla,
    pub active_line_number: Hsla,
    pub gutter_hover: Hsla,
    pub selection: Hsla,
    pub occurrence: Hsla,
    pub caret: Hsla,
}

impl EditorColors {
    pub fn from_app(cx: &gpui::App) -> Self {
        let theme = cx.theme();
        let style = &theme.highlight_theme.style;
        let selection = theme.selection;
        Self {
            background: style
                .editor_background
                .unwrap_or_else(|| theme.input_background()),
            foreground: style.editor_foreground.unwrap_or(theme.foreground),
            line_number: style
                .editor_line_number
                .unwrap_or(theme.muted_foreground),
            active_line_number: style
                .editor_active_line_number
                .unwrap_or(theme.foreground),
            gutter_hover: theme.list_hover,
            selection,
            occurrence: selection.alpha((selection.a * 0.45).min(0.35)),
            caret: theme.caret,
        }
    }
}
