//! Shared text/UTF-8 helpers for hit-testing and wrapped-line layout.

use gpui::WrappedLine;

pub(crate) const EDITOR_TAB_SIZE: usize = 4;

#[derive(Debug, Clone)]
pub(crate) struct ExpandedTabText {
    pub(crate) text: String,
    char_boundaries: Vec<usize>,
    byte_boundaries: Vec<(usize, usize)>,
}

impl ExpandedTabText {
    pub(crate) fn original_char_to_expanded_byte(&self, char_idx: usize) -> usize {
        self.char_boundaries
            .get(char_idx)
            .copied()
            .unwrap_or_else(|| self.text.len())
    }

    pub(crate) fn original_byte_to_expanded_byte(&self, byte_idx: usize) -> usize {
        self.byte_boundaries
            .iter()
            .find(|(orig, _)| *orig == byte_idx)
            .map(|(_, expanded)| *expanded)
            .unwrap_or_else(|| {
                self.byte_boundaries
                    .iter()
                    .rev()
                    .find(|(orig, _)| *orig <= byte_idx)
                    .map(|(_, expanded)| *expanded)
                    .unwrap_or(0)
            })
    }

    pub(crate) fn expanded_byte_to_original_char(&self, byte_idx: usize) -> usize {
        let byte_idx = byte_idx.min(self.text.len());
        self.char_boundaries
            .iter()
            .position(|expanded| *expanded >= byte_idx)
            .unwrap_or_else(|| self.char_boundaries.len().saturating_sub(1))
    }
}

pub(crate) fn expand_tabs(text: &str, tab_size: usize) -> ExpandedTabText {
    let mut expanded = String::with_capacity(text.len());
    let mut char_boundaries = Vec::with_capacity(text.chars().count() + 1);
    let mut byte_boundaries = Vec::with_capacity(text.chars().count() + 1);
    let mut visual_col = 0usize;

    for (byte_idx, ch) in text.char_indices() {
        char_boundaries.push(expanded.len());
        byte_boundaries.push((byte_idx, expanded.len()));
        if ch == '\t' {
            let width = tab_size - (visual_col % tab_size);
            expanded.extend(std::iter::repeat_n(' ', width));
            visual_col += width;
        } else {
            expanded.push(ch);
            visual_col += 1;
        }
    }

    char_boundaries.push(expanded.len());
    byte_boundaries.push((text.len(), expanded.len()));

    ExpandedTabText {
        text: expanded,
        char_boundaries,
        byte_boundaries,
    }
}

/// Largest byte index `<= byte_idx` that is a valid UTF-8 boundary in `s`.
pub(crate) fn floor_char_boundary(s: &str, byte_idx: usize) -> usize {
    let byte_idx = byte_idx.min(s.len());
    if s.is_char_boundary(byte_idx) {
        byte_idx
    } else {
        (0..=byte_idx)
            .rev()
            .find(|&i| s.is_char_boundary(i))
            .unwrap_or(0)
    }
}

/// Smallest byte index `>= byte_idx` that is a valid UTF-8 boundary in `s`.
pub(crate) fn ceil_char_boundary(s: &str, byte_idx: usize) -> usize {
    let byte_idx = byte_idx.min(s.len());
    if s.is_char_boundary(byte_idx) {
        byte_idx
    } else {
        (byte_idx..=s.len())
            .find(|&i| s.is_char_boundary(i))
            .unwrap_or(s.len())
    }
}

/// Number of visual rows a [`WrappedLine`] occupies.
pub(crate) fn wrap_rows(wrapped: &WrappedLine) -> usize {
    wrapped.wrap_boundaries().len() + 1
}

#[cfg(test)]
mod tests {
    use super::{ceil_char_boundary, expand_tabs, floor_char_boundary, EDITOR_TAB_SIZE};

    #[test]
    fn char_boundary_helpers() {
        let s = "a你b";
        assert_eq!(floor_char_boundary(s, 0), 0);
        assert_eq!(floor_char_boundary(s, 2), 1);
        assert_eq!(ceil_char_boundary(s, 2), 4);
        assert_eq!(ceil_char_boundary(s, 4), 4);
    }

    #[test]
    fn expand_tabs_tracks_boundaries() {
        let expanded = expand_tabs("\tlet", EDITOR_TAB_SIZE);
        assert_eq!(expanded.text, "    let");
        assert_eq!(expanded.original_char_to_expanded_byte(0), 0);
        assert_eq!(expanded.original_char_to_expanded_byte(1), 4);
        assert_eq!(expanded.expanded_byte_to_original_char(2), 1);
        assert_eq!(expanded.expanded_byte_to_original_char(5), 2);
    }
}
