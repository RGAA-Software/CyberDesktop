//! Named UI icon paths and extension → SVG lookup (Tabler + Zed file_icons).

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::file_type_icons;
use crate::tabler_icons;

fn named_icon_paths() -> &'static HashMap<&'static str, &'static str> {
    static NAMED_ICON_PATHS: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    NAMED_ICON_PATHS.get_or_init(|| {
        HashMap::from([
            ("folder", tabler_icons::FOLDER),
            ("new_folder", tabler_icons::FOLDER_PLUS),
            ("new_file", tabler_icons::FILE_PLUS),
            ("home", tabler_icons::HOME),
        ])
    })
}

/// App-bundled SVG path for a named UI icon.
pub fn named_icon_path(name: &str) -> Option<&'static str> {
    named_icon_paths().get(name).copied()
}

/// App-bundled SVG path for a file extension (e.g. `"pdf"`).
pub fn extension_svg_path(ext: &str) -> Option<&'static str> {
    let path = file_type_icons::svg_path_for_extension(ext);
    if path == file_type_icons::FALLBACK_FILE_ICON {
        None
    } else {
        Some(path)
    }
}
