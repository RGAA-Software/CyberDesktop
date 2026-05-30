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

/// Returns true when CyberFiles should open `path` in CyberEditor (text or known code).
pub fn is_cybereditor_openable(path: &Path) -> bool {
    if path.is_dir() {
        return false;
    }

    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "makefile" | "gnumakefile" | "cmakelists.txt" | "dockerfile" | "containerfile"
            | ".gitignore" | ".gitattributes" | ".editorconfig" => return true,
            _ if lower.ends_with(".dockerfile") => return true,
            _ => {}
        }
    }

    let Some(ext) = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
    else {
        return true;
    };

    if is_binary_extension(&ext) {
        return false;
    }

    is_text_or_code_extension(&ext)
}

fn is_binary_extension(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "bmp" | "tif" | "tiff" | "psd" | "heic"
            | "avif" | "raw" | "cr2" | "nef"
            | "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "cab" | "iso" | "img"
            | "mp3" | "mp4" | "m4a" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "wav" | "flac"
            | "ogg" | "aac" | "wma" | "webm"
            | "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp"
            | "exe" | "dll" | "sys" | "ocx" | "scr" | "msi" | "com" | "apk" | "dmg" | "pkg"
            | "deb" | "rpm" | "appimage"
            | "woff" | "woff2" | "ttf" | "otf" | "eot"
            | "db" | "sqlite" | "sqlite3" | "mdb" | "accdb"
            | "class" | "jar" | "pyc" | "pyo" | "so" | "dylib" | "obj" | "o" | "a" | "lib"
            | "pdb" | "bin" | "dat" | "vhd" | "vmdk" | "wasm"
    )
}

fn is_text_or_code_extension(ext: &str) -> bool {
    matches!(
        ext,
        "rs" | "js" | "cjs" | "mjs" | "jsx" | "ts" | "tsx" | "py" | "pyw" | "pyi" | "json"
            | "jsonc" | "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" | "sh" | "bash"
            | "zsh" | "fish" | "ps1" | "psm1" | "html" | "htm" | "xhtml" | "css" | "scss"
            | "sass" | "less" | "toml" | "yaml" | "yml" | "md" | "markdown" | "mdx" | "sql"
            | "mysql" | "pgsql" | "psql" | "xml" | "xsl" | "xslt" | "svg" | "go" | "java" | "kt"
            | "kts" | "ktm" | "swift" | "rb" | "rake" | "gemspec" | "php" | "php3" | "php4"
            | "php5" | "phtml" | "cs" | "lua" | "zig" | "scala" | "sc" | "mk" | "diff" | "patch"
            | "ex" | "exs" | "proto" | "cmake" | "graphql" | "gql" | "astro" | "svelte" | "erb"
            | "ejs" | "vue" | "ini" | "cfg" | "conf" | "properties" | "log" | "txt" | "text"
            | "bat" | "cmd" | "r" | "pl" | "pm" | "hs" | "clj" | "cljs" | "cljc" | "dart" | "v"
            | "sv" | "tf" | "hcl"
    )
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

    #[test]
    fn cybereditor_openable_filters_binaries() {
        assert!(is_cybereditor_openable(Path::new("main.rs")));
        assert!(is_cybereditor_openable(Path::new("notes.txt")));
        assert!(is_cybereditor_openable(Path::new("README")));
        assert!(!is_cybereditor_openable(Path::new("photo.png")));
        assert!(!is_cybereditor_openable(Path::new("app.exe")));
    }
}
