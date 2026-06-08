//! Markdown preview rendering engine.
//!
//! Parses markdown via pulldown-cmark and renders it as gpui-component elements.

use std::ops::Range;

pub mod render;

pub use pulldown_cmark;

/// A parsed markdown document with source mappings.
#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub source: String,
    pub blocks: Vec<MarkdownBlock>,
}

/// A block-level markdown element.
#[derive(Debug, Clone)]
pub enum MarkdownBlock {
    Heading(u8, Vec<MarkdownInline>, Range<usize>),
    Paragraph(Vec<MarkdownInline>, Range<usize>),
    CodeBlock(Option<String>, String, Range<usize>),
    BlockQuote(Vec<MarkdownBlock>, Range<usize>),
    Rule(Range<usize>),
}

/// An inline markdown element.
#[derive(Debug, Clone)]
pub enum MarkdownInline {
    Text(String),
    Code(String),
    Link(String, String), // url, text
    Image(String, String), // url, alt
}

impl MarkdownDocument {
    pub fn parse(source: impl Into<String>) -> Self {
        let source = source.into();
        let blocks = parse_blocks(&source);
        Self { source, blocks }
    }
}

fn parse_blocks(source: &str) -> Vec<MarkdownBlock> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

    let parser = Parser::new(source);
    let events: Vec<(Event, Range<usize>)> = parser.into_offset_iter().collect();

    let mut blocks = Vec::new();
    let mut i = 0;

    while i < events.len() {
        match &events[i].0 {
            Event::Start(Tag::Heading { level, .. }) => {
                let level_u8 = *level as u8;
                let start = events[i].1.start;
                let (inlines, end) = parse_inlines(&events, i + 1, TagEnd::Heading(*level));
                i = end + 1;
                blocks.push(MarkdownBlock::Heading(level_u8, inlines, start..events[end].1.end));
            }
            Event::Start(Tag::Paragraph) => {
                let start = events[i].1.start;
                let (inlines, end) = parse_inlines(&events, i + 1, TagEnd::Paragraph);
                i = end + 1;
                blocks.push(MarkdownBlock::Paragraph(inlines, start..events[end].1.end));
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let start = events[i].1.start;
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.to_string()),
                    _ => None,
                };
                let mut code = String::new();
                let mut end = i + 1;
                while end < events.len() {
                    match &events[end].0 {
                        Event::Text(t) => code.push_str(t),
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => {}
                    }
                    end += 1;
                }
                let block_end = events[end].1.end;
                i = end + 1;
                blocks.push(MarkdownBlock::CodeBlock(lang, code, start..block_end));
            }
            Event::Start(Tag::BlockQuote(_)) => {
                let start = events[i].1.start;
                let mut end = i + 1;
                while end < events.len() {
                    if let Event::End(TagEnd::BlockQuote(_)) = &events[end].0 {
                        break;
                    }
                    end += 1;
                }
                let block_end = events[end].1.end;
                // Extract inner text for V1
                let mut text = String::new();
                for j in (i + 1)..end {
                    if let Event::Text(t) = &events[j].0 {
                        text.push_str(t);
                    }
                }
                i = end + 1;
                blocks.push(MarkdownBlock::BlockQuote(
                    vec![MarkdownBlock::Paragraph(vec![MarkdownInline::Text(text)], start..block_end)],
                    start..block_end,
                ));
            }
            Event::Rule => {
                let range = events[i].1.clone();
                i += 1;
                blocks.push(MarkdownBlock::Rule(range));
            }
            Event::Start(Tag::List(_)) | Event::End(TagEnd::List(_)) => {
                // Skip lists for V1 simplified parser
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    blocks
}

fn parse_inlines(
    events: &[(pulldown_cmark::Event, Range<usize>)],
    start: usize,
    end_tag: pulldown_cmark::TagEnd,
) -> (Vec<MarkdownInline>, usize) {
    use pulldown_cmark::{Event, Tag};

    let mut inlines = Vec::new();
    let mut current_text = String::new();
    let mut i = start;

    while i < events.len() {
        match &events[i].0 {
            Event::End(tag) if *tag == end_tag => {
                if !current_text.is_empty() {
                    inlines.push(MarkdownInline::Text(std::mem::take(&mut current_text)));
                }
                return (inlines, i);
            }
            Event::Text(t) => current_text.push_str(t),
            Event::Code(c) => {
                if !current_text.is_empty() {
                    inlines.push(MarkdownInline::Text(std::mem::take(&mut current_text)));
                }
                inlines.push(MarkdownInline::Code(c.to_string()));
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                if !current_text.is_empty() {
                    inlines.push(MarkdownInline::Text(std::mem::take(&mut current_text)));
                }
                let url = dest_url.to_string();
                let mut link_text = String::new();
                i += 1;
                while i < events.len() {
                    match &events[i].0 {
                        Event::End(pulldown_cmark::TagEnd::Link) => break,
                        Event::Text(t) => link_text.push_str(t),
                        _ => {}
                    }
                    i += 1;
                }
                inlines.push(MarkdownInline::Link(url, link_text));
                continue;
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                if !current_text.is_empty() {
                    inlines.push(MarkdownInline::Text(std::mem::take(&mut current_text)));
                }
                let url = dest_url.to_string();
                let mut alt = String::new();
                i += 1;
                while i < events.len() {
                    match &events[i].0 {
                        Event::End(pulldown_cmark::TagEnd::Image) => break,
                        Event::Text(t) => alt.push_str(t),
                        _ => {}
                    }
                    i += 1;
                }
                inlines.push(MarkdownInline::Image(url, alt));
                continue;
            }
            Event::SoftBreak | Event::HardBreak => current_text.push(' '),
            _ => {}
        }
        i += 1;
    }

    if !current_text.is_empty() {
        inlines.push(MarkdownInline::Text(current_text));
    }
    (inlines, events.len().saturating_sub(1))
}
