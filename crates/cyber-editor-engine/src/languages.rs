//! Embedded tree-sitter grammars and language configs for CyberEditor.

use std::{path::Path, sync::Arc};

use futures_lite::future::block_on;
use gpui::{App, AppContext, Global};
use language::{Language, LanguageRegistry, LoadedLanguage, PLAIN_TEXT};

const EMBEDDED_LANGUAGE_FOLDERS: &[&str] = &[
    "bash",
    "c",
    "cpp",
    "css",
    "diff",
    "gitcommit",
    "go",
    "gomod",
    "gowork",
    "javascript",
    "jsdoc",
    "json",
    "jsonc",
    "markdown",
    "markdown-inline",
    "python",
    "regex",
    "rust",
    "tsx",
    "typescript",
    "yaml",
];

struct GlobalLanguageRegistry(Arc<LanguageRegistry>);

impl Global for GlobalLanguageRegistry {}

/// Register native grammars and embedded `config.toml` languages.
pub fn register_embedded_languages(registry: &Arc<LanguageRegistry>) {
    registry.register_native_grammars(
        grammars::native_grammars()
            .into_iter()
            .map(|(name, grammar)| (name, grammar)),
    );

    for &folder in EMBEDDED_LANGUAGE_FOLDERS {
        let config = grammars::load_config_for_feature(folder, true);
        let folder = Arc::<str>::from(folder);
        registry.register_language(
            config.name.clone(),
            config.grammar.clone(),
            config.matcher.clone(),
            config.hidden,
            None,
            Arc::new(move || {
                Ok(LoadedLanguage {
                    config: grammars::load_config_for_feature(&folder, true),
                    queries: grammars::load_queries(&folder),
                    context_provider: None,
                    toolchain_provider: None,
                    manifest_name: None,
                })
            }),
        );
    }
}

/// CyberEditor `language_for_path` id → registry lookup key (extension or name fragment).
pub fn lookup_key_for_language_id(language_id: &str) -> &str {
    match language_id {
        "rust" => "rs",
        "javascript" => "js",
        "typescript" => "ts",
        "tsx" => "tsx",
        "python" => "py",
        "css" => "css",
        "json" => "json",
        "yaml" => "yaml",
        "markdown" => "md",
        "bash" => "sh",
        "c" => "c",
        "cpp" => "cpp",
        "go" => "go",
        "text" => "txt",
        other => other,
    }
}

pub fn language_registry<C: AppContext>(cx: &C) -> Arc<LanguageRegistry> {
    cx.read_global(|registry: &GlobalLanguageRegistry, _| registry.0.clone())
}

pub fn init_language_registry(cx: &mut App) {
    if cx.has_global::<GlobalLanguageRegistry>() {
        return;
    }
    let registry = Arc::new(LanguageRegistry::new(cx.background_executor().clone()));
    register_embedded_languages(&registry);
    cx.set_global(GlobalLanguageRegistry(registry));
}

pub async fn load_language(
    registry: &Arc<LanguageRegistry>,
    language_id: &str,
    path: Option<&Path>,
) -> Option<Arc<Language>> {
    if language_id == "text" {
        return Some(PLAIN_TEXT.clone());
    }

    if let Some(path) = path {
        if let Ok(language) = registry
            .clone()
            .load_language_for_file_path(path)
            .await
        {
            return Some(language);
        }
    }

    let key = lookup_key_for_language_id(language_id);
    registry
        .clone()
        .language_for_name_or_extension(key)
        .await
        .ok()
}

pub fn load_language_blocking(
    registry: &Arc<LanguageRegistry>,
    language_id: &str,
    path: Option<&Path>,
) -> Option<Arc<Language>> {
    block_on(load_language(registry, language_id, path))
}
