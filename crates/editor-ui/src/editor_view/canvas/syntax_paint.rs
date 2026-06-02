//! Syntax-colored text runs and occurrence highlighting for the editor canvas.

use std::ops::Range;

use editor_text_engine::{Document, HighlightKind, SyntaxState, TextBuffer};
use gpui::{Font, Hsla, Pixels, SharedString, TextRun, Window, WrappedLine, rgb};

use super::super::text_util::{ceil_char_boundary, floor_char_boundary, ExpandedTabText, wrap_rows};

/// Shapes `text` wrapped to `width`, returning its single logical [`WrappedLine`].
pub(super) fn shape_one_wrapped(
    window: &mut Window,
    text: &str,
    runs: &[TextRun],
    font_size: Pixels,
    width: Pixels,
) -> Option<WrappedLine> {
    window
        .text_system()
        .shape_text(
            SharedString::from(text.to_string()),
            font_size,
            runs,
            Some(width),
            None,
        )
        .ok()?
        .into_iter()
        .next()
}

/// Visual-row count of `text` wrapped to `width` (single-run; matches layout).
pub(super) fn measure_rows(
    window: &mut Window,
    text: &str,
    font: &Font,
    font_size: Pixels,
    width: Pixels,
) -> usize {
    if width <= gpui::px(0.0) || text.is_empty() {
        return 1;
    }
    let run = TextRun {
        len: text.len(),
        font: font.clone(),
        color: rgb(0xffffff).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    shape_one_wrapped(window, text, &[run], font_size, width)
        .map(|w| wrap_rows(&w))
        .unwrap_or(1)
}

/// Builds colored [`TextRun`]s for one line from the syntax highlights.
pub(super) fn build_runs(
    syntax: &SyntaxState,
    buffer: &TextBuffer,
    line_text: &str,
    expanded: Option<&ExpandedTabText>,
    line_start_byte: usize,
    font: &Font,
    default_color: Hsla,
) -> Vec<TextRun> {
    let display_text = expanded.map(|expanded| expanded.text.as_str()).unwrap_or(line_text);
    let len = display_text.len();
    let frag_end_byte = line_start_byte + line_text.len();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut pos = 0usize;

    let mk = |len: usize, color: Hsla| TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };

    for span in syntax.highlights_rope(buffer.rope(), line_start_byte..frag_end_byte) {
        let abs_start = span.start.max(line_start_byte).min(frag_end_byte);
        let abs_end = span.end.max(line_start_byte).min(frag_end_byte);
        if abs_end <= abs_start {
            continue;
        }
        let rel_start = abs_start - line_start_byte;
        let rel_end = abs_end - line_start_byte;
        let mapped_start = expanded
            .map(|expanded| expanded.original_byte_to_expanded_byte(rel_start))
            .unwrap_or(rel_start);
        let mapped_end = expanded
            .map(|expanded| expanded.original_byte_to_expanded_byte(rel_end))
            .unwrap_or(rel_end);
        let mut start = ceil_char_boundary(display_text, mapped_start);
        let mut end = floor_char_boundary(display_text, mapped_end);
        if end < mapped_end {
            end = ceil_char_boundary(display_text, mapped_end);
        }
        start = start.max(pos).min(len);
        end = end.max(start).min(len);
        if end <= start {
            continue;
        }
        if start > pos {
            runs.push(mk(start - pos, default_color));
        }
        runs.push(mk(end - start, kind_color(span.kind)));
        pos = end;
    }
    if pos < len {
        runs.push(mk(len - pos, default_color));
    }
    if runs.is_empty() && len > 0 {
        runs.push(mk(len, default_color));
    }
    runs
}

/// Char-column ranges of whole-word, case-sensitive matches of `needle` in `line_text`.
pub(super) fn word_occurrences(line_text: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() || needle.len() > line_text.len() {
        return out;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0usize;
    while let Some(rel) = line_text[from..].find(needle) {
        let bstart = from + rel;
        let bend = bstart + needle.len();
        let before_ok = line_text[..bstart]
            .chars()
            .next_back()
            .map_or(true, |c| !is_word(c));
        let after_ok = line_text[bend..].chars().next().map_or(true, |c| !is_word(c));
        if before_ok && after_ok {
            let scol = line_text[..bstart].chars().count();
            let ecol = line_text[..bend].chars().count();
            out.push((scol, ecol));
        }
        from = bend.max(bstart + 1);
    }
    out
}

/// The selected text to highlight occurrences of (single non-empty word-like selection).
pub(super) fn occurrence_word(document: &Document) -> Option<(String, Range<usize>)> {
    let sels = document.selections();
    if sels.len() != 1 {
        return None;
    }
    let p = sels.primary();
    if p.is_empty() {
        return None;
    }
    let text = document.buffer().slice_text(p.range());
    let count = text.chars().count();
    if count == 0 || count > 100 || !text.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some((text, p.range()))
}

fn kind_color(kind: HighlightKind) -> Hsla {
    let rgb_value = match kind {
        HighlightKind::Keyword => 0x569cd6,
        HighlightKind::Function => 0xdcdcaa,
        HighlightKind::Type => 0x4ec9b0,
        HighlightKind::String => 0xce9178,
        HighlightKind::Number => 0xb5cea8,
        HighlightKind::Comment => 0x6a9955,
        HighlightKind::Constant => 0x569cd6,
        HighlightKind::Variable => 0x9cdcfe,
        HighlightKind::Property => 0x9cdcfe,
        HighlightKind::Operator => 0xd4d4d4,
        HighlightKind::Punctuation => 0xd4d4d4,
        HighlightKind::Tag => 0x569cd6,
        HighlightKind::Attribute => 0x9cdcfe,
        HighlightKind::Label => 0xc8c8c8,
        HighlightKind::Constructor => 0x4ec9b0,
        HighlightKind::Other => 0xd4d4d4,
    };
    rgb(rgb_value).into()
}
