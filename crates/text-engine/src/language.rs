//! File path → syntax language id mapping for the engine highlighter.

use std::path::Path;

/// Maps a file path to a language id understood by [`crate::syntax::SyntaxState`].
pub fn language_for_path(path: Option<&Path>) -> &'static str {
    let Some(path) = path else {
        return "text";
    };

    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "makefile" | "gnumakefile" => return "make",
            "cmakelists.txt" => return "cmake",
            "dockerfile" | "containerfile" => return "bash",
            ".gitignore" | ".gitattributes" | ".editorconfig" => return "bash",
            _ => {}
        }
        if lower.ends_with(".dockerfile") {
            return "bash";
        }
    }

    let Some(ext) = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
    else {
        return "text";
    };

    match ext.as_str() {
        "rs" => "rust",
        "js" | "cjs" | "mjs" | "jsx" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "py" | "pyw" | "pyi" => "python",
        "json" | "jsonc" => "json",
        "c" => "c",
        "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "sh" | "bash" | "zsh" | "fish" => "bash",
        "ps1" | "psm1" => "bash",
        "html" | "htm" | "xhtml" => "html",
        "css" | "scss" | "sass" | "less" => "css",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" | "mdx" => "markdown",
        "sql" | "mysql" | "pgsql" | "psql" => "sql",
        "xml" | "xsl" | "xslt" | "svg" => "html",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" | "ktm" => "kotlin",
        "swift" => "swift",
        "rb" | "rake" | "gemspec" => "ruby",
        "php" | "php3" | "php4" | "php5" | "phtml" => "php",
        "cs" => "csharp",
        "lua" => "lua",
        "zig" => "zig",
        "scala" | "sc" => "scala",
        "mk" => "make",
        "diff" | "patch" => "diff",
        "ex" | "exs" => "elixir",
        "proto" => "proto",
        "cmake" => "cmake",
        "graphql" | "gql" => "text",
        "astro" => "astro",
        "svelte" => "svelte",
        "erb" => "erb",
        "ejs" => "ejs",
        "vue" => "html",
        "ini" | "cfg" | "conf" | "properties" => "toml",
        "log" | "txt" => "text",
        "bat" | "cmd" => "bash",
        "r" => "text",
        "pl" | "pm" => "text",
        "hs" => "text",
        "clj" | "cljs" | "cljc" => "text",
        "dart" => "text",
        "v" | "sv" => "text",
        "tf" | "hcl" => "text",
        _ => "text",
    }
}

/// Line comment prefix for toggle-comment, if supported for the language id.
pub fn line_comment_prefix(language: &str) -> Option<&'static str> {
    match language {
        "rust" | "javascript" | "typescript" | "tsx" | "c" | "cpp" | "csharp" | "go" | "java"
        | "kotlin" | "swift" | "scala" | "php" | "zig" | "lua" | "dart" => Some("//"),
        "python" | "bash" | "shell" | "yaml" | "toml" | "ruby" | "elixir" | "make" | "cmake"
        | "dockerfile" => Some("#"),
        "sql" => Some("--"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn maps_common_extensions() {
        assert_eq!(language_for_path(Some(Path::new("main.rs"))), "rust");
        assert_eq!(language_for_path(Some(Path::new("app.tsx"))), "tsx");
        assert_eq!(language_for_path(Some(Path::new("CMakeLists.txt"))), "cmake");
        assert_eq!(language_for_path(Some(Path::new("Makefile"))), "make");
    }
}
