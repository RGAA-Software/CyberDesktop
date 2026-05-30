//! Tree-sitter grammar + highlight query registry (aligned with gpui-component).

use tree_sitter::Language;

/// Returns the tree-sitter [`Language`] and highlight query for `language_id`.
pub(crate) fn language_config(language_id: &str) -> Option<(Language, &'static str)> {
    let (lang, query): (Language, &'static str) = match language_id {
        "json" | "jsonc" => (
            tree_sitter_json::LANGUAGE.into(),
            tree_sitter_json::HIGHLIGHTS_QUERY,
        ),
        "rust" => (
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::HIGHLIGHTS_QUERY,
        ),
        "python" => (
            tree_sitter_python::LANGUAGE.into(),
            tree_sitter_python::HIGHLIGHTS_QUERY,
        ),
        "javascript" => (
            tree_sitter_javascript::LANGUAGE.into(),
            tree_sitter_javascript::HIGHLIGHT_QUERY,
        ),
        "typescript" => (
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
        ),
        "tsx" => (
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
        ),
        "c" => (
            tree_sitter_c::LANGUAGE.into(),
            tree_sitter_c::HIGHLIGHT_QUERY,
        ),
        "cpp" => (
            tree_sitter_cpp::LANGUAGE.into(),
            tree_sitter_cpp::HIGHLIGHT_QUERY,
        ),
        "bash" | "shell" | "sh" => (
            tree_sitter_bash::LANGUAGE.into(),
            tree_sitter_bash::HIGHLIGHT_QUERY,
        ),
        "html" => (
            tree_sitter_html::LANGUAGE.into(),
            tree_sitter_html::HIGHLIGHTS_QUERY,
        ),
        "css" | "scss" => (
            tree_sitter_css::LANGUAGE.into(),
            tree_sitter_css::HIGHLIGHTS_QUERY,
        ),
        "toml" => (
            tree_sitter_toml_ng::LANGUAGE.into(),
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
        ),
        "yaml" => (
            tree_sitter_yaml::LANGUAGE.into(),
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
        ),
        "markdown" | "mdx" => (
            tree_sitter_md::LANGUAGE.into(),
            tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
        ),
        "sql" => (
            tree_sitter_sequel::LANGUAGE.into(),
            tree_sitter_sequel::HIGHLIGHTS_QUERY,
        ),
        "go" => (
            tree_sitter_go::LANGUAGE.into(),
            tree_sitter_go::HIGHLIGHTS_QUERY,
        ),
        "java" => (
            tree_sitter_java::LANGUAGE.into(),
            tree_sitter_java::HIGHLIGHTS_QUERY,
        ),
        "kotlin" => (
            tree_sitter_kotlin_sg::LANGUAGE.into(),
            tree_sitter_kotlin_sg::HIGHLIGHTS_QUERY,
        ),
        "swift" => (
            tree_sitter_swift::LANGUAGE.into(),
            tree_sitter_swift::HIGHLIGHTS_QUERY,
        ),
        "ruby" => (
            tree_sitter_ruby::LANGUAGE.into(),
            tree_sitter_ruby::HIGHLIGHTS_QUERY,
        ),
        "php" => (
            tree_sitter_php::LANGUAGE_PHP.into(),
            tree_sitter_php::HIGHLIGHTS_QUERY,
        ),
        "csharp" => (
            tree_sitter_c_sharp::LANGUAGE.into(),
            tree_sitter_c_sharp::HIGHLIGHTS_QUERY,
        ),
        "lua" => (
            tree_sitter_lua::LANGUAGE.into(),
            tree_sitter_lua::HIGHLIGHTS_QUERY,
        ),
        "zig" => (
            tree_sitter_zig::LANGUAGE.into(),
            tree_sitter_zig::HIGHLIGHTS_QUERY,
        ),
        "scala" => (
            tree_sitter_scala::LANGUAGE.into(),
            tree_sitter_scala::HIGHLIGHTS_QUERY,
        ),
        "make" | "makefile" => (
            tree_sitter_make::LANGUAGE.into(),
            tree_sitter_make::HIGHLIGHTS_QUERY,
        ),
        "diff" => (
            tree_sitter_diff::LANGUAGE.into(),
            tree_sitter_diff::HIGHLIGHTS_QUERY,
        ),
        "elixir" => (
            tree_sitter_elixir::LANGUAGE.into(),
            tree_sitter_elixir::HIGHLIGHTS_QUERY,
        ),
        "proto" | "protobuf" => (
            tree_sitter_proto::LANGUAGE.into(),
            include_str!("../queries/proto/highlights.scm"),
        ),
        "cmake" => (
            tree_sitter_cmake::LANGUAGE.into(),
            include_str!("../queries/cmake/highlights.scm"),
        ),
        "jsdoc" => (
            tree_sitter_jsdoc::LANGUAGE.into(),
            tree_sitter_jsdoc::HIGHLIGHTS_QUERY,
        ),
        "astro" => (
            tree_sitter_astro_next::LANGUAGE.into(),
            tree_sitter_astro_next::HIGHLIGHTS_QUERY,
        ),
        "svelte" => (
            tree_sitter_svelte_next::LANGUAGE.into(),
            tree_sitter_svelte_next::HIGHLIGHTS_QUERY,
        ),
        "erb" | "ejs" => (
            tree_sitter_embedded_template::LANGUAGE.into(),
            tree_sitter_embedded_template::HIGHLIGHTS_QUERY,
        ),
        _ => return None,
    };
    if query.is_empty() {
        return None;
    }
    Some((lang, query))
}

/// Canonical language ids with tree-sitter syntax highlighting.
pub const SUPPORTED_LANGUAGE_IDS: &[&str] = &[
    "astro",
    "bash",
    "c",
    "cmake",
    "cpp",
    "csharp",
    "css",
    "diff",
    "ejs",
    "elixir",
    "erb",
    "go",
    "html",
    "java",
    "javascript",
    "json",
    "jsdoc",
    "kotlin",
    "lua",
    "make",
    "markdown",
    "php",
    "proto",
    "python",
    "ruby",
    "rust",
    "scala",
    "shell",
    "sql",
    "svelte",
    "swift",
    "toml",
    "tsx",
    "typescript",
    "yaml",
    "zig",
];
