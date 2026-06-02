use std::path::PathBuf;

use gpui::SharedString;

use super::{detect_indent_style, detect_line_ending, language_for_path, IndentStyle, LineEndingKind};

pub(crate) struct EditorSession {
    file_path: Option<PathBuf>,
    language: SharedString,
    saved_text: String,
    find_query: String,
    replace_query: String,
    dirty: bool,
    line_numbers: bool,
    soft_wrap: bool,
    encoding_label: SharedString,
    line_ending: LineEndingKind,
    indent_style: IndentStyle,
}

impl EditorSession {
    pub(crate) fn new(file_path: Option<PathBuf>, initial_text: String) -> Self {
        let language = SharedString::from(language_for_path(file_path.as_deref()));
        let line_ending = detect_line_ending(&initial_text);
        let indent_style = detect_indent_style(&initial_text);
        Self {
            file_path,
            language,
            saved_text: initial_text,
            find_query: String::new(),
            replace_query: String::new(),
            dirty: false,
            line_numbers: true,
            soft_wrap: false,
            encoding_label: SharedString::from("UTF-8"),
            line_ending,
            indent_style,
        }
    }

    pub(crate) fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    pub(crate) fn language(&self) -> &SharedString {
        &self.language
    }

    pub(crate) fn dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn find_query(&self) -> &str {
        &self.find_query
    }

    pub(crate) fn replace_query(&self) -> &str {
        &self.replace_query
    }

    pub(crate) fn line_numbers(&self) -> bool {
        self.line_numbers
    }

    pub(crate) fn soft_wrap(&self) -> bool {
        self.soft_wrap
    }

    pub(crate) fn encoding_label(&self) -> &SharedString {
        &self.encoding_label
    }

    pub(crate) fn line_ending_label(&self) -> &'static str {
        self.line_ending.label()
    }

    pub(crate) fn indent_label(&self) -> String {
        self.indent_style.label()
    }

    pub(crate) fn preferred_indent_unit(&self) -> String {
        match &self.indent_style {
            IndentStyle::Spaces(size) => " ".repeat((*size).max(1) as usize),
            IndentStyle::Tabs => "\t".to_string(),
            IndentStyle::Mixed | IndentStyle::Unknown => "    ".to_string(),
        }
    }

    pub(crate) fn update_dirty_from_text(&mut self, current_text: &str) -> bool {
        let dirty = current_text != self.saved_text;
        let changed = dirty != self.dirty;
        self.dirty = dirty;
        self.line_ending = detect_line_ending(current_text);
        self.indent_style = detect_indent_style(current_text);
        changed
    }

    pub(crate) fn apply_save(&mut self, path: PathBuf, text: String) {
        self.file_path = Some(path.clone());
        self.language = SharedString::from(language_for_path(Some(&path)));
        self.line_ending = detect_line_ending(&text);
        self.indent_style = detect_indent_style(&text);
        self.saved_text = text;
        self.dirty = false;
    }

    pub(crate) fn apply_loaded_document(&mut self, path: PathBuf, text: String) {
        self.file_path = Some(path.clone());
        self.language = SharedString::from(language_for_path(Some(&path)));
        self.line_ending = detect_line_ending(&text);
        self.indent_style = detect_indent_style(&text);
        self.saved_text = text;
        self.dirty = false;
    }

    pub(crate) fn suggested_save_path(&self) -> PathBuf {
        if let Some(path) = self.file_path.clone() {
            return path;
        }

        let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        path.push("untitled.txt");
        path
    }

    pub(crate) fn toggle_line_numbers(&mut self) -> bool {
        self.line_numbers = !self.line_numbers;
        self.line_numbers
    }

    pub(crate) fn toggle_soft_wrap(&mut self) -> bool {
        self.soft_wrap = !self.soft_wrap;
        self.soft_wrap
    }

    pub(crate) fn set_find_query(&mut self, query: String) {
        self.find_query = query;
    }

    pub(crate) fn set_replace_query(&mut self, query: String) {
        self.replace_query = query;
    }
}
