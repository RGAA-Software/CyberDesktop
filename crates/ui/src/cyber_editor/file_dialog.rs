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

fn run_file_dialog<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .name("cybereditor-file-dialog".into())
        .spawn(f)
        .expect("failed to spawn file dialog thread")
        .join()
        .expect("file dialog thread panicked")
}

/// Open-file picker. Blocks the calling thread (spawns a short-lived dialog thread).
///
/// Call from [`gpui::AsyncApp::background_spawn`] or similar — never from the UI thread.
pub fn pick_open_file_path(start_dir: Option<&Path>) -> Option<PathBuf> {
    let start_dir = start_dir.map(|p| p.to_path_buf());
    run_file_dialog(move || {
        let mut dialog = rfd::FileDialog::new().set_title("Open File");
        if let Some(dir) = start_dir.as_deref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        for (name, extensions) in OPEN_FILTERS {
            dialog = dialog.add_filter(*name, extensions);
        }
        dialog.pick_file()
    })
}

/// Save-as picker. Blocks the calling thread (spawns a short-lived dialog thread).
///
/// Call from [`gpui::AsyncApp::background_spawn`] or similar — never from the UI thread.
pub fn pick_save_file_path(default_path: &Path) -> Option<PathBuf> {
    let default_path = default_path.to_path_buf();
    run_file_dialog(move || {
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
    })
}
