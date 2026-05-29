//! Substring search across a [`ropey::Rope`] in UTF-8 byte space (chunk by chunk).
//!
//! Notepad++/Scintilla search the document as one contiguous target; this module
//! does the same without materializing the whole file or walking every line.

use memchr::memmem::Finder;
use regex::bytes::Regex as BytesRegex;
use ropey::Rope;

/// Bytes retained between rope chunks so regex / case-insensitive matches split at boundaries are not lost.
pub const PATTERN_CHUNK_OVERLAP: usize = 255;

fn overlap_bytes(needle: &[u8], case_sensitive_literal: bool) -> usize {
    if case_sensitive_literal {
        needle.len().saturating_sub(1)
    } else if needle.is_empty() {
        PATTERN_CHUNK_OVERLAP
    } else {
        needle.len().saturating_sub(1).max(PATTERN_CHUNK_OVERLAP)
    }
}

/// First match at/after `from_byte` (match start must be `< limit_start`).
///
/// `case_sensitive_literal`: memmem on `needle`. Otherwise `bytes_pat` (regex, whole-word, or CI literal).
pub fn find_forward(
    rope: &Rope,
    needle: &[u8],
    bytes_pat: &BytesRegex,
    case_sensitive_literal: bool,
    from_byte: usize,
    limit_start: usize,
) -> Option<(usize, usize)> {
    if case_sensitive_literal && needle.is_empty() {
        return None;
    }
    let total = rope.len_bytes();
    if from_byte >= total {
        return None;
    }
    let overlap = overlap_bytes(needle, case_sensitive_literal);
    if case_sensitive_literal {
        let finder = Finder::new(needle);
        scan(rope, from_byte, total, overlap, |window, window_lo| {
            if let Some(rel) = finder.find(window) {
                let start = window_lo + rel;
                let end = start + needle.len();
                if start < limit_start && valid_line_match(window, rel, needle.len()) {
                    return Some((start, end));
                }
            }
            None
        })
    } else {
        scan(rope, from_byte, total, overlap, |window, window_lo| {
            for m in bytes_pat.find_iter(window) {
                let start = window_lo + m.start();
                let end = window_lo + m.end();
                if start >= limit_start {
                    return None;
                }
                if valid_line_match(window, m.start(), m.len()) {
                    return Some((start, end));
                }
            }
            None
        })
    }
}

/// Last `needle` with end `<= to_byte` before the cursor.
pub fn find_backward(
    rope: &Rope,
    needle: &[u8],
    bytes_pat: &BytesRegex,
    case_sensitive_literal: bool,
    to_byte: usize,
) -> Option<(usize, usize)> {
    if to_byte == 0 || (case_sensitive_literal && needle.is_empty()) {
        return None;
    }
    let to_byte = to_byte.min(rope.len_bytes());
    let overlap = overlap_bytes(needle, case_sensitive_literal);
    let mut last = None;
    if case_sensitive_literal {
        let finder = Finder::new(needle);
        scan(rope, 0, to_byte, overlap, |window, window_lo| {
            for rel in finder.find_iter(window) {
                let start = window_lo + rel;
                let end = start + needle.len();
                if end <= to_byte && valid_line_match(window, rel, needle.len()) {
                    last = Some((start, end));
                }
            }
            None
        });
    } else {
        scan(rope, 0, to_byte, overlap, |window, window_lo| {
            for m in bytes_pat.find_iter(window) {
                let start = window_lo + m.start();
                let end = window_lo + m.end();
                if end <= to_byte && valid_line_match(window, m.start(), m.len()) {
                    last = Some((start, end));
                }
            }
            None
        });
    }
    last
}

/// Last match with `start >= from_byte` and `end <= to_byte` (wrap segment after the cursor).
pub fn find_last_from(
    rope: &Rope,
    needle: &[u8],
    bytes_pat: &BytesRegex,
    case_sensitive_literal: bool,
    from_byte: usize,
) -> Option<(usize, usize)> {
    if case_sensitive_literal && needle.is_empty() {
        return None;
    }
    let to_byte = rope.len_bytes();
    if from_byte >= to_byte {
        return None;
    }
    let overlap = overlap_bytes(needle, case_sensitive_literal);
    let mut last = None;
    if case_sensitive_literal {
        let finder = Finder::new(needle);
        scan(rope, from_byte, to_byte, overlap, |window, window_lo| {
            for rel in finder.find_iter(window) {
                let start = window_lo + rel;
                let end = start + needle.len();
                if start >= from_byte
                    && end <= to_byte
                    && valid_line_match(window, rel, needle.len())
                {
                    last = Some((start, end));
                }
            }
            None
        });
    } else {
        scan(rope, from_byte, to_byte, overlap, |window, window_lo| {
            for m in bytes_pat.find_iter(window) {
                let start = window_lo + m.start();
                let end = window_lo + m.end();
                if start >= from_byte
                    && end <= to_byte
                    && valid_line_match(window, m.start(), m.len())
                {
                    last = Some((start, end));
                }
            }
            None
        });
    }
    last
}

fn valid_line_match(window: &[u8], rel_start: usize, len: usize) -> bool {
    window[rel_start..rel_start + len]
        .iter()
        .all(|&b| b != b'\n' && b != b'\r')
}

/// Count all line-scoped matches in the rope (Notepad++ Count / `ProcessCountAll`).
pub fn count_all(
    rope: &Rope,
    needle: &[u8],
    bytes_pat: &BytesRegex,
    case_sensitive_literal: bool,
) -> usize {
    if case_sensitive_literal && needle.is_empty() {
        return 0;
    }
    let total_bytes = rope.len_bytes();
    if total_bytes == 0 {
        return 0;
    }
    let overlap = overlap_bytes(needle, case_sensitive_literal);
    let finder = case_sensitive_literal.then(|| Finder::new(needle));
    let mut count = 0usize;
    let mut chunk_abs = 0usize;
    let mut tail = Vec::new();

    for chunk in rope.byte_slice(0..total_bytes).chunks() {
        let bytes = chunk.as_bytes();
        let window_lo = chunk_abs.saturating_sub(tail.len());
        let mut window = std::mem::take(&mut tail);
        window.extend_from_slice(bytes);

        if case_sensitive_literal {
            if let Some(finder) = &finder {
                for rel in finder.find_iter(&window) {
                    let abs = window_lo + rel;
                    if abs >= chunk_abs && valid_line_match(&window, rel, needle.len()) {
                        count += 1;
                    }
                }
            }
        } else {
            for m in bytes_pat.find_iter(&window) {
                let abs = window_lo + m.start();
                if abs >= chunk_abs && valid_line_match(&window, m.start(), m.len()) {
                    count += 1;
                }
            }
        }

        if overlap > 0 && window.len() > overlap {
            tail.extend_from_slice(&window[window.len() - overlap..]);
        }
        chunk_abs += bytes.len();
    }
    count
}

fn scan(
    rope: &Rope,
    from_byte: usize,
    to_byte: usize,
    overlap: usize,
    mut try_window: impl FnMut(&[u8], usize) -> Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    let to_byte = to_byte.min(rope.len_bytes());
    if from_byte >= to_byte {
        return None;
    }
    let mut chunk_abs = from_byte;
    let mut tail = Vec::new();

    for chunk in rope.byte_slice(from_byte..to_byte).chunks() {
        let bytes = chunk.as_bytes();
        let window_lo = chunk_abs.saturating_sub(tail.len());
        let mut window = std::mem::take(&mut tail);
        window.extend_from_slice(bytes);

        if let Some(hit) = try_window(&window, window_lo) {
            return Some(hit);
        }

        if overlap > 0 && window.len() > overlap {
            tail.extend_from_slice(&window[window.len() - overlap..]);
        }
        chunk_abs += bytes.len();
    }
    None
}
