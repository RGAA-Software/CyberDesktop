use std::path::Path;

pub(crate) fn language_for_path(path: Option<&Path>) -> &'static str {
    let Some(ext) = path
        .and_then(|path| path.extension())
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    else {
        return "text";
    };

    match ext.as_str() {
        "rs" => "rust",
        "js" | "cjs" | "mjs" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "javascript",
        "py" => "python",
        "html" | "htm" => "html",
        "css" => "css",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "sql" => "sql",
        "sh" => "bash",
        "xml" => "xml",
        "c" => "c",
        "cc" | "cpp" | "cxx" | "h" | "hpp" => "cpp",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "rb" => "ruby",
        "php" => "php",
        _ => "text",
    }
}

#[cfg(not(feature = "zed-engine"))]
pub(crate) fn line_comment_prefix(language: &str) -> Option<&'static str> {
    match language {
        "rust" | "javascript" | "typescript" | "tsx" | "c" | "cpp" | "go" | "java"
        | "kotlin" | "swift" | "php" => Some("//"),
        "python" | "bash" | "yaml" | "toml" | "ruby" => Some("#"),
        "sql" => Some("--"),
        _ => None,
    }
}
