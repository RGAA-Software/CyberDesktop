use std::collections::{BTreeSet, HashMap};

use chrono::{Datelike, Local, NaiveDate, TimeZone};

use crate::item::{FileItem, FileItemKind};
use crate::sort::SortDirection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupOption {
    #[default]
    None,
    Name,
    DateModified,
    DateCreated,
    Size,
    FileType,
    Tag,
}

/// Granularity when grouping by date fields (matches Files `GroupByDateUnit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupByDateUnit {
    #[default]
    Year,
    Month,
    Day,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileGroup {
    pub key: String,
    pub title: String,
    pub sort_index: i64,
    pub item_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayRow {
    GroupHeader {
        key: String,
        title: String,
        count: usize,
        collapsed: bool,
    },
    Item(usize),
}

struct TimeSpanLabel {
    key: String,
    sort_index: i64,
}

pub fn group_key_for(
    item: &FileItem,
    option: GroupOption,
    date_unit: GroupByDateUnit,
) -> (String, String) {
    let label = group_label_for(item, option, date_unit);
    (label.key.clone(), label.key)
}

fn group_label_for(
    item: &FileItem,
    option: GroupOption,
    date_unit: GroupByDateUnit,
) -> TimeSpanLabel {
    match option {
        GroupOption::None => TimeSpanLabel {
            key: String::new(),
            sort_index: 0,
        },
        GroupOption::Name => {
            let (key, _) = name_group(item);
            TimeSpanLabel {
                key: key.clone(),
                sort_index: name_sort_index(&key),
            }
        }
        GroupOption::DateModified => time_span_label(item.modified, date_unit, Local::now()),
        GroupOption::DateCreated => time_span_label(item.created, date_unit, Local::now()),
        GroupOption::Size => {
            let (key, _) = size_group(item);
            TimeSpanLabel {
                key: key.clone(),
                sort_index: size_sort_index(&key),
            }
        }
        GroupOption::FileType => {
            let (key, _) = type_group(item);
            TimeSpanLabel {
                key: key.clone(),
                sort_index: type_sort_index(&key),
            }
        }
        GroupOption::Tag => tag_group(item),
    }
}

fn name_group(item: &FileItem) -> (String, String) {
    let first = item
        .display_name
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().collect::<String>())
        .filter(|ch| ch.chars().next().is_some_and(|c| c.is_alphabetic()))
        .unwrap_or_else(|| "#".into());
    (first.clone(), first)
}

fn name_sort_index(key: &str) -> i64 {
    if key == "#" {
        i64::MAX
    } else {
        0
    }
}

fn size_group(item: &FileItem) -> (String, String) {
    if item.is_folder() {
        return ("size:folder".into(), String::new());
    }
    let size = item.size.unwrap_or(0);
    if size == 0 {
        return ("size:empty".into(), String::new());
    }
    if size < 16 * 1024 {
        return ("size:tiny".into(), String::new());
    }
    if size < 1024 * 1024 {
        return ("size:small".into(), String::new());
    }
    if size < 128 * 1024 * 1024 {
        return ("size:medium".into(), String::new());
    }
    if size < 1024 * 1024 * 1024 {
        return ("size:large".into(), String::new());
    }
    ("size:huge".into(), String::new())
}

fn size_sort_index(key: &str) -> i64 {
    match key {
        "size:folder" => 0,
        "size:empty" => 1,
        "size:tiny" => 2,
        "size:small" => 3,
        "size:medium" => 4,
        "size:large" => 5,
        "size:huge" => 6,
        _ => 7,
    }
}

fn type_group(item: &FileItem) -> (String, String) {
    // Network virtual-folder items group by their Shell category.
    if let Some(ref cat) = item.network_category {
        let label = item
            .network_category_label
            .clone()
            .unwrap_or_else(|| cat.clone());
        return (format!("net:{}", cat), label);
    }
    match item.kind {
        FileItemKind::Folder => ("type:folder".into(), String::new()),
        FileItemKind::Symlink => ("type:symlink".into(), String::new()),
        FileItemKind::Other => ("type:other".into(), String::new()),
        FileItemKind::File => {
            let ext = item
                .extension
                .as_deref()
                .filter(|ext| !ext.is_empty())
                .map(str::to_ascii_uppercase)
                .unwrap_or_else(|| "File".into());
            if ext.eq_ignore_ascii_case("file") {
                ("type:file".into(), String::new())
            } else {
                (format!("type:ext:{}", ext.to_ascii_lowercase()), ext)
            }
        }
    }
}

fn type_sort_index(key: &str) -> i64 {
    if key.starts_with("net:") {
        return network_category_sort_index(key);
    }
    if key == "type:folder" {
        0
    } else {
        1
    }
}

fn network_category_sort_index(key: &str) -> i64 {
    match key.strip_prefix("net:") {
        Some("network.category.infrastructure") => 0,
        Some("network.category.computer") => 1,
        Some("network.category.media_device") => 2,
        Some("network.category.printer") => 3,
        Some("network.category.other_device") => 4,
        _ => 5,
    }
}

fn tag_group(item: &FileItem) -> TimeSpanLabel {
    if let Some(tag) = item.tags.first() {
        TimeSpanLabel {
            key: format!("tag:{}", tag.name),
            sort_index: 0,
        }
    } else {
        TimeSpanLabel {
            key: "tag:none".into(),
            sort_index: i64::MAX,
        }
    }
}

fn week_of_year(date: NaiveDate) -> (i32, u32) {
    let iso = date.iso_week();
    (iso.year(), iso.week())
}

/// Port of Files `AbstractDateTimeFormatter.ToTimeSpanLabel` using stable locale-neutral keys.
fn time_span_label(
    time: Option<std::time::SystemTime>,
    unit: GroupByDateUnit,
    now: chrono::DateTime<Local>,
) -> TimeSpanLabel {
    let Some(time) = time else {
        return TimeSpanLabel {
            key: "date:unknown".into(),
            sort_index: -1,
        };
    };
    let Ok(duration) = time.duration_since(std::time::UNIX_EPOCH) else {
        return TimeSpanLabel {
            key: "date:unknown".into(),
            sort_index: -1,
        };
    };
    let Some(datetime) = Local.timestamp_opt(duration.as_secs() as i64, 0).single() else {
        return TimeSpanLabel {
            key: "date:unknown".into(),
            sort_index: -1,
        };
    };

    let today = now.date_naive();
    let date = datetime.date_naive();
    let diff_days = (today - date).num_days();

    if date > today {
        return TimeSpanLabel {
            key: "date:future".into(),
            sort_index: 1_000_000_006,
        };
    }
    if date == today {
        return TimeSpanLabel {
            key: "date:today".into(),
            sort_index: 1_000_000_005,
        };
    }
    if date == today.pred_opt().unwrap_or(today) {
        return TimeSpanLabel {
            key: "date:yesterday".into(),
            sort_index: 1_000_000_004,
        };
    }
    if unit == GroupByDateUnit::Day {
        return TimeSpanLabel {
            key: format!(
                "date:day:{:04}{:02}{:02}",
                date.year(),
                date.month(),
                date.day()
            ),
            sort_index: date.year() as i64 * 10_000 + date.month() as i64 * 100 + date.day() as i64,
        };
    }
    if diff_days <= 7 && week_of_year(today) == week_of_year(date) {
        return TimeSpanLabel {
            key: "date:earlier_week".into(),
            sort_index: 1_000_000_003,
        };
    }
    if diff_days <= 14 && week_of_year(today - chrono::Duration::days(7)) == week_of_year(date) {
        return TimeSpanLabel {
            key: "date:last_week".into(),
            sort_index: 1_000_000_002,
        };
    }
    if date.year() == today.year() && date.month() == today.month() {
        return TimeSpanLabel {
            key: "date:earlier_month".into(),
            sort_index: 1_000_000_001,
        };
    }
    let last_month = if today.month() == 1 {
        NaiveDate::from_ymd_opt(today.year() - 1, 12, 1).unwrap_or(today)
    } else {
        NaiveDate::from_ymd_opt(today.year(), today.month() - 1, 1).unwrap_or(today)
    };
    if date.year() == last_month.year() && date.month() == last_month.month() {
        return TimeSpanLabel {
            key: "date:last_month".into(),
            sort_index: 1_000_000_000,
        };
    }
    if unit == GroupByDateUnit::Month {
        return TimeSpanLabel {
            key: format!("date:month:{:04}{:02}", date.year(), date.month()),
            sort_index: date.year() as i64 * 10_000 + date.month() as i64 * 100,
        };
    }
    if date.year() == today.year() {
        return TimeSpanLabel {
            key: "date:earlier_year".into(),
            sort_index: 10_000_001,
        };
    }
    if date.year() == today.year() - 1 {
        return TimeSpanLabel {
            key: "date:last_year".into(),
            sort_index: 10_000_000,
        };
    }
    TimeSpanLabel {
        key: format!("date:year:{}", date.year()),
        sort_index: date.year() as i64,
    }
}

/// Group items by bucket key (order-independent — matches Files merge-by-key).
pub fn group_items(
    items: &[FileItem],
    option: GroupOption,
    date_unit: GroupByDateUnit,
    direction: SortDirection,
) -> Vec<FileGroup> {
    if option == GroupOption::None || items.is_empty() {
        return Vec::new();
    }

    let mut groups_map: HashMap<String, FileGroup> = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        let label = group_label_for(item, option, date_unit);
        groups_map
            .entry(label.key.clone())
            .or_insert_with(|| FileGroup {
                key: label.key.clone(),
                title: label.key.clone(),
                sort_index: label.sort_index,
                item_indices: Vec::new(),
            })
            .item_indices
            .push(index);
    }

    let mut groups: Vec<FileGroup> = groups_map.into_values().collect();
    groups.sort_by(|left, right| {
        let ordering = left
            .sort_index
            .cmp(&right.sort_index)
            .then_with(|| left.key.cmp(&right.key));
        match direction {
            SortDirection::Ascending => ordering,
            SortDirection::Descending => ordering.reverse(),
        }
    });
    groups
}

pub fn build_display_rows(
    items: &[FileItem],
    option: GroupOption,
    date_unit: GroupByDateUnit,
    direction: SortDirection,
    collapsed: &BTreeSet<String>,
) -> Vec<DisplayRow> {
    if option == GroupOption::None {
        return (0..items.len()).map(DisplayRow::Item).collect();
    }

    let groups = group_items(items, option, date_unit, direction);
    let mut rows = Vec::new();
    for group in groups {
        let collapsed = collapsed.contains(&group.key);
        rows.push(DisplayRow::GroupHeader {
            key: group.key.clone(),
            title: group.title.clone(),
            count: group.item_indices.len(),
            collapsed,
        });
        if !collapsed {
            for index in group.item_indices {
                rows.push(DisplayRow::Item(index));
            }
        }
    }
    rows
}

pub fn item_index_at_row(rows: &[DisplayRow], row: usize) -> Option<usize> {
    match rows.get(row)? {
        DisplayRow::Item(index) => Some(*index),
        DisplayRow::GroupHeader { .. } => None,
    }
}

pub fn row_for_item_index(rows: &[DisplayRow], item_index: usize) -> Option<usize> {
    rows.iter().position(|row| {
        matches!(row, DisplayRow::Item(index) if *index == item_index)
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration as StdDuration, UNIX_EPOCH};

    use super::*;
    use crate::item::FileItemKind;

    fn item(name: &str, kind: FileItemKind, size: Option<u64>) -> FileItem {
        FileItem {
            path: PathBuf::from(name),
            name_raw: name.to_string(),
            display_name: name.to_string(),
            extension: PathBuf::from(name)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string()),
            kind,
            size,
            created: None,
            modified: None,
            accessed: None,
            is_hidden: false,
            is_system: false,
            is_readonly: false,
            is_symlink: false,
            tags: Vec::new(),
            network_category: None,
            network_category_label: None,
        }
    }

    fn item_with_tag(name: &str, tag: &str) -> FileItem {
        let mut file = item(name, FileItemKind::File, Some(1));
        file.tags = vec![crate::item::TagRef {
            name: tag.into(),
            color: None,
        }];
        file
    }

    fn item_with_modified(name: &str, modified: std::time::SystemTime) -> FileItem {
        let mut file = item(name, FileItemKind::File, Some(1));
        file.modified = Some(modified);
        file
    }

    #[test]
    fn tag_group_uses_first_tag_name() {
        let (key, _) = group_key_for(
            &item_with_tag("a.txt", "Work"),
            GroupOption::Tag,
            GroupByDateUnit::Year,
        );
        assert_eq!(key, "tag:Work");
    }

    #[test]
    fn tag_group_untagged_uses_none_key() {
        let (key, _) = group_key_for(
            &item("plain.txt", FileItemKind::File, Some(1)),
            GroupOption::Tag,
            GroupByDateUnit::Year,
        );
        assert_eq!(key, "tag:none");
    }

    fn system_time_on(date: NaiveDate) -> std::time::SystemTime {
        let datetime = date.and_hms_opt(12, 0, 0).unwrap();
        let timestamp = Local
            .from_local_datetime(&datetime)
            .single()
            .expect("valid local datetime")
            .timestamp();
        UNIX_EPOCH + StdDuration::from_secs(timestamp.max(0) as u64)
    }

    #[test]
    fn name_group_non_alpha_goes_to_hash() {
        let (key, title) = group_key_for(
            &item("9readme.txt", FileItemKind::File, Some(1)),
            GroupOption::Name,
            GroupByDateUnit::Year,
        );
        assert_eq!(key, "#");
        assert_eq!(title, "#");
    }

    #[test]
    fn groups_merge_non_adjacent_items() {
        let today = Local::now().date_naive();
        let items = vec![
            item_with_modified("b.txt", system_time_on(today)),
            item_with_modified("a.txt", system_time_on(today)),
            item_with_modified("c.txt", system_time_on(today)),
        ];
        let groups = group_items(
            &items,
            GroupOption::DateModified,
            GroupByDateUnit::Year,
            SortDirection::Ascending,
        );
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "date:today");
        assert_eq!(groups[0].item_indices.len(), 3);
    }

    #[test]
    fn day_unit_uses_stable_day_key() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
        let now = Local
            .from_local_datetime(&today.and_hms_opt(12, 0, 0).unwrap())
            .single()
            .unwrap();
        let old = system_time_on(NaiveDate::from_ymd_opt(2026, 5, 20).unwrap());
        let label = time_span_label(Some(old), GroupByDateUnit::Day, now);
        assert_eq!(label.key, "date:day:20260520");
    }

    #[test]
    fn year_unit_uses_year_key() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
        let now = Local
            .from_local_datetime(&today.and_hms_opt(12, 0, 0).unwrap())
            .single()
            .unwrap();
        let old = system_time_on(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap());
        let label = time_span_label(Some(old), GroupByDateUnit::Year, now);
        assert_eq!(label.key, "date:year:2024");
    }

    #[test]
    fn network_items_group_by_category() {
        let mut computer = item("PC1", FileItemKind::Folder, None);
        computer.network_category = Some("network.category.computer".into());
        computer.network_category_label = Some("Computer".into());

        let mut printer = item("Printer1", FileItemKind::Folder, None);
        printer.network_category = Some("network.category.printer".into());
        printer.network_category_label = Some("Printer".into());

        let mut media = item("Media1", FileItemKind::Folder, None);
        media.network_category = Some("network.category.media_device".into());
        media.network_category_label = Some("Media Device".into());

        let items = vec![computer.clone(), printer.clone(), media.clone()];
        let groups = group_items(
            &items,
            GroupOption::FileType,
            GroupByDateUnit::Year,
            SortDirection::Ascending,
        );
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].key, "net:network.category.computer");
        assert_eq!(groups[1].key, "net:network.category.media_device");
        assert_eq!(groups[2].key, "net:network.category.printer");
    }
}
