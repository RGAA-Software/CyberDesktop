//! Global "search in files" across a directory tree.
//!
//! Built on the same libraries ripgrep uses: [`ignore`] for fast,
//! gitignore-aware traversal and `grep-searcher` + `grep-regex` for matching
//! (including automatic binary-file skipping). This is what powers a
//! Notepad++-style "Find in Files".

use std::path::{Path, PathBuf};

use anyhow::Result;
use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use ignore::WalkBuilder;

/// Options for a global search.
#[derive(Debug, Clone)]
pub struct GlobalSearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    /// Include hidden files / directories (and ignore .gitignore) when true.
    pub include_hidden: bool,
    /// Stop after collecting this many total matches.
    pub max_matches: usize,
}

impl Default for GlobalSearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            whole_word: false,
            regex: false,
            include_hidden: false,
            max_matches: 10_000,
        }
    }
}

/// A single matching line within a file.
#[derive(Debug, Clone)]
pub struct LineMatch {
    /// 1-based line number.
    pub line_number: u64,
    /// The matching line text, with the trailing newline trimmed.
    pub line_text: String,
    /// Byte offset of the match start within `line_text`.
    pub start: usize,
    /// Byte offset of the match end within `line_text`.
    pub end: usize,
}

/// All matches found in one file.
#[derive(Debug, Clone)]
pub struct FileMatches {
    pub path: PathBuf,
    pub matches: Vec<LineMatch>,
}

/// Searches `root` recursively for `query`.
pub fn search_directory(
    root: &Path,
    query: &str,
    options: &GlobalSearchOptions,
) -> Result<Vec<FileMatches>> {
    let pattern = if options.regex {
        query.to_string()
    } else {
        regex::escape(query)
    };

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!options.case_sensitive)
        .word(options.whole_word)
        .build(&pattern)?;

    let mut results: Vec<FileMatches> = Vec::new();
    let mut total = 0usize;

    let walk = WalkBuilder::new(root)
        .hidden(!options.include_hidden)
        .git_ignore(!options.include_hidden)
        .build();

    for entry in walk {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path().to_path_buf();

        let mut file_matches: Vec<LineMatch> = Vec::new();
        let matcher_ref = &matcher;
        let mut searcher = Searcher::new();
        let search_result = searcher.search_path(
            matcher_ref,
            &path,
            UTF8(|line_number, line| {
                if let Ok(Some(m)) = matcher_ref.find(line.as_bytes()) {
                    file_matches.push(LineMatch {
                        line_number,
                        line_text: line.trim_end_matches(['\r', '\n']).to_string(),
                        start: m.start(),
                        end: m.end(),
                    });
                }
                Ok(true)
            }),
        );
        if search_result.is_err() {
            continue;
        }

        if !file_matches.is_empty() {
            total += file_matches.len();
            results.push(FileMatches {
                path,
                matches: file_matches,
            });
            if total >= options.max_matches {
                break;
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_matches_across_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello world\nfoo\n").unwrap();
        fs::write(dir.path().join("b.txt"), "no match here\n").unwrap();
        fs::write(dir.path().join("c.txt"), "another hello\n").unwrap();

        let opts = GlobalSearchOptions::default();
        let results = search_directory(dir.path(), "hello", &opts).unwrap();

        let total: usize = results.iter().map(|f| f.matches.len()).sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn reports_columns() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "xx target yy\n").unwrap();
        let opts = GlobalSearchOptions::default();
        let results = search_directory(dir.path(), "target", &opts).unwrap();
        assert_eq!(results.len(), 1);
        let m = &results[0].matches[0];
        assert_eq!(m.start, 3);
        assert_eq!(m.end, 9);
        assert_eq!(m.line_number, 1);
    }
}
