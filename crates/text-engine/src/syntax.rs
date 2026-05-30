//! Tree-sitter syntax highlighting.
//!
//! [`SyntaxState`] owns a parser, the current tree and the highlight query for a
//! language. [`SyntaxState::highlights`] runs the query restricted to a byte
//! range, so the renderer only ever pays for the visible viewport, not the whole
//! file. Highlighting is purely tree-based — there is no LSP involvement.

use std::ops::Range;

use ropey::Rope;
use streaming_iterator::StreamingIterator;
use tree_sitter::{InputEdit, Language, Node, Parser, Point, Query, QueryCursor, TextProvider, Tree};

use crate::buffer::EditSummary;
use crate::syntax_languages::{language_config, SUPPORTED_LANGUAGE_IDS};

/// A coarse highlight category that the UI maps to a theme color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightKind {
    Keyword,
    Function,
    Type,
    String,
    Number,
    Comment,
    Constant,
    Variable,
    Property,
    Operator,
    Punctuation,
    Tag,
    Attribute,
    Label,
    Constructor,
    Other,
}

impl HighlightKind {
    fn from_capture_name(name: &str) -> HighlightKind {
        // Capture names look like "keyword.control", "string.special", etc.
        let base = name.split('.').next().unwrap_or(name);
        match base {
            "keyword" => HighlightKind::Keyword,
            "function" | "method" => HighlightKind::Function,
            "type" | "type_builtin" => HighlightKind::Type,
            "string" | "char" => HighlightKind::String,
            "number" | "float" | "integer" => HighlightKind::Number,
            "comment" => HighlightKind::Comment,
            "constant" | "boolean" => HighlightKind::Constant,
            "variable" => HighlightKind::Variable,
            "property" | "field" => HighlightKind::Property,
            "operator" => HighlightKind::Operator,
            "punctuation" => HighlightKind::Punctuation,
            "tag" => HighlightKind::Tag,
            "attribute" => HighlightKind::Attribute,
            "label" => HighlightKind::Label,
            "constructor" => HighlightKind::Constructor,
            _ => HighlightKind::Other,
        }
    }
}

/// A highlighted span in **byte** offsets into the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub kind: HighlightKind,
}

/// Returns the tree-sitter [`Language`] and highlight query source for a known
/// language id, or `None` for plain text / unsupported languages.
fn lookup_language_config(language_id: &str) -> Option<(Language, &'static str)> {
    language_config(language_id)
}

/// True if syntax highlighting is available for `language_id`.
pub fn is_supported(language_id: &str) -> bool {
    lookup_language_config(language_id).is_some()
}

/// All language ids with tree-sitter syntax highlighting.
pub fn supported_language_ids() -> &'static [&'static str] {
    SUPPORTED_LANGUAGE_IDS
}

/// Holds parser state for one document.
pub struct SyntaxState {
    parser: Parser,
    query: Option<Query>,
    tree: Option<Tree>,
    language_id: String,
    supported: bool,
}

impl SyntaxState {
    /// Creates state for `language_id` (no parse yet).
    pub fn new(language_id: &str) -> Self {
        let mut state = Self {
            parser: Parser::new(),
            query: None,
            tree: None,
            language_id: language_id.to_string(),
            supported: false,
        };
        state.configure(language_id);
        state
    }

    fn configure(&mut self, language_id: &str) {
        self.language_id = language_id.to_string();
        self.tree = None;
        self.query = None;
        self.supported = false;
        if let Some((language, query_src)) = lookup_language_config(language_id) {
            if self.parser.set_language(&language).is_ok() {
                if let Ok(query) = Query::new(&language, query_src) {
                    self.query = Some(query);
                    self.supported = true;
                }
            }
        }
    }

    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    pub fn is_supported(&self) -> bool {
        self.supported
    }

    /// Switches language and clears the tree.
    pub fn set_language(&mut self, language_id: &str) {
        if language_id != self.language_id {
            self.configure(language_id);
        }
    }

    /// Re-parses `text` from scratch. Convenience for in-memory strings/tests;
    /// the editor uses the rope-based [`SyntaxState::reparse`] on the hot path.
    pub fn update(&mut self, text: &str) {
        if !self.supported {
            self.tree = None;
            return;
        }
        self.tree = self.parser.parse(text, None);
    }

    /// Records a buffer edit against the current tree so the next
    /// [`SyntaxState::reparse`] can reuse unchanged subtrees. No-op until a tree
    /// exists.
    pub fn edit(&mut self, summary: &EditSummary) {
        if let Some(tree) = self.tree.as_mut() {
            tree.edit(&input_edit(summary));
        }
    }

    /// (Re)parses directly from `rope`, reusing the previous (edited) tree for
    /// incremental work. No full-document `String` is ever materialized: the
    /// parser pulls bytes chunk-by-chunk straight out of the rope.
    pub fn reparse(&mut self, rope: &Rope) {
        if !self.supported {
            self.tree = None;
            return;
        }
        let len_bytes = rope.len_bytes();
        let mut callback = |byte: usize, _: Point| -> &[u8] {
            if byte >= len_bytes {
                return &[];
            }
            let (chunk, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte);
            &chunk.as_bytes()[byte - chunk_byte_idx..]
        };
        self.tree = self
            .parser
            .parse_with_options(&mut callback, self.tree.as_ref(), None);
    }

    /// Like [`SyntaxState::highlights`] but reads node text from a [`Rope`],
    /// avoiding a full-document copy. This is the renderer's hot path.
    pub fn highlights_rope(&self, rope: &Rope, byte_range: Range<usize>) -> Vec<HighlightSpan> {
        let (Some(query), Some(tree)) = (self.query.as_ref(), self.tree.as_ref()) else {
            return Vec::new();
        };

        let capture_names = query.capture_names();
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(byte_range.clone());

        let mut raw: Vec<HighlightSpan> = Vec::new();
        let mut captures = cursor.captures(query, tree.root_node(), RopeProvider(rope));
        while let Some((m, idx)) = captures.next() {
            let capture = m.captures[*idx];
            let node = capture.node;
            let start = node.start_byte();
            let end = node.end_byte();
            if end <= byte_range.start || start >= byte_range.end {
                continue;
            }
            let name = capture_names
                .get(capture.index as usize)
                .copied()
                .unwrap_or("");
            raw.push(HighlightSpan {
                start,
                end,
                kind: HighlightKind::from_capture_name(name),
            });
        }

        resolve_overlaps(raw)
    }

    /// Returns highlight spans whose nodes intersect `byte_range`.
    ///
    /// On overlap the innermost (smallest) capture wins, which matches how a
    /// reader expects nested constructs to be colored.
    pub fn highlights(&self, text: &str, byte_range: Range<usize>) -> Vec<HighlightSpan> {
        let (Some(query), Some(tree)) = (self.query.as_ref(), self.tree.as_ref()) else {
            return Vec::new();
        };

        let capture_names = query.capture_names();
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(byte_range.clone());

        // Collect raw captures, then resolve overlaps preferring smaller spans.
        let mut raw: Vec<HighlightSpan> = Vec::new();
        let mut captures = cursor.captures(query, tree.root_node(), text.as_bytes());
        while let Some((m, idx)) = captures.next() {
            let capture = m.captures[*idx];
            let node = capture.node;
            let start = node.start_byte();
            let end = node.end_byte();
            if end <= byte_range.start || start >= byte_range.end {
                continue;
            }
            let name = capture_names
                .get(capture.index as usize)
                .copied()
                .unwrap_or("");
            raw.push(HighlightSpan {
                start,
                end,
                kind: HighlightKind::from_capture_name(name),
            });
        }

        resolve_overlaps(raw)
    }
}

/// Converts an engine [`EditSummary`] into a tree-sitter [`InputEdit`].
fn input_edit(summary: &EditSummary) -> InputEdit {
    let pt = |p: crate::buffer::BytePoint| Point::new(p.row, p.column);
    InputEdit {
        start_byte: summary.start_byte,
        old_end_byte: summary.old_end_byte,
        new_end_byte: summary.new_end_byte,
        start_position: pt(summary.start_point),
        old_end_position: pt(summary.old_end_point),
        new_end_position: pt(summary.new_end_point),
    }
}

/// A [`TextProvider`] that feeds tree-sitter node text straight from a [`Rope`]
/// in chunks, with zero whole-document allocation.
struct RopeProvider<'a>(&'a Rope);

impl<'a> TextProvider<&'a [u8]> for RopeProvider<'a> {
    type I = ChunksBytes<'a>;

    fn text(&mut self, node: Node) -> Self::I {
        let slice = self.0.byte_slice(node.byte_range());
        ChunksBytes {
            chunks: slice.chunks(),
        }
    }
}

/// Iterator adapter turning rope `&str` chunks into `&[u8]` chunks.
struct ChunksBytes<'a> {
    chunks: ropey::iter::Chunks<'a>,
}

impl<'a> Iterator for ChunksBytes<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        self.chunks.next().map(str::as_bytes)
    }
}

/// Produces a non-overlapping, sorted span list. Smaller (more specific) spans
/// override larger ones they sit inside.
fn resolve_overlaps(mut spans: Vec<HighlightSpan>) -> Vec<HighlightSpan> {
    // Sort by start, then by smaller length last so it wins when we paint.
    spans.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));

    // Sweep, splitting on overlaps so the innermost span shows.
    let mut result: Vec<HighlightSpan> = Vec::with_capacity(spans.len());
    for span in spans {
        if span.end <= span.start {
            continue;
        }
        if let Some(last) = result.last().copied() {
            if span.start < last.end {
                let li = result.len() - 1;
                if span.end <= last.end {
                    // New span sits inside the previous: trim previous, insert
                    // the inner span, then re-add the tail of the outer span.
                    result[li].end = span.start;
                    result.push(span);
                    if last.end > span.end {
                        result.push(HighlightSpan {
                            start: span.end,
                            end: last.end,
                            kind: last.kind,
                        });
                    }
                } else {
                    // Partial overlap: clip the new span to start at last.end.
                    result.push(HighlightSpan {
                        start: last.end,
                        end: span.end,
                        kind: span.kind,
                    });
                }
                continue;
            }
        }
        result.push(span);
    }
    result.retain(|s| s.end > s.start);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_language_count() {
        assert!(supported_language_ids().len() >= 34);
        assert!(is_supported("rust"));
        assert!(is_supported("typescript"));
        assert!(is_supported("tsx"));
        assert!(is_supported("markdown"));
        assert!(!is_supported("text"));
    }

    #[test]
    fn unsupported_language_yields_nothing() {
        let mut s = SyntaxState::new("text");
        assert!(!s.is_supported());
        s.update("hello world");
        assert!(s.highlights("hello world", 0..11).is_empty());
    }

    #[test]
    fn rust_keywords_are_highlighted() {
        let src = "fn main() { let x = 1; }";
        let mut s = SyntaxState::new("rust");
        assert!(s.is_supported());
        s.update(src);
        let spans = s.highlights(src, 0..src.len());
        assert!(!spans.is_empty());
        // "fn" should be a keyword span at offset 0.
        assert!(spans
            .iter()
            .any(|sp| sp.start == 0 && sp.kind == HighlightKind::Keyword));
    }

    #[test]
    fn json_strings_are_highlighted() {
        let src = r#"{"key": "value"}"#;
        let mut s = SyntaxState::new("json");
        s.update(src);
        let spans = s.highlights(src, 0..src.len());
        assert!(spans.iter().any(|sp| sp.kind == HighlightKind::String));
    }

    #[test]
    fn spans_are_sorted_and_non_overlapping() {
        let src = "fn main() { let x = 1; }";
        let mut s = SyntaxState::new("rust");
        s.update(src);
        let spans = s.highlights(src, 0..src.len());
        for pair in spans.windows(2) {
            assert!(pair[0].end <= pair[1].start, "spans overlap: {pair:?}");
        }
    }
}
