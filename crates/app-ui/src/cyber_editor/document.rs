use std::path::Path;

use gpui::SharedString;
use rust_i18n::t;

pub(crate) struct LoadedDocument {
    pub(crate) text: String,
    pub(crate) load_error: Option<String>,
}

pub(crate) fn load_document(path: Option<&Path>) -> LoadedDocument {
    let Some(path) = path else {
        return LoadedDocument {
            text: String::new(),
            load_error: None,
        };
    };

    if !path.exists() {
        return LoadedDocument {
            text: String::new(),
            load_error: None,
        };
    }

    if path.is_dir() {
        return LoadedDocument {
            text: String::new(),
            load_error: Some(t!("editor.error.is_directory", path = path.display()).to_string()),
        };
    }

    match std::fs::read_to_string(path) {
        Ok(text) => LoadedDocument {
            text,
            load_error: None,
        },
        Err(err) => LoadedDocument {
            text: String::new(),
            load_error: Some(
                t!(
                    "editor.error.open_failed",
                    path = path.display(),
                    err = err.to_string()
                )
                .to_string(),
            ),
        },
    }
}

pub(crate) fn display_name(path: Option<&Path>) -> SharedString {
    match path.and_then(|path| path.file_name()).and_then(|name| name.to_str()) {
        Some(name) if !name.is_empty() => SharedString::from(name),
        _ => SharedString::from(t!("editor.untitled")),
    }
}

pub(crate) fn display_language(language: &SharedString) -> SharedString {
    SharedString::from(t!("editor.language", name = language.as_ref()))
}

