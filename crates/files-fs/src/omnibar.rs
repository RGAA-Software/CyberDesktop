use std::path::{Path, PathBuf};

use unicode_width::UnicodeWidthChar;

use crate::item::DirectoryReadOptions;

/// One clickable segment in the omnibar breadcrumb trail.
#[derive(Debug, Clone)]
pub struct PathBreadcrumb {
    pub label: String,
    pub path: PathBuf,
}

/// Group in the breadcrumb root (home) chevron menu (Files: Quick access, Drives).
#[derive(Debug, Clone)]
pub struct BreadcrumbMenuSection {
    pub heading: Option<String>,
    pub entries: Vec<OmnibarPathSuggestion>,
}

/// Builds path segments for breadcrumb UI (e.g. `C:\` → `Users` → `hy`).
pub fn path_breadcrumbs(path: &Path) -> Vec<PathBreadcrumb> {
    let mut segments: Vec<PathBuf> = path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect();
    segments.reverse();

    if segments.is_empty() && !path.as_os_str().is_empty() {
        segments.push(path.to_path_buf());
    }

    segments
        .into_iter()
        .map(|segment| PathBreadcrumb {
            label: breadcrumb_label(&segment),
            path: segment,
        })
        .collect()
}

fn breadcrumb_label(path: &Path) -> String {
    let text = path.to_string_lossy();
    #[cfg(windows)]
    {
        let bytes = text.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes.len() <= 3 && bytes.last() == Some(&b'\\')
        {
            return text.to_string();
        }
    }
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| text.to_string())
}

/// Suggestion row for omnibar autocomplete.
#[derive(Debug, Clone, PartialEq)]
pub struct OmnibarPathSuggestion {
    pub path: PathBuf,
    pub label: String,
    /// Semi-transparent row in breadcrumb dropdown (Files: hidden folders when shown).
    pub dimmed: bool,
}

const MAX_SUGGESTIONS: usize = 10;
const MAX_BREADCRUMB_ENTRIES: usize = 50;

/// Result of loading a breadcrumb segment chevron menu (Files `ItemDropDownFlyoutOpening`).
#[derive(Debug, Clone, PartialEq)]
pub enum BreadcrumbDropdownResult {
    /// `read_dir` failed (e.g. access denied).
    AccessDenied,
    /// Readable but no subfolders after filtering.
    Empty,
    Entries(Vec<OmnibarPathSuggestion>),
}

/// Subfolder entries for a breadcrumb segment dropdown (chevron menu).
///
/// `exclude_path` — skip the active directory (Files: no navigate to current folder).
pub fn breadcrumb_dropdown_entries(
    path: &Path,
    read_options: DirectoryReadOptions,
    exclude_path: Option<&Path>,
) -> BreadcrumbDropdownResult {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return BreadcrumbDropdownResult::AccessDenied;
        }
        Err(_) => return BreadcrumbDropdownResult::AccessDenied,
    };

    let mut suggestions = Vec::new();
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }
        let hidden = is_hidden_dir_entry(&entry_path, &entry);
        let system = is_system_dir_entry(&entry);
        let dot = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if hidden && !read_options.show_hidden_items {
            continue;
        }
        if system && !read_options.show_system_items {
            continue;
        }
        if dot && !read_options.show_dot_files {
            continue;
        }
        if exclude_path.is_some_and(|ex| paths_equal(&entry_path, ex)) {
            continue;
        }
        let dimmed = hidden && read_options.show_hidden_items;
        suggestions.push(OmnibarPathSuggestion {
            label: entry.file_name().to_string_lossy().to_string(),
            path: entry_path,
            dimmed,
        });
        if suggestions.len() >= MAX_BREADCRUMB_ENTRIES {
            break;
        }
    }

    suggestions.sort_by(|a, b| a.label.cmp(&b.label));
    if suggestions.is_empty() {
        BreadcrumbDropdownResult::Empty
    } else {
        BreadcrumbDropdownResult::Entries(suggestions)
    }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    std::fs::canonicalize(a)
        .ok()
        .zip(std::fs::canonicalize(b).ok())
        .map(|(a, b)| a == b)
        .unwrap_or_else(|| a == b)
}

#[cfg(windows)]
fn is_hidden_dir_entry(path: &Path, entry: &std::fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    entry
        .metadata()
        .map(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
        .unwrap_or_else(|_| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'))
        })
}

#[cfg(not(windows))]
fn is_hidden_dir_entry(path: &Path, _: &std::fs::DirEntry) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

#[cfg(windows)]
fn is_system_dir_entry(entry: &std::fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    entry
        .metadata()
        .map(|m| m.file_attributes() & FILE_ATTRIBUTE_SYSTEM != 0)
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn is_system_dir_entry(_: &std::fs::DirEntry) -> bool {
    false
}

/// Root chevron menu: pinned folders then drives (Files `ItemDropDownFlyoutOpening` parity).
pub fn breadcrumb_root_menu_sections(
    quick_access: impl IntoIterator<Item = (String, PathBuf)>,
    drives: impl IntoIterator<Item = (String, PathBuf)>,
    quick_access_heading: Option<String>,
    drives_heading: Option<String>,
) -> Vec<BreadcrumbMenuSection> {
    let quick: Vec<_> = quick_access
        .into_iter()
        .map(|(label, path)| OmnibarPathSuggestion {
            label,
            path,
            dimmed: false,
        })
        .collect();
    let drive_list: Vec<_> = drives
        .into_iter()
        .map(|(label, path)| OmnibarPathSuggestion {
            label,
            path,
            dimmed: false,
        })
        .collect();

    let mut sections = Vec::new();
    if !quick.is_empty() {
        sections.push(BreadcrumbMenuSection {
            heading: quick_access_heading,
            entries: quick,
        });
    }
    if !drive_list.is_empty() {
        sections.push(BreadcrumbMenuSection {
            heading: drives_heading,
            entries: drive_list,
        });
    }
    sections
}

/// Path-mode suggestions: history when input is empty/special, otherwise child folder names.
pub fn omnibar_path_suggestions(
    query: &str,
    path_history: &[String],
) -> Vec<OmnibarPathSuggestion> {
    let trimmed = query.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("home")
        || trimmed.eq_ignore_ascii_case("settings")
        || trimmed.eq_ignore_ascii_case("recycle bin")
    {
        return path_history
            .iter()
            .take(MAX_SUGGESTIONS)
            .map(|entry| {
                let path = PathBuf::from(entry);
                OmnibarPathSuggestion {
                    label: entry.clone(),
                    path,
                    dimmed: false,
                }
            })
            .collect();
    }

    let (parent, partial) = parent_and_partial(trimmed);
    let parent_path = PathBuf::from(&parent);
    if !parent_path.is_dir() {
        return Vec::new();
    }

    let partial_lower = partial.to_ascii_lowercase();
    let mut suggestions = Vec::new();

    let Ok(entries) = std::fs::read_dir(&parent_path) else {
        return suggestions;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !partial.is_empty() && !name.to_ascii_lowercase().starts_with(&partial_lower) {
            continue;
        }
        suggestions.push(OmnibarPathSuggestion {
            label: path.to_string_lossy().to_string(),
            path,
            dimmed: false,
        });
        if suggestions.len() >= MAX_SUGGESTIONS {
            break;
        }
    }

    suggestions.sort_by(|a, b| a.label.cmp(&b.label));
    suggestions
}

/// Global-search suggestions: recent queries when input is empty, otherwise prefix/filter match.
pub fn omnibar_search_suggestions(query: &str, search_history: &[String]) -> Vec<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return search_history
            .iter()
            .take(MAX_SUGGESTIONS)
            .cloned()
            .collect();
    }
    let needle = trimmed.to_ascii_lowercase();
    search_history
        .iter()
        .filter(|entry| entry.to_ascii_lowercase().contains(&needle))
        .take(MAX_SUGGESTIONS)
        .cloned()
        .collect()
}

/// Which path segments are visible vs hidden in the ellipsis menu (Files `BreadcrumbBarLayout`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbVisibleLayout {
    /// Segments `[0..hidden_prefix_len)` appear only in the `…` dropdown.
    pub hidden_prefix_len: usize,
    /// Visible segment indices (suffix), in order.
    pub visible_indices: Vec<usize>,
}

const ROOT_BLOCK_WIDTH: f32 = 72.0;
const ELLIPSIS_BLOCK_WIDTH: f32 = 40.0;
const SEGMENT_PADDING: f32 = 24.0;
/// Conservative estimate for `text_sm()` (~14px) when pixel metrics are unavailable.
const SEGMENT_CELL_WIDTH: f32 = 8.5;
const CHEVRON_WIDTH: f32 = 32.0;
/// `path-breadcrumb-bar` `gap(px(2.))` between blocks.
pub const BREADCRUMB_BLOCK_GAP: f32 = 2.0;

fn segment_text_width_px(label: &str) -> f32 {
    let units: u32 = label.chars().map(|c| c.width().unwrap_or(1) as u32).sum();
    (units.max(1) as f32) * SEGMENT_CELL_WIDTH
}

fn segment_width_px(label: &str, has_chevron: bool) -> f32 {
    SEGMENT_PADDING + segment_text_width_px(label) + if has_chevron { CHEVRON_WIDTH } else { 0.0 }
}

fn breadcrumb_trail_width_px(
    segment_widths: &[f32],
    first_visible: usize,
    show_root: bool,
    root_width: f32,
    ellipsis_width: f32,
    block_gap: f32,
) -> f32 {
    let n = segment_widths.len();
    let mut blocks: Vec<f32> = Vec::new();
    if show_root {
        blocks.push(root_width);
    }
    if first_visible > 0 {
        blocks.push(ellipsis_width);
    }
    blocks.extend_from_slice(&segment_widths[first_visible..n]);
    let sum: f32 = blocks.iter().sum();
    let gaps = if blocks.len() > 1 {
        block_gap * (blocks.len() - 1) as f32
    } else {
        0.0
    };
    sum + gaps
}

/// Files-style collapse using pre-measured block widths (preferred in UI).
pub fn breadcrumb_visible_layout_for_widths(
    segment_widths: &[f32],
    available_width: f32,
    show_root: bool,
    root_width: f32,
    ellipsis_width: f32,
    block_gap: f32,
) -> BreadcrumbVisibleLayout {
    let n = segment_widths.len();
    if n == 0 {
        return BreadcrumbVisibleLayout {
            hidden_prefix_len: 0,
            visible_indices: Vec::new(),
        };
    }

    let root = if show_root { root_width } else { 0.0 };

    for first_visible in 0..n {
        let total = breadcrumb_trail_width_px(
            segment_widths,
            first_visible,
            show_root,
            root,
            ellipsis_width,
            block_gap,
        );
        if total <= available_width {
            return BreadcrumbVisibleLayout {
                hidden_prefix_len: first_visible,
                visible_indices: (first_visible..n).collect(),
            };
        }
    }

    BreadcrumbVisibleLayout {
        hidden_prefix_len: n.saturating_sub(1),
        visible_indices: vec![n - 1],
    }
}

/// Files-style collapse: hide the **prefix** when the trail does not fit (keep tail visible).
pub fn breadcrumb_visible_layout_for_width(
    segments: &[PathBreadcrumb],
    available_width: f32,
    show_root: bool,
) -> BreadcrumbVisibleLayout {
    let n = segments.len();
    let widths: Vec<f32> = segments
        .iter()
        .enumerate()
        .map(|(i, s)| segment_width_px(&s.label, i + 1 < n))
        .collect();
    breadcrumb_visible_layout_for_widths(
        &widths,
        available_width,
        show_root,
        ROOT_BLOCK_WIDTH,
        ELLIPSIS_BLOCK_WIDTH,
        BREADCRUMB_BLOCK_GAP,
    )
}

/// Back-compat helper when width is unknown (assume a wide bar).
pub fn breadcrumb_visible_layout(segment_count: usize) -> BreadcrumbVisibleLayout {
    breadcrumb_visible_layout_for_width(
        &(0..segment_count)
            .map(|i| PathBreadcrumb {
                label: format!("segment-{i}"),
                path: PathBuf::from(format!("/{i}")),
            })
            .collect::<Vec<_>>(),
        f32::MAX,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn path_breadcrumbs_windows_drive_root() {
        let crumbs = path_breadcrumbs(Path::new(r"C:\"));
        assert_eq!(crumbs.len(), 1);
        assert_eq!(crumbs[0].label, "C:\\");
        assert_eq!(crumbs[0].path, Path::new(r"C:\"));
    }

    #[test]
    fn path_breadcrumbs_nested_path() {
        let crumbs = path_breadcrumbs(Path::new(r"C:\Users\hy"));
        assert_eq!(crumbs.len(), 3);
        assert_eq!(crumbs[0].path, Path::new(r"C:\"));
        assert_eq!(crumbs[1].path, Path::new(r"C:\Users"));
        assert_eq!(crumbs[2].path, Path::new(r"C:\Users\hy"));
    }

    #[test]
    fn segment_width_treats_cjk_wider_than_ascii() {
        let ascii = segment_width_px("abc", false);
        let cjk = segment_width_px("中文", false);
        assert!(cjk > ascii, "CJK labels must reserve more horizontal space");
    }

    #[test]
    fn breadcrumb_visible_layout_hides_prefix_when_narrow() {
        let segments: Vec<PathBreadcrumb> = (0..6)
            .map(|i| PathBreadcrumb {
                label: format!("part-{i}"),
                path: PathBuf::from(format!(r"C:\p{i}")),
            })
            .collect();
        let layout = breadcrumb_visible_layout_for_width(&segments, 200.0, true);
        assert!(layout.hidden_prefix_len > 0);
        assert_eq!(
            *layout.visible_indices.last().unwrap(),
            5,
            "last segment stays visible"
        );
        assert!(layout.visible_indices.windows(2).all(|w| w[0] + 1 == w[1]));
    }

    #[test]
    fn breadcrumb_visible_layout_collapses_deep_trail_when_measured_wide() {
        let labels = [
            "C:\\",
            "Users",
            "developer",
            "Documents",
            "Projects",
            "CyberFiles",
            "crates",
            "ui",
        ];
        let n = labels.len();
        let widths: Vec<f32> = labels
            .iter()
            .enumerate()
            .map(|(i, label)| segment_width_px(label, i + 1 < n))
            .collect();
        let layout = breadcrumb_visible_layout_for_widths(
            &widths,
            420.0,
            true,
            ROOT_BLOCK_WIDTH,
            ELLIPSIS_BLOCK_WIDTH,
            BREADCRUMB_BLOCK_GAP,
        );
        assert!(
            layout.hidden_prefix_len >= 4,
            "deep paths should hide more than two prefix segments, got hidden_prefix_len={}",
            layout.hidden_prefix_len
        );
        assert!(!layout.visible_indices.is_empty());
        assert_eq!(*layout.visible_indices.last().unwrap(), n - 1);
    }

    #[test]
    fn breadcrumb_visible_layout_shows_all_when_wide() {
        let segments = path_breadcrumbs(Path::new(r"C:\Users\hy"));
        let layout = breadcrumb_visible_layout_for_width(&segments, 10_000.0, true);
        assert_eq!(layout.hidden_prefix_len, 0);
        assert_eq!(layout.visible_indices, vec![0, 1, 2]);
    }

    #[test]
    fn breadcrumb_dropdown_excludes_working_path() {
        let parent = std::env::temp_dir();
        let child = parent.join("breadcrumb_dropdown_test_dir");
        let _ = std::fs::create_dir_all(&child);
        let result = breadcrumb_dropdown_entries(
            &parent,
            DirectoryReadOptions {
                show_hidden_items: true,
                ..Default::default()
            },
            Some(&child),
        );
        let BreadcrumbDropdownResult::Entries(entries) = result else {
            panic!("expected entries, got {result:?}");
        };
        assert!(!entries.iter().any(|e| e.path == child));
        let _ = std::fs::remove_dir_all(&child);
    }

    #[test]
    fn breadcrumb_root_menu_sections_order() {
        let sections = breadcrumb_root_menu_sections(
            [("Docs".into(), PathBuf::from(r"C:\Docs"))],
            [("C:\\".into(), PathBuf::from(r"C:\"))],
            Some("Quick".into()),
            Some("Drives".into()),
        );
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].entries[0].label, "Docs");
        assert_eq!(sections[1].entries[0].label, "C:\\");
    }
}

fn parent_and_partial(input: &str) -> (String, String) {
    let path = Path::new(input);
    let parent = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| input.to_string());
    let partial = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    (parent, partial)
}
