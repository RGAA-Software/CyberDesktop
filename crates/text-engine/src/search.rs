//! In-buffer search and replace (literal or regex, case / whole-word options).
//!
//! Search is line-scoped: each rope line is searched independently. This keeps
//! memory bounded (we only materialize one short line at a time, never the whole
//! file) and matches Notepad++'s default behaviour where patterns do not span
//! line breaks. `find_next` / `find_prev` scan lazily and stop at the first hit,
//! so they stay fast even on huge files.

use anyhow::Result;
use regex::Regex;

use crate::buffer::TextBuffer;

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
        Ok(Self { regex })
    }

    /// Finds the first match at or after `from_char`, wrapping around the end.
    pub fn find_next(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        let rope = buffer.rope();
        let total = rope.len_chars();
        let from = from_char.min(total);
        let line_count = rope.len_lines();
        let start_line = rope.char_to_line(from);

        // Forward pass: start_line..end.
        for line in start_line..line_count {
            let line_start = rope.line_to_char(line);
            let s = line_string(buffer, line);
            let from_byte = if line == start_line {
                char_to_byte(&s, from - line_start)
            } else {
                0
            };
            if from_byte <= s.len() {
                if let Some(m) = self.regex.find_at(&s, from_byte) {
                    return Some(to_match(&s, line_start, m.start(), m.end()));
                }
            }
        }

        // Wrap pass: 0..=start_line, requiring matches before the cursor.
        for line in 0..=start_line.min(line_count.saturating_sub(1)) {
            let line_start = rope.line_to_char(line);
            let s = line_string(buffer, line);
            let limit_byte = if line == start_line {
                char_to_byte(&s, from - line_start)
            } else {
                s.len()
            };
            if let Some(m) = self.regex.find(&s) {
                if line < start_line || m.start() < limit_byte {
                    return Some(to_match(&s, line_start, m.start(), m.end()));
                }
            }
        }
        None
    }

    /// Finds the last match before `from_char`, wrapping around the start.
    pub fn find_prev(&self, buffer: &TextBuffer, from_char: usize) -> Option<Match> {
        let rope = buffer.rope();
        let total = rope.len_chars();
        let from = from_char.min(total);
        let line_count = rope.len_lines();
        let start_line = rope.char_to_line(from);

        // Backward pass: start_line down to 0, last match before cursor.
        for line in (0..=start_line.min(line_count.saturating_sub(1))).rev() {
            let line_start = rope.line_to_char(line);
            let s = line_string(buffer, line);
            let limit_byte = if line == start_line {
                char_to_byte(&s, from - line_start)
            } else {
                s.len()
            };
            if let Some(m) = self
                .regex
                .find_iter(&s)
                .take_while(|m| m.start() < limit_byte)
                .last()
            {
                return Some(to_match(&s, line_start, m.start(), m.end()));
            }
        }

        // Wrap: from end down to start_line.
        for line in (start_line..line_count).rev() {
            let line_start = rope.line_to_char(line);
            let s = line_string(buffer, line);
            if line == start_line {
                let limit_byte = char_to_byte(&s, from - line_start);
                if let Some(m) = self.regex.find_iter(&s).find(|m| m.start() >= limit_byte) {
                    // first at/after cursor, but we want the very last overall on wrap
                    if let Some(last) = self.regex.find_iter(&s).last() {
                        return Some(to_match(&s, line_start, last.start(), last.end()));
                    }
                    return Some(to_match(&s, line_start, m.start(), m.end()));
                }
            } else if let Some(m) = self.regex.find_iter(&s).last() {
                return Some(to_match(&s, line_start, m.start(), m.end()));
            }
        }
        None
    }

    /// Counts all matches in the buffer.
    pub fn count(&self, buffer: &TextBuffer) -> usize {
        let rope = buffer.rope();
        let mut total = 0;
        for line in 0..rope.len_lines() {
            let s = line_string(buffer, line);
            total += self.regex.find_iter(&s).count();
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
            let s = line_string(buffer, line);
            for m in self.regex.find_iter(&s) {
                total += 1;
                let start = line_start + byte_to_char(&s, m.start());
                if index.is_none() && start == head {
                    index = Some(total);
                }
            }
        }
        (total, index)
    }

    /// Returns all matches in document order.
    pub fn all_matches(&self, buffer: &TextBuffer) -> Vec<Match> {
        let rope = buffer.rope();
        let mut out = Vec::new();
        for line in 0..rope.len_lines() {
            let line_start = rope.line_to_char(line);
            let s = line_string(buffer, line);
            for m in self.regex.find_iter(&s) {
                out.push(to_match(&s, line_start, m.start(), m.end()));
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

/// The text of `line` without its trailing CR/LF.
fn line_string(buffer: &TextBuffer, line: usize) -> String {
    buffer.line_text(line)
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
    fn find_prev_basic() {
        let buf = TextBuffer::from_str("foo foo foo");
        let s = Searcher::new("foo", SearchOptions::default()).unwrap();
        let m = s.find_prev(&buf, 11).unwrap();
        assert_eq!((m.start, m.end), (8, 11));
        let m2 = s.find_prev(&buf, m.start).unwrap();
        assert_eq!((m2.start, m2.end), (4, 7));
    }
}
