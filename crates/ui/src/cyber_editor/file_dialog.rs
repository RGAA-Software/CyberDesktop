//! Native open/save file dialogs for CyberEditor (Windows file picker).

use std::path::{Path, PathBuf};

const OPEN_FILTERS: &[(&str, &[&str])] = &[
    (
        "Text & code",
        &[
            "txt", "log", "md", "json", "toml", "yaml", "yml", "xml", "sql", "rs", "py", "js",
            "ts", "tsx", "jsx", "c", "cpp", "h", "hpp", "go", "java", "kt", "rb", "php", "sh",
            "css", "html", "htm",
        ],
    ),
    ("All files", &["*"]),
];

/// Blocking open-file picker. Call from a background thread or `background_spawn`.
pub fn pick_open_file_path(start_dir: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title("Open File");
    if let Some(dir) = start_dir.and_then(|p| p.parent()) {
        dialog = dialog.set_directory(dir);
    }
    for (name, extensions) in OPEN_FILTERS {
        dialog = dialog.add_filter(*name, extensions);
    }
    dialog.pick_file()
}

/// Blocking save-as picker. Call from a background thread or `background_spawn`.
pub fn pick_save_file_path(default_path: &Path) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title("Save As");
    if let Some(parent) = default_path.parent() {
        if parent.as_os_str().len() > 0 {
            dialog = dialog.set_directory(parent);
        }
    }
    if let Some(name) = default_path.file_name() {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    for (name, extensions) in OPEN_FILTERS {
        dialog = dialog.add_filter(*name, extensions);
    }
    dialog.save_file()
}
