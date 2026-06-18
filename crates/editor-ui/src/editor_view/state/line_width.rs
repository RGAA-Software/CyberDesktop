//! Per-line prefix width cache for long-row horizontal layout.

use std::collections::HashMap;

use editor_text_engine::{EditSummary, TextBuffer};
use gpui::{px, Font, Hsla, Pixels, SharedString, TextRun, Window};

use crate::editor_view::text_util::{expand_tabs, EDITOR_TAB_SIZE};

const CHECKPOINT_STEP: usize = 256;

/// Lines longer than this (in chars) use horizontal viewport slicing.
pub(crate) const LONG_LINE_COL_THRESHOLD: usize = 512;

pub(crate) struct LineWidthCache {
    line_width: HashMap<usize, (u64, f32)>,
    checkpoints: HashMap<usize, (u64, Vec<(usize, f32)>)>,
}

impl LineWidthCache {
    pub fn new() -> Self {
        Self {
            line_width: HashMap::new(),
            checkpoints: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.line_width.clear();
        self.checkpoints.clear();
    }

    pub fn invalidate_line(&mut self, line: usize) {
        self.line_width.remove(&line);
        self.checkpoints.remove(&line);
    }

    pub fn invalidate_from_edits(&mut self, edits: &[EditSummary]) {
        for edit in edits {
            let end_row = edit
                .old_end_point
                .row
                .max(edit.new_end_point.row)
                .max(edit.start_point.row);
            for line in edit.start_point.row..=end_row {
                self.invalidate_line(line);
            }
        }
    }

    pub fn line_width_px(
        &mut self,
        line: usize,
        line_len: usize,
        revision: u64,
        char_width: Pixels,
        is_long: bool,
        window: &mut Window,
        font: &Font,
        font_size: Pixels,
        buf: &TextBuffer,
        default_color: Hsla,
    ) -> Pixels {
        if let Some((rev, w)) = self.line_width.get(&line) {
            if *rev == revision {
                return px(*w);
            }
        }
        let w = if is_long {
            f32::from(self.col_x_px(
                line,
                line_len,
                revision,
                window,
                font,
                font_size,
                buf,
                default_color,
                char_width,
            ))
        } else {
            let text = buf.line_text(line);
            let expanded = expand_tabs(&text, EDITOR_TAB_SIZE);
            f32::from(shape_plain_width(
                window,
                font,
                font_size,
                &expanded.text,
                default_color,
            ))
        };
        self.line_width.insert(line, (revision, w));
        px(w)
    }

    pub fn col_x_px(
        &mut self,
        line: usize,
        col: usize,
        revision: u64,
        window: &mut Window,
        font: &Font,
        font_size: Pixels,
        buf: &TextBuffer,
        default_color: Hsla,
        char_width: Pixels,
    ) -> Pixels {
        let line_len = buf.line_len_chars(line);
        let col = col.min(line_len);
        if line_len <= LONG_LINE_COL_THRESHOLD {
            let text = buf.line_text(line);
            let expanded = expand_tabs(&text, EDITOR_TAB_SIZE);
            let shaped = shape_plain(window, font, font_size, &expanded.text, default_color);
            return shaped.x_for_index(expanded.original_char_to_expanded_byte(col));
        }

        let target_checkpoint = (col / CHECKPOINT_STEP) * CHECKPOINT_STEP;
        let base_col = self.ensure_checkpoint(
            line,
            target_checkpoint,
            revision,
            window,
            font,
            font_size,
            buf,
            default_color,
            char_width,
        );
        let base_x = self.checkpoint_x(line, base_col, revision).unwrap_or(0.0);
        if col <= base_col {
            return px(base_x);
        }
        let segment = buf.line_chars_slice(line, base_col, col);
        let segment_w = if segment.is_empty() {
            0.0
        } else {
            let expanded = expand_tabs(&segment, EDITOR_TAB_SIZE);
            f32::from(shape_plain_width(
                window,
                font,
                font_size,
                &expanded.text,
                default_color,
            ))
        };
        let x = base_x + segment_w;
        self.store_checkpoint(line, revision, col, x);
        px(x)
    }

    fn ensure_checkpoint(
        &mut self,
        line: usize,
        target_col: usize,
        revision: u64,
        window: &mut Window,
        font: &Font,
        font_size: Pixels,
        buf: &TextBuffer,
        default_color: Hsla,
        _char_width: Pixels,
    ) -> usize {
        if target_col == 0 {
            self.store_checkpoint(line, revision, 0, 0.0);
            return 0;
        }
        if self.checkpoint_x(line, target_col, revision).is_some() {
            return target_col;
        }
        let mut col = self
            .checkpoints
            .get(&line)
            .filter(|(rev, _)| *rev == revision)
            .map(|(_, cps)| cps.last().map(|(c, _)| *c).unwrap_or(0))
            .unwrap_or(0);
        while col < target_col {
            let next = (col + CHECKPOINT_STEP).min(target_col);
            let base_x = self.checkpoint_x(line, col, revision).unwrap_or(0.0);
            let segment = buf.line_chars_slice(line, col, next);
            let segment_w = if segment.is_empty() {
                0.0
            } else {
                let expanded = expand_tabs(&segment, EDITOR_TAB_SIZE);
                f32::from(shape_plain_width(
                    window,
                    font,
                    font_size,
                    &expanded.text,
                    default_color,
                ))
            };
            let x = base_x + segment_w;
            self.store_checkpoint(line, revision, next, x);
            col = next;
        }
        col
    }

    fn checkpoint_x(&self, line: usize, col: usize, revision: u64) -> Option<f32> {
        let (rev, cps) = self.checkpoints.get(&line)?;
        if *rev != revision {
            return None;
        }
        cps.iter().find(|(c, _)| *c == col).map(|(_, x)| *x)
    }

    fn store_checkpoint(&mut self, line: usize, revision: u64, col: usize, x: f32) {
        let entry = self
            .checkpoints
            .entry(line)
            .or_insert((revision, Vec::new()));
        if entry.0 != revision {
            entry.0 = revision;
            entry.1.clear();
        }
        if let Some(existing) = entry.1.iter_mut().find(|(c, _)| *c == col) {
            existing.1 = x;
        } else {
            entry.1.push((col, x));
            entry.1.sort_by_key(|(c, _)| *c);
        }
    }
}

fn shape_plain_width(
    window: &mut Window,
    font: &Font,
    font_size: Pixels,
    text: &str,
    default_color: Hsla,
) -> Pixels {
    shape_plain(window, font, font_size, text, default_color).width
}

fn shape_plain(
    window: &mut Window,
    font: &Font,
    font_size: Pixels,
    text: &str,
    default_color: Hsla,
) -> gpui::ShapedLine {
    let run = TextRun {
        len: text.len(),
        font: font.clone(),
        color: default_color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window.text_system().shape_line(
        SharedString::from(text.to_string()),
        font_size,
        &[run],
        None,
    )
}
