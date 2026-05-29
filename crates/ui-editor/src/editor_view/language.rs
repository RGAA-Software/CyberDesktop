//! File-extension → syntax language id mapping for the engine highlighter.

use std::path::Path;

/// Maps a file extension to a language id understood by the engine's highlighter.
pub fn language_for_path(path: Option<&Path>) -> &'static str {
    let Some(ext) = path
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
    else {
        return "text";
    };
    match ext.as_str() {
        "rs" => "rust",
        "js" | "cjs" | "mjs" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "py" => "python",
        "json" => "json",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" => "cpp",
        "sh" | "bash" => "bash",
        _ => "text",
    }
}
