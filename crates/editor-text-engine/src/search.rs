//! In-buffer search and replace (literal or regex, case / whole-word options).
//!
//! Matches are line-scoped (patterns do not span `\n`/`\r`), matching Notepad++.
//! Find / Count scan the rope in UTF-8 byte chunks via [`crate::rope_scan`] instead
//! of materializing every line.

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
    /// Plain substring (`!regex && !whole_word`): memmem fast path when case-sensitive.
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

    fn case_sensitive_literal(&self) -> bool {
        self.literal.is_some() && self.case_sensitive
    }

    fn rope_needle(&self) -> &[u8] {
        self.literal
            .as_deref()
            .map(str::as_bytes)
            .unwrap_or(&[])
    }

    fn find_next_rope(
        &self,
        buffer: &TextBuffer,
        from_char: usize,
        limit_start_char: usize,
    ) -> Option<Match> {
        let from_byte = buffer.char_to_byte(from_char);
        let limit = buffer.char_to_byte(limit_start_char);
        let hit = rope_scan::find_forward(
            buffer.rope(),
            self.rope_needle(),
            &self.bytes_regex,
            self.case_sensitive_literal(),
            from_byte,
            limit,
        )?;
        Some(self.byte_match(buffer, hit.0, hit.1))
    }

    fn find_prev_rope(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        let to_byte = buffer.char_to_byte(from_char);
        let hit = rope_scan::find_backward(
            buffer.rope(),
            self.rope_needle(),
            &self.bytes_regex,
            self.case_sensitive_literal(),
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
        self.find_next_rope(buffer, from_char, usize::MAX)
    }

    pub fn find_next_wrap(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        self.find_next_rope(buffer, 0, from_char)
    }

    /// Finds the last match before `from_char`, wrapping around the start.
    pub fn find_prev(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        self.find_prev_no_wrap(buffer, from_char)
            .or_else(|| self.find_prev_wrap(buffer, from_char))
    }

    /// Backward search from `from_char` without wrapping to the end of the file.
    pub fn find_prev_no_wrap(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        self.find_prev_rope(buffer, from_char)
    }

    pub fn find_prev_wrap(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        let from_byte = buffer.char_to_byte(from_char);
        let hit = rope_scan::find_last_from(
            buffer.rope(),
            self.rope_needle(),
            &self.bytes_regex,
            self.case_sensitive_literal(),
            from_byte,
        )?;
        Some(self.byte_match(buffer, hit.0, hit.1))
    }

    /// Counts all matches in the buffer.
    pub fn count(&self, buffer: &TextBuffer) -> usize {
        rope_scan::count_all(
            buffer.rope(),
            self.rope_needle(),
            &self.bytes_regex,
            self.case_sensitive_literal(),
        )
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

    /// Returns all matches in document order (rope chunk scan; same semantics as
    /// [`Searcher::count`]).
    pub fn all_matches(&self, buffer: &TextBuffer) -> Vec<Match> {
        self.collect_byte_matches(buffer)
            .into_iter()
            .map(|(start_byte, end_byte)| self.byte_match(buffer, start_byte, end_byte))
            .collect()
    }

    /// Line-scoped match byte ranges in document order.
    pub(crate) fn collect_byte_matches(&self, buffer: &TextBuffer) -> Vec<(usize, usize)> {
        rope_scan::collect_all(
            buffer.rope(),
            self.rope_needle(),
            &self.bytes_regex,
            self.case_sensitive_literal(),
        )
    }

    /// Builds the post-replace document text from byte-range hits (literal template).
    pub(crate) fn build_literal_replace_text(
        rope: &ropey::Rope,
        hits: &[(usize, usize)],
        replacement: &str,
    ) -> String {
        let total = rope.len_bytes();
        let growth: isize = hits
            .iter()
            .map(|(s, e)| replacement.len() as isize - (*e as isize - *s as isize))
            .sum();
        let mut out = String::with_capacity((total as isize + growth).max(0) as usize);
        let mut pos = 0usize;
        for &(start, end) in hits {
            for chunk in rope.byte_slice(pos..start).chunks() {
                out.push_str(chunk);
            }
            out.push_str(replacement);
            pos = end;
        }
        for chunk in rope.byte_slice(pos..total).chunks() {
            out.push_str(chunk);
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

fn byte_to_char(s: &str, byte_idx: usize) -> usize {
    s[..byte_idx.min(s.len())].chars().count()
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

    #[test]
    fn regex_count_many_lines() {
        let mut text = String::new();
        for _ in 0..20_000 {
            text.push_str("x\n");
        }
        text.push_str("a1\na2\n");
        let buf = TextBuffer::from_str(&text);
        let opts = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let s = Searcher::new(r"a\d", opts).unwrap();
        assert_eq!(s.count(&buf), 2);
        let m = s.find_next_no_wrap(&buf, 0).unwrap();
        assert_eq!(buf.slice_text(m.start..m.end), "a1");
    }

    #[test]
    fn find_prev_wrap_suffix() {
        let buf = TextBuffer::from_str("foo bar foo");
        let s = Searcher::new("foo", SearchOptions::default()).unwrap();
        // Before first "foo": wrap searches tail and finds last "foo"
        let m = s.find_prev(&buf, 0).unwrap();
        assert_eq!((m.start, m.end), (8, 11));
    }
}
