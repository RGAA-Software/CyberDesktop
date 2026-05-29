//! Shared text/UTF-8 helpers for hit-testing and wrapped-line layout.

use gpui::WrappedLine;

/// Byte offset of the `char_idx`-th character in `s`.
pub(crate) fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// Number of visual rows a [`WrappedLine`] occupies.
pub(crate) fn wrap_rows(wrapped: &WrappedLine) -> usize {
    wrapped.wrap_boundaries().len() + 1
}
