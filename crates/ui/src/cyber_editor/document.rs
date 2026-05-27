use std::path::Path;

use gpui::SharedString;

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
            load_error: Some(format!("{} is a directory, not a file.", path.display())),
        };
    }

    match std::fs::read_to_string(path) {
        Ok(text) => LoadedDocument {
            text,
            load_error: None,
        },
        Err(err) => LoadedDocument {
            text: String::new(),
            load_error: Some(format!(
                "Failed to open {} as UTF-8 text: {err}",
                path.display()
            )),
        },
    }
}

pub(crate) fn display_name(path: Option<&Path>) -> SharedString {
    match path.and_then(|path| path.file_name()).and_then(|name| name.to_str()) {
        Some(name) if !name.is_empty() => SharedString::from(name),
        _ => SharedString::from("Untitled"),
    }
}

pub(crate) fn display_language(language: &SharedString) -> SharedString {
    SharedString::from(format!("Language: {}", language))
}

pub(crate) fn display_path(path: Option<&Path>) -> SharedString {
    match path {
        Some(path) => SharedString::from(path.to_string_lossy().to_string()),
        None => SharedString::from("No file open"),
    }
}
