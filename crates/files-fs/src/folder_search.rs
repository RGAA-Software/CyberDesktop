use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use crate::file_tag::paths_for_file_tag;

#[cfg(windows)]
use app_platform_windows::search_indexed_aqs;

const DEFAULT_MAX_RESULTS: usize = 500;
const MAX_SEARCH_DEPTH: u32 = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchScope {
    CurrentFolder(PathBuf),
    Home(PathBuf),
    Library(PathBuf),
    Tag(String),
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub path: PathBuf,
    pub display_name: String,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchQuery {
    Plain { text: String },
    Aqs { query: String },
    Tag {
        tag_name: String,
        filter: Option<String>,
    },
}

/// Parse omnibar / search-box text into plain, AQS (`$…`), or tag (`tag:Name`) queries.
pub fn parse_search_query(raw: &str) -> SearchQuery {
    let trimmed = raw.trim();
    if let Some(tag_part) = trimmed.strip_prefix("tag:") {
        let tag_part = tag_part.trim();
        let mut parts = tag_part.splitn(2, char::is_whitespace);
        let tag_name = parts.next().unwrap_or("").to_string();
        let filter = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        return SearchQuery::Tag { tag_name, filter };
    }
    if let Some(aqs) = trimmed.strip_prefix('$') {
        return SearchQuery::Aqs {
            query: aqs.trim().to_string(),
        };
    }
    SearchQuery::Plain {
        text: trimmed.to_string(),
    }
}

pub fn search_folder(
    scope: SearchScope,
    query: &SearchQuery,
    cancel: &AtomicBool,
) -> anyhow::Result<Vec<SearchHit>> {
    match query {
        SearchQuery::Tag { tag_name, filter } => search_tag(tag_name, filter.as_deref(), cancel),
        SearchQuery::Plain { text } if text.is_empty() => Ok(Vec::new()),
        SearchQuery::Plain { text } => {
            let root = scope_directory(&scope)?;
            let needle = text.to_ascii_lowercase();
            search_recursive(&root, &needle, cancel)
        }
        SearchQuery::Aqs { query: text } => {
            let root = scope_directory(&scope)?;
            #[cfg(windows)]
            {
                if let Ok(paths) =
                    search_indexed_aqs(&root, text, cancel, DEFAULT_MAX_RESULTS)
                {
                    let mut hits: Vec<SearchHit> = paths
                        .into_iter()
                        .filter_map(|path| path_to_hit(&path).ok())
                        .collect();
                    hits.sort_by(|left, right| {
                        left.display_name
                            .to_ascii_lowercase()
                            .cmp(&right.display_name.to_ascii_lowercase())
                    });
                    return Ok(hits);
                }
            }
            let needle = text.to_ascii_lowercase();
            search_recursive(&root, &needle, cancel)
        }
    }
}

fn scope_directory(scope: &SearchScope) -> anyhow::Result<PathBuf> {
    match scope {
        SearchScope::CurrentFolder(path)
        | SearchScope::Home(path)
        | SearchScope::Library(path) => {
            if path.is_dir() {
                Ok(path.clone())
            } else {
                anyhow::bail!("search scope is not a directory")
            }
        }
        SearchScope::Tag(_) => anyhow::bail!("tag scope requires a tag: query"),
    }
}

/// Filesystem path shown in breadcrumbs for a search scope.
pub fn search_scope_path(scope: &SearchScope) -> PathBuf {
    match scope {
        SearchScope::CurrentFolder(path) | SearchScope::Home(path) | SearchScope::Library(path) => {
            path.clone()
        }
        SearchScope::Tag(name) => PathBuf::from(format!("tag:{name}")),
    }
}

fn search_tag(
    tag_name: &str,
    filter: Option<&str>,
    cancel: &AtomicBool,
) -> anyhow::Result<Vec<SearchHit>> {
    let filter = filter.map(|value| value.to_ascii_lowercase());
    let mut hits: Vec<SearchHit> = paths_for_file_tag(tag_name)
        .into_iter()
        .take_while(|_| !cancel.load(Ordering::Relaxed))
        .filter(|path| {
            filter.as_ref().is_none_or(|needle| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.to_ascii_lowercase().contains(needle))
            })
        })
        .filter_map(|path| path_to_hit(&path).ok())
        .collect();
    hits.sort_by(|left, right| {
        left.display_name
            .to_ascii_lowercase()
            .cmp(&right.display_name.to_ascii_lowercase())
    });
    Ok(hits)
}

fn search_recursive(
    root: &Path,
    needle: &str,
    cancel: &AtomicBool,
) -> anyhow::Result<Vec<SearchHit>> {
    let mut hits = Vec::new();
    walk_dir(root, needle, cancel, &mut hits, 0);
    hits.sort_by(|left, right| {
        left.display_name
            .to_ascii_lowercase()
            .cmp(&right.display_name.to_ascii_lowercase())
    });
    Ok(hits)
}

fn walk_dir(dir: &Path, needle: &str, cancel: &AtomicBool, hits: &mut Vec<SearchHit>, depth: u32) {
    if cancel.load(Ordering::Relaxed)
        || hits.len() >= DEFAULT_MAX_RESULTS
        || depth > MAX_SEARCH_DEPTH
    {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if cancel.load(Ordering::Relaxed) || hits.len() >= DEFAULT_MAX_RESULTS {
            return;
        }
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if name.to_ascii_lowercase().contains(needle) {
            if let Ok(hit) = path_to_hit(&path) {
                hits.push(hit);
            }
        }
        if path.is_dir() {
            walk_dir(&path, needle, cancel, hits, depth + 1);
        }
    }
}

fn path_to_hit(path: &Path) -> anyhow::Result<SearchHit> {
    let metadata = std::fs::metadata(path)?;
    Ok(SearchHit {
        display_name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        modified: metadata.modified().ok(),
        path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn parse_plain_query() {
        assert_eq!(
            parse_search_query("  readme  "),
            SearchQuery::Plain {
                text: "readme".into()
            }
        );
    }

    #[test]
    fn parse_aqs_query() {
        assert_eq!(
            parse_search_query("$kind:=document"),
            SearchQuery::Aqs {
                query: "kind:=document".into()
            }
        );
    }

    #[test]
    fn parse_tag_query() {
        assert_eq!(
            parse_search_query("tag:Work"),
            SearchQuery::Tag {
                tag_name: "Work".into(),
                filter: None,
            }
        );
        assert_eq!(
            parse_search_query("tag:Work report"),
            SearchQuery::Tag {
                tag_name: "Work".into(),
                filter: Some("report".into()),
            }
        );
    }

    #[test]
    fn recursive_search_finds_nested_file() {
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("alpha").join("beta");
        fs::create_dir_all(&nested).unwrap();
        let target = nested.join("target-readme.txt");
        fs::write(&target, b"x").unwrap();
        fs::write(temp.path().join("ignore.txt"), b"x").unwrap();

        let cancel = AtomicBool::new(false);
        let scope = SearchScope::CurrentFolder(temp.path().to_path_buf());
        let query = SearchQuery::Plain {
            text: "readme".into(),
        };
        let hits = search_folder(scope, &query, &cancel).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, target);
    }
}
