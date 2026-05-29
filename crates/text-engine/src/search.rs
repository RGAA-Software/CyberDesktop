//! In-buffer search and replace (literal or regex, case / whole-word options).
//!
//! Search is line-scoped: each rope line is searched independently. This keeps
//! memory bounded (we only materialize one short line at a time, never the whole
//! file) and matches Notepad++'s default behaviour where patterns do not span
//! line breaks. `find_next` / `find_prev` scan lazily and stop at the first hit,
//! so they stay fast even on huge files.

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::Result;
use regex::Regex;

use regex::bytes::Regex as BytesRegex;

use crate::buffer::TextBuffer;
use crate::rope_scan;

/// Default cap for [`Searcher::find_in_lines`] (Find in File panel).
pub const FIND_IN_FILE_MAX_MATCHES: usize = 10_000;

/// One match when searching a pre-sliced line list (e.g. current tab buffer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineSearchHit {
    pub line_number: u64,
    pub line_text: String,
    pub start: usize,
    pub end: usize,
}

/// Result of a cancellable line-list search.
#[derive(Debug, PartialEq, Eq)]
pub enum FindInLinesOutcome {
    Ok(Vec<LineSearchHit>),
    Cancelled,
}

/// Options controlling how a query is compiled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            whole_word: false,
            regex: false,
        }
    }
}

/// A match expressed in document character offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
}

/// A compiled query ready to run against a [`TextBuffer`].
#[derive(Debug, Clone)]
pub struct Searcher {
    regex: Regex,
    bytes_regex: BytesRegex,
    /// Plain substring (`!regex && !whole_word`): uses `str::find` / `rfind` per line.
    literal: Option<Box<str>>,
    case_sensitive: bool,
}

impl Searcher {
    /// Compiles `query` according to `options`.
    pub fn new(query: &str, options: SearchOptions) -> Result<Self> {
        let mut pattern = if options.regex {
            query.to_string()
        } else {
            regex::escape(query)
        };
        if options.whole_word {
            pattern = format!(r"\b(?:{pattern})\b");
        }
        if !options.case_sensitive {
            pattern = format!("(?i){pattern}");
        }
        let regex = Regex::new(&pattern)?;
        let bytes_regex = BytesRegex::new(&pattern)?;
        let literal = if !options.regex && !options.whole_word && !query.is_empty() {
            Some(query.into())
        } else {
            None
        };
        Ok(Self {
            regex,
            bytes_regex,
            literal,
            case_sensitive: options.case_sensitive,
        })
    }

    fn byte_match(&self, buffer: &TextBuffer, start_byte: usize, end_byte: usize) -> Match {
        Match {
            start: buffer.byte_to_char(start_byte),
            end: buffer.byte_to_char(end_byte),
        }
    }

    /// Fast path: scan rope bytes (Scintilla-style) instead of one line per step.
    fn find_next_literal_rope(
        &self,
        buffer: &TextBuffer,
        from_char: usize,
        limit_start_char: usize,
    ) -> Option<Match> {
        let lit = self.literal.as_deref()?;
        let from_byte = buffer.char_to_byte(from_char);
        let limit = buffer.char_to_byte(limit_start_char);
        let hit = rope_scan::find_forward(
            buffer.rope(),
            lit.as_bytes(),
            &self.bytes_regex,
            self.case_sensitive,
            from_byte,
            limit,
        )?;
        Some(self.byte_match(buffer, hit.0, hit.1))
    }

    fn find_prev_literal_rope(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        let lit = self.literal.as_deref()?;
        let to_byte = buffer.char_to_byte(from_char);
        let hit = rope_scan::find_backward(
            buffer.rope(),
            lit.as_bytes(),
            &self.bytes_regex,
            self.case_sensitive,
            to_byte,
        )?;
        Some(self.byte_match(buffer, hit.0, hit.1))
    }

    /// Finds the first match at or after `from_char`, wrapping around the end.
    pub fn find_next(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        self.find_next_no_wrap(buffer, from_char)
            .or_else(|| self.find_next_wrap(buffer, from_char))
    }

    /// Forward search from `from_char` without wrapping to the start of the file.
    pub fn find_next_no_wrap(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        if self.literal.is_some() {
            return self.find_next_literal_rope(buffer, from_char, usize::MAX);
        }
        let rope = buffer.rope();
        let total = rope.len_chars();
        let from = from_char.min(total);
        let line_count = rope.len_lines();
        let start_line = rope.char_to_line(from);

        for line in start_line..line_count {
            let line_start = rope.line_to_char(line);
            if line == start_line && from <= line_start && buffer.line_len_chars(line) == 0 {
                continue;
            }
            let body = line_body(buffer, line);
            let from_byte = if line == start_line {
                char_to_byte(body.as_ref(), from.saturating_sub(line_start))
            } else {
                0
            };
            if let Some((start, end)) = self.find_forward_in_line(body.as_ref(), from_byte) {
                return Some(to_match(body.as_ref(), line_start, start, end));
            }
        }
        None
    }

    pub fn find_next_wrap(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        if self.literal.is_some() {
            return self.find_next_literal_rope(buffer, 0, from_char);
        }
        let rope = buffer.rope();
        let total = rope.len_chars();
        let from = from_char.min(total);
        let line_count = rope.len_lines();
        let start_line = rope.char_to_line(from);

        for line in 0..=start_line.min(line_count.saturating_sub(1)) {
            let line_start = rope.line_to_char(line);
            let body = line_body(buffer, line);
            let limit_byte = if line == start_line {
                char_to_byte(body.as_ref(), from.saturating_sub(line_start))
            } else {
                body.len()
            };
            if let Some((start, end)) = self.find_forward_in_line(body.as_ref(), 0) {
                if line < start_line || start < limit_byte {
                    return Some(to_match(body.as_ref(), line_start, start, end));
                }
            }
        }
        None
    }

    /// Finds the last match before `from_char`, wrapping around the start.
    pub fn find_prev(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        self.find_prev_no_wrap(buffer, from_char)
            .or_else(|| self.find_prev_wrap(buffer, from_char))
    }

    /// Backward search from `from_char` without wrapping to the end of the file.
    pub fn find_prev_no_wrap(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        if self.literal.is_some() {
            return self.find_prev_literal_rope(buffer, from_char);
        }
        let rope = buffer.rope();
        let total = rope.len_chars();
        let from = from_char.min(total);
        let line_count = rope.len_lines();
        let start_line = rope.char_to_line(from);

        for line in (0..=start_line.min(line_count.saturating_sub(1))).rev() {
            let line_start = rope.line_to_char(line);
            let body = line_body(buffer, line);
            let limit_byte = if line == start_line {
                char_to_byte(body.as_ref(), from.saturating_sub(line_start))
            } else {
                body.len()
            };
            if let Some((start, end)) = self.find_backward_in_line(body.as_ref(), limit_byte) {
                return Some(to_match(body.as_ref(), line_start, start, end));
            }
        }
        None
    }

    pub fn find_prev_wrap(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        if self.literal.is_some() {
            let total = buffer.rope().len_chars();
            return self.find_prev_literal_rope(buffer, total);
        }
        let rope = buffer.rope();
        let total = rope.len_chars();
        let from = from_char.min(total);
        let line_count = rope.len_lines();
        let start_line = rope.char_to_line(from);

        for line in (start_line..line_count).rev() {
            let line_start = rope.line_to_char(line);
            let body = line_body(buffer, line);
            if line == start_line {
                let limit_byte = char_to_byte(body.as_ref(), from.saturating_sub(line_start));
                if let Some((start, end)) = self.find_backward_in_line(body.as_ref(), body.len()) {
                    if start >= limit_byte {
                        return Some(to_match(body.as_ref(), line_start, start, end));
                    }
                }
            } else if let Some((start, end)) =
                self.find_backward_in_line(body.as_ref(), body.len())
            {
                return Some(to_match(body.as_ref(), line_start, start, end));
            }
        }
        None
    }

    fn find_forward_in_line(&self, line: &str, from_byte: usize) -> Option<(usize, usize)> {
        let from_byte = from_byte.min(line.len());
        if let Some(lit) = &self.literal {
            if self.case_sensitive {
                let hay = &line[from_byte..];
                if !hay.contains(lit.as_ref()) {
                    return None;
                }
                let start = from_byte + hay.find(lit.as_ref())?;
                return Some((start, start + lit.len()));
            }
        }
        self.regex
            .find_at(line, from_byte)
            .map(|m| (m.start(), m.end()))
    }

    fn find_backward_in_line(&self, line: &str, limit_byte: usize) -> Option<(usize, usize)> {
        let limit_byte = limit_byte.min(line.len());
        if limit_byte == 0 {
            return None;
        }
        if let Some(lit) = &self.literal {
            if self.case_sensitive {
                let hay = &line[..limit_byte];
                if !hay.contains(lit.as_ref()) {
                    return None;
                }
                let start = hay.rfind(lit.as_ref())?;
                return Some((start, start + lit.len()));
            }
        }
        self.regex
            .find_iter(line)
            .take_while(|m| m.start() < limit_byte)
            .last()
            .map(|m| (m.start(), m.end()))
    }

    /// Counts all matches in the buffer.
    pub fn count(&self, buffer: &TextBuffer) -> usize {
        if let Some(lit) = self.literal.as_deref() {
            return rope_scan::count_all(
                buffer.rope(),
                lit.as_bytes(),
                &self.bytes_regex,
                self.case_sensitive,
            );
        }
        let rope = buffer.rope();
        let mut total = 0;
        for line in 0..rope.len_lines() {
            let body = line_body(buffer, line);
            total += self.regex.find_iter(body.as_ref()).count();
        }
        total
    }

    /// Single-pass count of all matches plus the 1-based index of the match that
    /// starts exactly at `head` (if any). Avoids the two full scans that a
    /// separate `count` + `all_matches().position()` would cost on large files.
    pub fn count_and_index(&self, buffer: &TextBuffer, head: usize) -> (usize, Option<usize>) {
        let rope = buffer.rope();
        let mut total = 0usize;
        let mut index = None;
        for line in 0..rope.len_lines() {
            let line_start = rope.line_to_char(line);
            let body = line_body(buffer, line);
            for m in self.regex.find_iter(body.as_ref()) {
                total += 1;
                let start = line_start + byte_to_char(body.as_ref(), m.start());
                if index.is_none() && start == head {
                    index = Some(total);
                }
            }
        }
        (total, index)
    }

    /// Searches `lines` (one entry per document line) with periodic `cancel` checks.
    pub fn find_in_lines(
        &self,
        lines: &[String],
        cancel: &AtomicBool,
        lines_done: Option<&AtomicUsize>,
        matches_found: Option<&AtomicUsize>,
        max_matches: usize,
    ) -> FindInLinesOutcome {
        let mut out = Vec::new();
        let mut total = 0usize;
        for (line_idx, line) in lines.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return FindInLinesOutcome::Cancelled;
            }
            if let Some(ld) = lines_done {
                if line_idx % 64 == 0 {
                    ld.store(line_idx, Ordering::Relaxed);
                    if let Some(mf) = matches_found {
                        mf.store(total, Ordering::Relaxed);
                    }
                }
            }
            let line_number = (line_idx + 1) as u64;
            let trimmed = line.trim_end_matches(['\r', '\n']);
            for m in self.regex.find_iter(trimmed) {
                out.push(LineSearchHit {
                    line_number,
                    line_text: trimmed.to_string(),
                    start: m.start(),
                    end: m.end(),
                });
                total += 1;
                if total >= max_matches {
                    if let Some(ld) = lines_done {
                        ld.store(lines.len(), Ordering::Relaxed);
                    }
                    if let Some(mf) = matches_found {
                        mf.store(total, Ordering::Relaxed);
                    }
                    return FindInLinesOutcome::Ok(out);
                }
            }
        }
        if let Some(ld) = lines_done {
            ld.store(lines.len(), Ordering::Relaxed);
        }
        if let Some(mf) = matches_found {
            mf.store(total, Ordering::Relaxed);
        }
        FindInLinesOutcome::Ok(out)
    }

    /// Returns all matches in document order.
    pub fn all_matches(&self, buffer: &TextBuffer) -> Vec<Match> {
        let rope = buffer.rope();
        let mut out = Vec::new();
        for line in 0..rope.len_lines() {
            let line_start = rope.line_to_char(line);
            let body = line_body(buffer, line);
            for m in self.regex.find_iter(body.as_ref()) {
                out.push(to_match(
                    body.as_ref(),
                    line_start,
                    m.start(),
                    m.end(),
                ));
            }
        }
        out
    }

    /// Computes the replacement text for a match, honoring regex capture groups
    /// (`$1`, `${name}`) when in regex mode; for literal mode the template is
    /// inserted verbatim.
    pub fn replacement_for(
        &self,
        buffer: &TextBuffer,
        m: Match,
        template: &str,
        regex_mode: bool,
    ) -> String {
        if !regex_mode {
            return template.to_string();
        }
        let matched: String = buffer
            .rope()
            .slice(m.start..m.end)
            .chars()
            .collect();
        let mut out = String::new();
        if let Some(caps) = self.regex.captures(&matched) {
            caps.expand(template, &mut out);
        } else {
            out.push_str(template);
        }
        out
    }
}

/// Line text without trailing CR/LF; borrows from the rope when contiguous.
fn line_body<'a>(buffer: &'a TextBuffer, line: usize) -> Cow<'a, str> {
    let rope = buffer.rope();
    if line >= rope.len_lines() {
        return Cow::Borrowed("");
    }
    let slice = rope.line(line);
    if let Some(s) = slice.as_str() {
        return Cow::Borrowed(trim_line_ending(s));
    }
    Cow::Owned(trim_line_ending_owned(slice.to_string()))
}

fn trim_line_ending(s: &str) -> &str {
    s.trim_end_matches(&['\r', '\n'][..])
}

fn trim_line_ending_owned(mut s: String) -> String {
    while s.ends_with('\n') || s.ends_with('\r') {
        s.pop();
    }
    s
}

fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

fn byte_to_char(s: &str, byte_idx: usize) -> usize {
    s[..byte_idx.min(s.len())].chars().count()
}

fn to_match(s: &str, line_start_char: usize, start_byte: usize, end_byte: usize) -> Match {
    Match {
        start: line_start_char + byte_to_char(s, start_byte),
        end: line_start_char + byte_to_char(s, end_byte),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_find_next_and_wrap() {
        let buf = TextBuffer::from_str("foo bar foo");
        let s = Searcher::new("foo", SearchOptions::default()).unwrap();
        let m1 = s.find_next(&buf, 0).unwrap();
        assert_eq!((m1.start, m1.end), (0, 3));
        let m2 = s.find_next(&buf, m1.end).unwrap();
        assert_eq!((m2.start, m2.end), (8, 11));
        // wrap
        let m3 = s.find_next(&buf, m2.end).unwrap();
        assert_eq!((m3.start, m3.end), (0, 3));
    }

    #[test]
    fn case_insensitive_default() {
        let buf = TextBuffer::from_str("Foo");
        let s = Searcher::new("foo", SearchOptions::default()).unwrap();
        assert!(s.find_next(&buf, 0).is_some());
    }

    #[test]
    fn whole_word() {
        let buf = TextBuffer::from_str("foobar foo");
        let opts = SearchOptions {
            whole_word: true,
            ..Default::default()
        };
        let s = Searcher::new("foo", opts).unwrap();
        let m = s.find_next(&buf, 0).unwrap();
        assert_eq!((m.start, m.end), (7, 10));
    }

    #[test]
    fn count_and_all_matches_multiline() {
        let buf = TextBuffer::from_str("a\nba\nbba\n");
        let s = Searcher::new("a", SearchOptions::default()).unwrap();
        assert_eq!(s.count(&buf), 3);
        assert_eq!(s.all_matches(&buf).len(), 3);
    }

    #[test]
    fn regex_capture_replacement() {
        let buf = TextBuffer::from_str("name=value");
        let opts = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let s = Searcher::new(r"(\w+)=(\w+)", opts).unwrap();
        let m = s.find_next(&buf, 0).unwrap();
        let rep = s.replacement_for(&buf, m, "$2=$1", true);
        assert_eq!(rep, "value=name");
    }

    #[test]
    fn count_many_lines_literal() {
        let mut text = String::new();
        for _ in 0..20_000 {
            text.push_str("x\n");
        }
        text.push_str("needle\nneedle\n");
        let buf = TextBuffer::from_str(&text);
        let s = Searcher::new("needle", SearchOptions::default()).unwrap();
        assert_eq!(s.count(&buf), 2);
    }

    #[test]
    fn find_next_after_many_lines() {
        let mut text = String::new();
        for _ in 0..20_000 {
            text.push_str("x\n");
        }
        text.push_str("needle\n");
        let buf = TextBuffer::from_str(&text);
        let s = Searcher::new("needle", SearchOptions::default()).unwrap();
        let m = s.find_next_no_wrap(&buf, 0).unwrap();
        assert_eq!(
            buf.slice_text(m.start..m.end),
            "needle".to_string()
        );
    }

    #[test]
    fn find_in_lines_cancelled() {
        let lines = vec!["aaa".into(), "bbb".into(), "ccc".into()];
        let s = Searcher::new("a", SearchOptions::default()).unwrap();
        let cancel = AtomicBool::new(false);
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            s.find_in_lines(&lines, &cancel, None, None, 100),
            FindInLinesOutcome::Cancelled
        );
    }

    #[test]
    fn find_in_lines_collects_hits() {
        let lines = vec!["foo bar".into(), "no".into(), "foo".into()];
        let s = Searcher::new("foo", SearchOptions::default()).unwrap();
        let cancel = AtomicBool::new(false);
        let FindInLinesOutcome::Ok(hits) = s.find_in_lines(&lines, &cancel, None, None, 100)
        else {
            panic!("expected Ok");
        };
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line_number, 1);
        assert_eq!(hits[1].line_number, 3);
    }

    #[test]
    fn find_prev_basic() {
        let buf = TextBuffer::from_str("foo foo foo");
        let s = Searcher::new("foo", SearchOptions::default()).unwrap();
        let m = s.find_prev(&buf, 11).unwrap();
        assert_eq!((m.start, m.end), (8, 11));
        let m2 = s.find_prev(&buf, m.start).unwrap();
        assert_eq!((m2.start, m2.end), (4, 7));
    }
}
