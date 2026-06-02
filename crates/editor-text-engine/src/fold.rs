//! Indentation-based code folding (crease detection).

use crate::buffer::TextBuffer;

/// An active fold: `header_line` stays visible; `(header_line+1)..=end_line` are hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldRange {
    pub header_line: usize,
    pub end_line: usize,
}

impl FoldRange {
    pub fn new(header_line: usize, end_line: usize) -> Self {
        Self {
            header_line,
            end_line,
        }
    }

    pub fn contains_hidden_line(&self, line: usize) -> bool {
        line > self.header_line && line <= self.end_line
    }
}

/// Leading whitespace column count (tabs count as one column).
pub fn line_indent(buf: &TextBuffer, line: usize) -> usize {
    let text = buf.line_text(line);
    text.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

pub fn is_blank_line(buf: &TextBuffer, line: usize) -> bool {
    buf.line_text(line).chars().all(|c| c.is_whitespace())
}

/// True when `line` starts an indent-delimited block.
pub fn starts_indent(buf: &TextBuffer, line: usize) -> bool {
    if is_blank_line(buf, line) {
        return false;
    }
    let start = line_indent(buf, line);
    let count = buf.line_count();
    for l in (line + 1)..count {
        if is_blank_line(buf, l) {
            continue;
        }
        return line_indent(buf, l) > start;
    }
    false
}

/// Indentation-based foldable region starting at `line`, if any.
pub fn crease_at_line(buf: &TextBuffer, line: usize) -> Option<FoldRange> {
    if !starts_indent(buf, line) {
        return None;
    }
    let start_indent = line_indent(buf, line);
    let mut end = line;
    for l in (line + 1)..buf.line_count() {
        if is_blank_line(buf, l) {
            continue;
        }
        if line_indent(buf, l) <= start_indent {
            break;
        }
        end = l;
    }
    if end > line {
        Some(FoldRange::new(line, end))
    } else {
        None
    }
}

pub fn is_line_hidden(line: usize, folds: &[FoldRange]) -> bool {
    folds.iter().any(|f| f.contains_hidden_line(line))
}

pub fn fold_header(line: usize, folds: &[FoldRange]) -> Option<&FoldRange> {
    folds.iter().find(|f| f.header_line == line)
}

pub fn is_folded_header(line: usize, folds: &[FoldRange]) -> bool {
    fold_header(line, folds).is_some()
}

/// Buffer lines visible in the editor (skips folded body lines).
pub fn build_display_lines(buf: &TextBuffer, folds: &[FoldRange]) -> Vec<usize> {
    (0..buf.line_count())
        .filter(|&line| !is_line_hidden(line, folds))
        .collect()
}

pub fn display_line_count(buf: &TextBuffer, folds: &[FoldRange]) -> usize {
    build_display_lines(buf, folds).len().max(1)
}
