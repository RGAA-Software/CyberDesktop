use std::collections::HashMap;
use std::path::{Path, PathBuf};

use files_core::{load_config, FileTagConfig};

use crate::item::{DirectoryReadOptions, FileItem, TagRef};
use crate::sort::{sort_items, SortPreferences};

/// Preset hex colors for file tags (Files-style palette).
pub const TAG_COLOR_PRESETS: &[&str] = &[
    "#E53935", "#D81B60", "#8E24AA", "#5E35B1", "#3949AB", "#1E88E5", "#039BE5", "#00ACC1",
    "#00897B", "#43A047", "#7CB342", "#C0CA33", "#FDD835", "#FFB300", "#FB8C00", "#F4511E",
    "#6D4C41", "#546E7A",
];

/// Parse `#RRGGBB` or `RRGGBB` into gpui-style `0xRRGGBB`.
pub fn parse_tag_color_hex(color: &str) -> Option<u32> {
    let hex = color.trim().strip_prefix('#').unwrap_or(color.trim());
    if hex.len() != 6 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

fn normalize_tag_color_key(color: &str) -> String {
    color.trim().trim_start_matches('#').to_ascii_lowercase()
}

/// Pick a preset color not already used by existing tags (falls back to full palette when exhausted).
pub fn pick_random_tag_color(used: &[Option<String>]) -> String {
    use std::collections::HashSet;

    let used_set: HashSet<String> = used
        .iter()
        .filter_map(|color| color.as_deref().map(normalize_tag_color_key))
        .collect();

    let available: Vec<&str> = TAG_COLOR_PRESETS
        .iter()
        .copied()
        .filter(|preset| !used_set.contains(&normalize_tag_color_key(preset)))
        .collect();

    let pool: Vec<&str> = if available.is_empty() {
        TAG_COLOR_PRESETS.to_vec()
    } else {
        available
    };

    let idx = tag_color_seed(used.len()) % pool.len();
    pool[idx].to_string()
}

fn tag_color_seed(extra: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0) as usize;
    nanos ^ extra.wrapping_mul(0x9e3779b9)
}

/// Build a flat file list for a tag's associated paths (Files tag search result page subset).
pub fn file_items_for_tag_paths(
    paths: &[PathBuf],
    options: DirectoryReadOptions,
    sort: SortPreferences,
) -> Vec<FileItem> {
    let mut items = Vec::new();
    for path in paths {
        if !path.exists() {
            continue;
        }
        if let Ok(item) = FileItem::from_path(path.clone(), options) {
            items.push(item);
        }
    }
    sort_items(&mut items, sort);
    items
}

/// Paths assigned to a file tag in config (existing paths only).
pub fn paths_for_file_tag(tag_name: &str) -> Vec<PathBuf> {
    load_config()
        .map(|config| config.file_tags)
        .unwrap_or_default()
        .into_iter()
        .find(|tag| tag.name == tag_name)
        .map(|tag| {
            tag.paths
                .iter()
                .map(PathBuf::from)
                .filter(|path| path.exists())
                .collect()
        })
        .unwrap_or_default()
}

/// Given tag definitions, construct path → tags mapping (one path may map to many tags).
pub fn build_path_tag_index(
    tags: &[(String, Option<String>, Vec<PathBuf>)],
) -> HashMap<PathBuf, Vec<TagRef>> {
    let mut index: HashMap<PathBuf, Vec<TagRef>> = HashMap::new();
    for (name, color, paths) in tags {
        let tag_ref = TagRef {
            name: name.clone(),
            color: color.clone(),
        };
        for path in paths {
            index
                .entry(path.clone())
                .or_default()
                .push(tag_ref.clone());
        }
    }
    for tags_for_path in index.values_mut() {
        tags_for_path.sort_by(|left, right| left.name.cmp(&right.name));
        tags_for_path.dedup_by(|left, right| left.name == right.name);
    }
    index
}

pub fn build_path_tag_index_from_config() -> HashMap<PathBuf, Vec<TagRef>> {
    let tags = load_config()
        .map(|config| config.file_tags)
        .unwrap_or_default();
    build_path_tag_index_from_configs(&tags)
}

pub fn build_path_tag_index_from_configs(tags: &[FileTagConfig]) -> HashMap<PathBuf, Vec<TagRef>> {
    let inputs: Vec<(String, Option<String>, Vec<PathBuf>)> = tags
        .iter()
        .map(|tag| {
            (
                tag.name.clone(),
                tag.color.clone(),
                tag.paths.iter().map(PathBuf::from).collect(),
            )
        })
        .collect();
    build_path_tag_index(&inputs)
}

pub fn tags_for_path(index: &HashMap<PathBuf, Vec<TagRef>>, path: &Path) -> Vec<TagRef> {
    index.get(path).cloned().unwrap_or_default()
}

pub fn apply_tags_to_items(items: &mut [FileItem], index: &HashMap<PathBuf, Vec<TagRef>>) {
    for item in items {
        item.tags = tags_for_path(index, &item.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_path_tag_index_maps_multiple_tags_per_path() {
        let tags = vec![
            (
                "Work".into(),
                Some("#E53935".into()),
                vec![PathBuf::from(r"C:\docs\report.pdf")],
            ),
            (
                "Important".into(),
                Some("#1E88E5".into()),
                vec![PathBuf::from(r"C:\docs\report.pdf")],
            ),
            (
                "Personal".into(),
                None,
                vec![PathBuf::from(r"C:\photos\vacation.jpg")],
            ),
        ];
        let index = build_path_tag_index(&tags);
        let report_tags = index
            .get(&PathBuf::from(r"C:\docs\report.pdf"))
            .expect("report path");
        assert_eq!(report_tags.len(), 2);
        assert_eq!(report_tags[0].name, "Important");
        assert_eq!(report_tags[1].name, "Work");
    }

    #[test]
    fn parse_tag_color_hex_accepts_hash_prefix() {
        assert_eq!(parse_tag_color_hex("#E53935"), Some(0xE5_39_35));
        assert_eq!(parse_tag_color_hex("E53935"), Some(0xE5_39_35));
        assert!(parse_tag_color_hex("not-a-color").is_none());
    }

    #[test]
    fn pick_random_tag_color_avoids_used_presets() {
        let used = vec![
            Some("#E53935".into()),
            Some("#D81B60".into()),
            Some("#8E24AA".into()),
        ];
        let picked = pick_random_tag_color(&used);
        assert!(TAG_COLOR_PRESETS.contains(&picked.as_str()));
        assert_ne!(picked.to_ascii_lowercase(), "#e53935");
        assert_ne!(picked.to_ascii_lowercase(), "#d81b60");
        assert_ne!(picked.to_ascii_lowercase(), "#8e24aa");
    }
}
